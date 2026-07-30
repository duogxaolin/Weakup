//! The scheduler: sole owner of timers and sole writer of job state (task 10.1).
//!
//! # Why there are no per-job timers
//!
//! The obvious design is one timer per job, armed for the remaining duration. It is
//! also wrong on a desktop, because a laptop lid closes. A timer armed for two hours
//! does not advance across suspend on every platform, and even where it does, a clock
//! change or a timezone change silently invalidates it. The bug that follows is the
//! worst kind: the machine simply never shuts down, and nothing logs an error.
//!
//! So there are no per-job timers. There is one periodic tick that, on every pass,
//! recomputes each job from its absolute `target_instant_utc` (D8). Waking from sleep
//! needs no special handling — the next tick reads the same absolute instant and
//! notices it has passed. Task 10.7's "recompute rather than trust a running timer"
//! is therefore a property of the design rather than a step someone has to remember.
//!
//! # Why the grace period runs inside the tick
//!
//! [`PowerOffGate::run`] blocks for a minute. Running it inline blocks the next tick,
//! which is what we want: nothing else should fire while a shutdown is counting down.
//! Cancellation still works because it arrives on another thread via
//! [`JobScheduler::cancel_grace_period`].

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use chrono_tz::Tz;

use crate::application::grace_period::{GraceOutcome, GraceState, PowerOffGate};
use crate::core::{AppError, AppResult};
use crate::data::JobRepository;
use crate::domain::{
    Job, JobStatus, JobType, ReconcileOutcome, TriggerResolver, TriggerSpec,
    POWER_OFF_OVERTOLERANCE_MINUTES,
};
use crate::platform::keep_awake::KeepAwakeCoordinator;

/// Time source, injected so tests can place "now" wherever a rule needs it.
pub trait SchedulerClock: Send + Sync {
    fn now_utc(&self) -> DateTime<Utc>;
}

/// The real clock.
pub struct SystemSchedulerClock;

impl SchedulerClock for SystemSchedulerClock {
    fn now_utc(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

/// Something worth telling the user about, emitted by the scheduler as it works.
///
/// A trait rather than a direct notification call so the scheduler stays free of
/// Tauri and remains testable. Section 9 plugs the real notifier in behind this.
pub trait SchedulerObserver: Send + Sync {
    /// A keep-awake job reached its target and ended normally.
    fn job_completed(&self, job: &Job);
    /// A power-off countdown started. `seconds` is the full grace length.
    fn grace_period_started(&self, job: &Job, seconds: u64);
    /// The user cancelled a countdown; the machine stays on.
    fn grace_period_cancelled(&self, job: &Job);
    /// A power-off was attempted and failed. Never silent (task 10.10).
    fn power_off_failed(&self, job: &Job, error: &AppError);
    /// A power-off target passed by more than the tolerance while the app was not
    /// running. Deliberately not executed (task 10.5).
    fn job_overdue(&self, job: &Job, minutes_late: i64);
}

/// Discards everything. The default when no observer is attached.
pub struct NullObserver;

impl SchedulerObserver for NullObserver {
    fn job_completed(&self, _job: &Job) {}
    fn grace_period_started(&self, _job: &Job, _seconds: u64) {}
    fn grace_period_cancelled(&self, _job: &Job) {}
    fn power_off_failed(&self, _job: &Job, _error: &AppError) {}
    fn job_overdue(&self, _job: &Job, _minutes_late: i64) {}
}

/// What a caller must confirm before a request is carried out.
///
/// Returned instead of acting, so the decision is the user's. Task 10.9 requires the
/// single-active-job replacement to be confirmed explicitly rather than happening
/// quietly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfirmationRequired {
    /// A job of this type already holds the active slot.
    ReplaceActiveJob { job_type: JobType, existing_id: String },
}

/// The result of asking the scheduler to create a job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreateOutcome {
    Created(Job),
    NeedsConfirmation(ConfirmationRequired),
}

/// A request to create a job, as it arrives from the UI.
///
/// Note what is absent: a target instant. The web view never resolves a trigger
/// (task 11.2); it says "22:30 in this timezone" and the scheduler resolves it with
/// the same [`TriggerResolver`] the shared vectors pin down.
#[derive(Debug, Clone)]
pub struct CreateJobRequest {
    pub job_type: JobType,
    pub trigger: TriggerSpec,
    pub timezone: String,
}

pub struct JobScheduler {
    repository: Arc<dyn JobRepository>,
    keep_awake: Arc<KeepAwakeCoordinator>,
    gate: Arc<PowerOffGate>,
    clock: Arc<dyn SchedulerClock>,
    observer: Arc<dyn SchedulerObserver>,
    /// Serialises ticks. A tick can block for a minute inside the grace period, and a
    /// second tick entering meanwhile could start a second countdown for the same job.
    tick_lock: Mutex<()>,
    tick_count: AtomicU64,
}

impl JobScheduler {
    pub fn new(
        repository: Arc<dyn JobRepository>,
        keep_awake: Arc<KeepAwakeCoordinator>,
        gate: Arc<PowerOffGate>,
        clock: Arc<dyn SchedulerClock>,
        observer: Arc<dyn SchedulerObserver>,
    ) -> Self {
        Self {
            repository,
            keep_awake,
            gate,
            clock,
            observer,
            tick_lock: Mutex::new(()),
            tick_count: AtomicU64::new(0),
        }
    }

    /// Creates a job, resolving its target instant here rather than trusting a caller.
    ///
    /// Returns [`CreateOutcome::NeedsConfirmation`] rather than replacing an existing
    /// active job of the same type. `confirm_replace` is how the caller says yes.
    pub fn create_job(&self, request: &CreateJobRequest) -> AppResult<CreateOutcome> {
        let (job, existing) = self.prepare_job(request)?;

        if let Some(existing) = existing {
            return Ok(CreateOutcome::NeedsConfirmation(
                ConfirmationRequired::ReplaceActiveJob {
                    job_type: request.job_type,
                    existing_id: existing.id,
                },
            ));
        }

        self.repository.insert(&job)?;
        self.sync_keep_awake()?;
        Ok(CreateOutcome::Created(job))
    }

    /// Creates a job, cancelling any existing active job of the same type.
    ///
    /// Separate entry point on purpose: replacing is a different act from creating,
    /// and a boolean parameter on one function makes the destructive case reachable
    /// by a default value.
    pub fn create_job_replacing_active(&self, request: &CreateJobRequest) -> AppResult<Job> {
        let (job, existing) = self.prepare_job(request)?;

        if existing.is_some() {
            self.repository
                .replace_active_job_of_type(request.job_type, &job)?;
        } else {
            self.repository.insert(&job)?;
        }

        self.sync_keep_awake()?;
        Ok(job)
    }

    /// Validates, resolves, and builds the job; also reports any active job it would
    /// displace. Does not write anything.
    fn prepare_job(&self, request: &CreateJobRequest) -> AppResult<(Job, Option<Job>)> {
        TriggerResolver::validate(&request.trigger, request.job_type)?;

        let timezone: Tz = request
            .timezone
            .parse()
            .map_err(|_| AppError::Validation {
                message: format!("unknown timezone: {}", request.timezone),
            })?;

        let now = self.clock.now_utc();
        let target = TriggerResolver::resolve(&request.trigger, timezone, now)?;

        let job = Job {
            id: new_job_id(),
            job_type: request.job_type,
            trigger: request.trigger,
            status: JobStatus::Active,
            target_instant_utc: target,
            created_at_utc: now,
            updated_at_utc: now,
            timezone: request.timezone.clone(),
            failure_message: None,
        };

        let existing = self
            .repository
            .list_occupying_active_slot()?
            .into_iter()
            .find(|candidate| candidate.job_type == request.job_type);

        Ok((job, existing))
    }

    /// One pass over every active job.
    ///
    /// Called on a fixed interval by [`Self::spawn`], and directly on startup and on
    /// resume from sleep. Idempotent: running it twice in the same second changes
    /// nothing the second time, which is what makes it safe to call it from all three
    /// places rather than having a separate startup path that could drift.
    pub fn tick(&self) -> AppResult<()> {
        // Held for the whole pass, including any grace period inside it.
        let _guard = self.tick_lock.lock().map_err(|_| AppError::Storage {
            message: "scheduler lock poisoned".to_string(),
        })?;

        self.tick_count.fetch_add(1, Ordering::SeqCst);
        let now = self.clock.now_utc();

        for job in self.repository.list_occupying_active_slot()? {
            // Paused jobs hold the slot but must not fire.
            if job.status != JobStatus::Active {
                continue;
            }
            self.evaluate(&job, now)?;
        }

        self.sync_keep_awake()
    }

    /// Applies the domain reconciler's verdict for one job.
    fn evaluate(&self, job: &Job, now: DateTime<Utc>) -> AppResult<()> {
        let outcome = TriggerResolver::reconcile(job.job_type, job.target_instant_utc, now);

        match outcome {
            ReconcileOutcome::StillPending => Ok(()),

            ReconcileOutcome::Completed => {
                self.repository
                    .update_status(&job.id, JobStatus::Completed, None)?;
                self.observer.job_completed(job);
                Ok(())
            }

            ReconcileOutcome::ProceedToGracePeriod => self.run_grace_period(job),

            ReconcileOutcome::Overdue => {
                // Task 10.5. The machine has been running for longer than the
                // tolerance since the target passed, so the user is plainly using it.
                // Shutting down now would destroy work. The executor is not invoked.
                let minutes_late = job
                    .remaining(now)
                    .map(|remaining| -remaining.num_minutes())
                    .unwrap_or(POWER_OFF_OVERTOLERANCE_MINUTES);

                self.repository.update_status(
                    &job.id,
                    JobStatus::Overdue,
                    Some(&format!(
                        "Missed by {minutes_late} minutes while the app was not running, \
                         so the power-off was not carried out."
                    )),
                )?;
                self.observer.job_overdue(job, minutes_late);
                Ok(())
            }
        }
    }

    /// Runs the countdown and records what happened.
    ///
    /// The only caller of [`PowerOffGate::run`] outside the gate's own tests, and it
    /// is reached only from a `ProceedToGracePeriod` verdict.
    fn run_grace_period(&self, job: &Job) -> AppResult<()> {
        self.observer
            .grace_period_started(job, crate::application::grace_period::GRACE_PERIOD_SECONDS);

        match self.gate.run(&job.id) {
            GraceOutcome::PoweredOff => {
                // Recorded before the machine goes down so the next launch does not
                // see a still-active job and shut down again.
                self.repository
                    .update_status(&job.id, JobStatus::Completed, None)?;
                Ok(())
            }

            GraceOutcome::Cancelled => {
                self.repository
                    .update_status(&job.id, JobStatus::Cancelled, None)?;
                self.observer.grace_period_cancelled(job);
                Ok(())
            }

            GraceOutcome::Failed(error) => {
                // Task 10.10: persisted and surfaced, never swallowed. The user must
                // not be left believing the machine will shut down when it will not.
                self.repository.update_status(
                    &job.id,
                    JobStatus::Failed,
                    Some(&error.user_message()),
                )?;
                self.observer.power_off_failed(job, &error);
                Ok(())
            }
        }
    }

    /// Cancels a countdown in progress. Task 11.6.
    pub fn cancel_grace_period(&self) {
        self.gate.cancel();
    }

    /// The countdown in progress, if any.
    pub fn grace_state(&self) -> Option<GraceState> {
        self.gate.state()
    }

    pub fn pause_job(&self, id: &str) -> AppResult<()> {
        self.require_job(id)?;
        self.repository.update_status(id, JobStatus::Paused, None)?;
        self.sync_keep_awake()
    }

    /// Resumes a paused job, re-resolving its target from now.
    ///
    /// A duration job paused for an hour should get its full remaining time back, not
    /// fire the instant it resumes. Re-resolving is the only way to get that, since
    /// the stored target is absolute.
    pub fn resume_job(&self, id: &str) -> AppResult<()> {
        let job = self.require_job(id)?;

        let timezone: Tz = job.timezone.parse().map_err(|_| AppError::Validation {
            message: format!("unknown timezone: {}", job.timezone),
        })?;

        let now = self.clock.now_utc();
        if let Some(target) = TriggerResolver::resolve(&job.trigger, timezone, now)? {
            self.repository.update_target(id, target)?;
        }

        self.repository.update_status(id, JobStatus::Active, None)?;
        self.sync_keep_awake()
    }

    pub fn cancel_job(&self, id: &str) -> AppResult<()> {
        self.require_job(id)?;
        self.repository
            .update_status(id, JobStatus::Cancelled, None)?;
        self.sync_keep_awake()
    }

    pub fn list_jobs(&self) -> AppResult<Vec<Job>> {
        self.repository.list_all()
    }

    pub fn find_job(&self, id: &str) -> AppResult<Option<Job>> {
        self.repository.find(id)
    }

    /// Whether any job still holds an active slot. The tray asks this before quitting.
    pub fn has_active_job(&self) -> AppResult<bool> {
        Ok(!self.repository.list_occupying_active_slot()?.is_empty())
    }

    /// Re-resolves every absolute-time job after a timezone change (task 10.8).
    ///
    /// Duration jobs are deliberately left alone: "in 30 minutes" means the same
    /// instant regardless of what the clock on the wall is called. Only a wall-clock
    /// time moves.
    pub fn apply_timezone_change(&self, new_timezone: &str) -> AppResult<usize> {
        let timezone: Tz = new_timezone.parse().map_err(|_| AppError::Validation {
            message: format!("unknown timezone: {new_timezone}"),
        })?;

        let now = self.clock.now_utc();
        let mut changed = 0;

        for job in self.repository.list_occupying_active_slot()? {
            if !job.is_timezone_sensitive() {
                continue;
            }
            if let Some(target) = TriggerResolver::resolve(&job.trigger, timezone, now)? {
                if job.target_instant_utc != Some(target) {
                    self.repository.update_target(&job.id, target)?;
                    changed += 1;
                }
            }
        }

        Ok(changed)
    }

    /// Points the keep-awake assertion at exactly the set of jobs that want it.
    ///
    /// Called after every state change rather than at each transition. Section 8's
    /// coordinator takes the set of interested jobs as the source of truth, so one
    /// call here replaces an acquire or release scattered through create, pause,
    /// resume, cancel, and complete — five places to leak an assertion, reduced to
    /// none.
    fn sync_keep_awake(&self) -> AppResult<()> {
        let holders: Vec<String> = self
            .repository
            .list_occupying_active_slot()?
            .into_iter()
            .filter(|job| job.job_type == JobType::KeepAwake && job.status == JobStatus::Active)
            .map(|job| job.id)
            .collect();

        self.keep_awake.reconcile(holders)?;
        Ok(())
    }

    fn require_job(&self, id: &str) -> AppResult<Job> {
        self.repository.find(id)?.ok_or_else(|| AppError::Storage {
            message: format!("no job with id {id}"),
        })
    }

    pub fn tick_count(&self) -> u64 {
        self.tick_count.load(Ordering::SeqCst)
    }

    pub fn keep_awake(&self) -> &Arc<KeepAwakeCoordinator> {
        &self.keep_awake
    }
}

/// How often the tick runs.
///
/// One second, because the UI shows a per-second countdown and because a coarser
/// interval would make a power-off fire up to that interval late. The work per tick is
/// one indexed query over the handful of jobs holding an active slot.
pub const TICK_INTERVAL_SECONDS: u64 = 1;

/// Runs blocking work on Tauri's process-wide async runtime.
///
/// Tauri's macOS setup callback runs on AppKit's main thread, not inside a Tokio
/// runtime. Calling `tokio::task::spawn_blocking` there panics during launch with
/// "there is no reactor running". Tauri owns a runtime that is available from any
/// thread, so startup code must cross through this boundary instead.
fn spawn_blocking_on_app_runtime<F, R>(operation: F) -> tauri::async_runtime::JoinHandle<R>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    tauri::async_runtime::spawn_blocking(operation)
}

impl JobScheduler {
    /// Starts the periodic tick on Tauri's async runtime and returns immediately.
    ///
    /// The tick runs on a blocking thread, not an async task: it can sit inside a
    /// 60-second grace period, and occupying an async worker for a minute would stall
    /// unrelated work on the runtime.
    pub fn spawn(scheduler: Arc<Self>) -> tauri::async_runtime::JoinHandle<()> {
        spawn_blocking_on_app_runtime(move || loop {
            if let Err(error) = scheduler.tick() {
                // A failed tick must not end the loop. A transient storage error
                // would otherwise silently stop every future job.
                log::error!("scheduler tick failed: {error}");
            }
            std::thread::sleep(std::time::Duration::from_secs(TICK_INTERVAL_SECONDS));
        })
    }
}

/// A unique job id.
///
/// Timestamp plus a counter rather than a UUID dependency. Uniqueness only has to
/// hold within one installation's database, and a monotonic counter gives that even
/// when two jobs are created in the same microsecond.
fn new_job_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let sequence = COUNTER.fetch_add(1, Ordering::SeqCst);
    let micros = Utc::now().timestamp_micros();
    format!("job-{micros}-{sequence}")
}

#[cfg(test)]
mod runtime_boundary_tests {
    use super::spawn_blocking_on_app_runtime;

    #[test]
    fn startup_can_spawn_blocking_work_without_entering_a_tokio_runtime() {
        // This is the shape of Tauri's macOS setup callback: an ordinary OS thread,
        // with no Tokio runtime entered. A direct `tokio::task::spawn_blocking` here
        // panics, which made the release .app abort just after its window appeared.
        assert!(tokio::runtime::Handle::try_current().is_err());

        let handle = spawn_blocking_on_app_runtime(|| 42);
        let result = tauri::async_runtime::block_on(handle).expect("blocking task panicked");

        assert_eq!(result, 42);
    }
}

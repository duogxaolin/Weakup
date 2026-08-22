//! Scheduler tests.
//!
//! Every one of these binds [`FakePowerOffExecutor`]. A real executor here would shut
//! down the machine running the suite.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Duration, NaiveDate, TimeZone, Utc};

use crate::application::fake_repository::FakeJobRepository;
use crate::application::grace_period::{
    gate_with_script, InstantGraceClock, PowerOffGate, GRACE_PERIOD_SECONDS,
    REMOTE_GRACE_PERIOD_SECONDS,
};
use crate::application::scheduler::{
    ConfirmationRequired, CreateJobRequest, CreateOutcome, JobScheduler, SchedulerClock,
    SchedulerObserver,
};
use crate::core::AppError;
use crate::domain::{Job, JobOrigin, JobStatus, JobType, TriggerSpec};
use crate::platform::keep_awake::{FakeKeepAwakeController, KeepAwakeCoordinator};
use crate::platform::power_off::FakePowerOffExecutor;

/// A clock the test moves by hand.
struct TestClock {
    now: Mutex<DateTime<Utc>>,
}

impl TestClock {
    fn at(now: DateTime<Utc>) -> Self {
        Self {
            now: Mutex::new(now),
        }
    }

    fn advance(&self, by: Duration) {
        let mut now = self.now.lock().unwrap();
        *now += by;
    }
}

impl SchedulerClock for TestClock {
    fn now_utc(&self) -> DateTime<Utc> {
        *self.now.lock().unwrap()
    }
}

#[derive(Default)]
struct RecordingObserver {
    completed: AtomicUsize,
    grace_started: AtomicUsize,
    /// The lengths announced to the user, in order. Recorded rather than discarded so a
    /// test can check the number shown matches the number actually waited — two constants
    /// would let the UI draw a one-minute countdown over a five-minute wait.
    grace_lengths: Mutex<Vec<u64>>,
    grace_cancelled: AtomicUsize,
    failures: Mutex<Vec<String>>,
    overdue: Mutex<Vec<i64>>,
}

impl SchedulerObserver for RecordingObserver {
    fn job_completed(&self, _job: &Job) {
        self.completed.fetch_add(1, Ordering::SeqCst);
    }
    fn grace_period_started(&self, _job: &Job, seconds: u64) {
        self.grace_started.fetch_add(1, Ordering::SeqCst);
        self.grace_lengths.lock().unwrap().push(seconds);
    }
    fn grace_period_cancelled(&self, _job: &Job) {
        self.grace_cancelled.fetch_add(1, Ordering::SeqCst);
    }
    fn power_off_failed(&self, _job: &Job, error: &AppError) {
        self.failures.lock().unwrap().push(error.to_string());
    }
    fn job_overdue(&self, _job: &Job, minutes_late: i64) {
        self.overdue.lock().unwrap().push(minutes_late);
    }
}

fn utc(y: i32, m: u32, d: u32, h: u32, min: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(y, m, d, h, min, 0).unwrap()
}

const NOW: fn() -> DateTime<Utc> = || utc(2026, 7, 30, 12, 0);

/// Everything a test needs, wired together with fakes throughout.
struct Harness {
    scheduler: Arc<JobScheduler>,
    repository: Arc<FakeJobRepository>,
    executor: Arc<FakePowerOffExecutor>,
    keep_awake_controller: Arc<FakeKeepAwakeController>,
    observer: Arc<RecordingObserver>,
    clock: Arc<TestClock>,
    /// Reads the grace clock's tick count. A closure rather than the clock itself so a
    /// test can swap in a scripted clock without the harness knowing which it holds.
    grace_ticks: Box<dyn Fn() -> u64>,
}

impl Harness {
    fn new() -> Self {
        Self::with(FakeJobRepository::new(), FakePowerOffExecutor::succeeding())
    }

    fn with_jobs(jobs: Vec<Job>) -> Self {
        Self::with(
            FakeJobRepository::with_jobs(jobs),
            FakePowerOffExecutor::succeeding(),
        )
    }

    fn with(repository: FakeJobRepository, executor: FakePowerOffExecutor) -> Self {
        let executor = Arc::new(executor);
        let grace_clock = Arc::new(InstantGraceClock::default());
        let gate = Arc::new(PowerOffGate::new(executor.clone(), grace_clock.clone()));
        let ticks = {
            let grace_clock = grace_clock.clone();
            Box::new(move || grace_clock.tick_count())
        };
        Self::assemble(repository, executor, gate, ticks)
    }

    /// A harness whose countdown runs `script` on each second, from inside the
    /// countdown. Lets a test cancel at an exact moment without racing a thread.
    fn with_grace_script(
        jobs: Vec<Job>,
        script: impl Fn(&PowerOffGate, u64) + Send + Sync + 'static,
    ) -> Self {
        let executor = Arc::new(FakePowerOffExecutor::succeeding());
        let (gate, grace_clock) = gate_with_script(executor.clone(), script);
        let ticks = {
            let grace_clock = grace_clock.clone();
            Box::new(move || grace_clock.tick_count())
        };
        Self::assemble(FakeJobRepository::with_jobs(jobs), executor, gate, ticks)
    }

    fn assemble(
        repository: FakeJobRepository,
        executor: Arc<FakePowerOffExecutor>,
        gate: Arc<PowerOffGate>,
        grace_ticks: Box<dyn Fn() -> u64>,
    ) -> Self {
        let repository = Arc::new(repository);
        let keep_awake_controller = Arc::new(FakeKeepAwakeController::working());
        let keep_awake = Arc::new(KeepAwakeCoordinator::new(keep_awake_controller.clone()));
        let clock = Arc::new(TestClock::at(NOW()));
        let observer = Arc::new(RecordingObserver::default());

        let scheduler = Arc::new(JobScheduler::new(
            repository.clone(),
            keep_awake,
            gate,
            clock.clone(),
            observer.clone(),
        ));

        Self {
            scheduler,
            repository,
            executor,
            keep_awake_controller,
            observer,
            clock,
            grace_ticks,
        }
    }

    fn grace_ticks(&self) -> u64 {
        (self.grace_ticks)()
    }
}

/// A job as it would have been stored on a previous run.
fn stored_job(id: &str, job_type: JobType, trigger: TriggerSpec, target: Option<DateTime<Utc>>) -> Job {
    Job {
        id: id.to_string(),
        job_type,
        trigger,
        status: JobStatus::Active,
        target_instant_utc: target,
        created_at_utc: NOW(),
        updated_at_utc: NOW(),
        timezone: "Asia/Ho_Chi_Minh".to_string(),
        failure_message: None,
        origin: JobOrigin::Local,
    }
}

/// The same, but created by an authorized remote command rather than at the machine.
fn remote_job(id: &str, job_type: JobType, trigger: TriggerSpec, target: Option<DateTime<Utc>>) -> Job {
    Job {
        origin: JobOrigin::Remote,
        ..stored_job(id, job_type, trigger, target)
    }
}

fn request(job_type: JobType, trigger: TriggerSpec) -> CreateJobRequest {
    CreateJobRequest {
        job_type,
        trigger,
        timezone: "Asia/Ho_Chi_Minh".to_string(),
    }
}

// ---------------------------------------------------------------------------
// The safety guarantees. These are the tests that must never be weakened.
// ---------------------------------------------------------------------------

#[test]
fn a_power_off_never_happens_without_the_full_grace_period() {
    // Task 10.3. The countdown is not a separate step the scheduler could forget: the
    // gate owns the executor, so this asserts the whole route.
    let harness = Harness::with_jobs(vec![stored_job(
        "power-1",
        JobType::PowerOff,
        TriggerSpec::Duration { minutes: 30 },
        Some(NOW() + Duration::minutes(30)),
    )]);

    harness.clock.advance(Duration::minutes(30));
    harness.scheduler.tick().unwrap();

    assert_eq!(harness.executor.call_count(), 1);
    assert_eq!(
        harness.grace_ticks(),
        GRACE_PERIOD_SECONDS,
        "the executor was reached, so a full countdown must have elapsed first"
    );
    assert_eq!(harness.observer.grace_started.load(Ordering::SeqCst), 1);
}

#[test]
fn a_power_off_overdue_beyond_tolerance_does_not_shut_the_machine_down() {
    // Task 10.5. The machine has clearly been in use since the target passed.
    // Shutting down now would destroy the user's work.
    let harness = Harness::with_jobs(vec![stored_job(
        "power-1",
        JobType::PowerOff,
        TriggerSpec::at_time(2, 0),
        Some(NOW() - Duration::minutes(90)),
    )]);

    harness.scheduler.tick().unwrap();

    harness.executor.assert_never_called();
    assert_eq!(harness.grace_ticks(), 0);
    assert_eq!(
        harness.repository.status_of("power-1"),
        Some(JobStatus::Overdue)
    );
    assert_eq!(harness.observer.overdue.lock().unwrap().len(), 1);

    let message = harness
        .repository
        .failure_message_of("power-1")
        .expect("the user is told why nothing happened");
    assert!(message.contains("not carried out"), "got: {message}");
}

#[test]
fn a_power_off_overdue_within_tolerance_proceeds_through_the_countdown() {
    // Task 10.6, the other side of the tolerance. Closing the lid for five minutes
    // must not silently drop a scheduled shutdown.
    let harness = Harness::with_jobs(vec![stored_job(
        "power-1",
        JobType::PowerOff,
        TriggerSpec::at_time(2, 0),
        Some(NOW() - Duration::minutes(5)),
    )]);

    harness.scheduler.tick().unwrap();

    assert_eq!(harness.executor.call_count(), 1);
    assert_eq!(harness.grace_ticks(), GRACE_PERIOD_SECONDS);
    assert_eq!(
        harness.repository.status_of("power-1"),
        Some(JobStatus::Completed)
    );
}

#[test]
fn a_remotely_created_power_off_gets_the_longer_countdown_through_the_real_path() {
    // The gate's own tests prove it reads the origin; this proves the scheduler hands it
    // a job to read rather than a bare id, so the remote length survives the whole route
    // from stored job to executor.
    let harness = Harness::with_jobs(vec![remote_job(
        "power-1",
        JobType::PowerOff,
        TriggerSpec::at_time(2, 0),
        Some(NOW() - Duration::minutes(5)),
    )]);

    harness.scheduler.tick().unwrap();

    assert_eq!(harness.grace_ticks(), REMOTE_GRACE_PERIOD_SECONDS);
    assert_eq!(harness.executor.call_count(), 1);
    // What the user was told is what they got. A mismatch here means a countdown
    // displayed at one length and waited at another.
    assert_eq!(
        *harness.observer.grace_lengths.lock().unwrap(),
        vec![REMOTE_GRACE_PERIOD_SECONDS]
    );
}

#[test]
fn a_locally_scheduled_power_off_still_announces_sixty_seconds() {
    // The unchanged half, asserted alongside so a change that gave every job the longer
    // countdown could not pass by making only the remote test greener.
    let harness = Harness::with_jobs(vec![stored_job(
        "power-1",
        JobType::PowerOff,
        TriggerSpec::at_time(2, 0),
        Some(NOW() - Duration::minutes(5)),
    )]);

    harness.scheduler.tick().unwrap();

    assert_eq!(
        *harness.observer.grace_lengths.lock().unwrap(),
        vec![GRACE_PERIOD_SECONDS]
    );
}

#[test]
fn cancelling_the_countdown_leaves_the_machine_running() {
    // Cancels from inside the countdown's tenth second, which is exactly what the
    // user pressing the cancel button does.
    let harness = Harness::with_grace_script(
        vec![stored_job(
            "power-1",
            JobType::PowerOff,
            TriggerSpec::Duration { minutes: 1 },
            Some(NOW() - Duration::seconds(1)),
        )],
        |gate, elapsed| {
            if elapsed == 10 {
                gate.cancel();
            }
        },
    );

    harness.scheduler.tick().unwrap();

    harness.executor.assert_never_called();
    assert!(
        harness.grace_ticks() < GRACE_PERIOD_SECONDS,
        "the countdown stopped early"
    );
    assert_eq!(
        harness.repository.status_of("power-1"),
        Some(JobStatus::Cancelled)
    );
    assert_eq!(harness.observer.grace_cancelled.load(Ordering::SeqCst), 1);
}

#[test]
fn a_paused_power_off_job_never_fires() {
    let mut job = stored_job(
        "power-1",
        JobType::PowerOff,
        TriggerSpec::Duration { minutes: 30 },
        Some(NOW() - Duration::minutes(1)),
    );
    job.status = JobStatus::Paused;
    let harness = Harness::with_jobs(vec![job]);

    harness.scheduler.tick().unwrap();

    harness.executor.assert_never_called();
    assert_eq!(
        harness.repository.status_of("power-1"),
        Some(JobStatus::Paused),
        "a paused job holds its slot without firing"
    );
}

#[test]
fn a_failed_power_off_is_persisted_and_announced() {
    // Task 10.10.
    let harness = Harness::with(
        FakeJobRepository::with_jobs(vec![stored_job(
            "power-1",
            JobType::PowerOff,
            TriggerSpec::Duration { minutes: 1 },
            Some(NOW() - Duration::seconds(1)),
        )]),
        FakePowerOffExecutor::failing(AppError::PowerOffPrivilegeDenied {
            detail: Some("no privilege".to_string()),
        }),
    );

    harness.scheduler.tick().unwrap();

    assert_eq!(
        harness.repository.status_of("power-1"),
        Some(JobStatus::Failed)
    );
    assert!(harness.repository.failure_message_of("power-1").is_some());
    assert_eq!(harness.observer.failures.lock().unwrap().len(), 1);
}

// ---------------------------------------------------------------------------
// Recomputing from the absolute target (task 10.7)
// ---------------------------------------------------------------------------

#[test]
fn a_target_reached_during_sleep_fires_on_the_next_tick() {
    // No timer survives suspend. Reading the absolute target on each tick means the
    // scheduler needs no wake notification to notice.
    let harness = Harness::with_jobs(vec![stored_job(
        "wake-1",
        JobType::KeepAwake,
        TriggerSpec::Duration { minutes: 120 },
        Some(NOW() + Duration::minutes(120)),
    )]);

    harness.scheduler.tick().unwrap();
    assert_eq!(
        harness.repository.status_of("wake-1"),
        Some(JobStatus::Active)
    );

    // The machine was asleep for three hours; no tick ran in between.
    harness.clock.advance(Duration::hours(3));
    harness.scheduler.tick().unwrap();

    assert_eq!(
        harness.repository.status_of("wake-1"),
        Some(JobStatus::Completed)
    );
    harness.keep_awake_controller.assert_not_held();
}

#[test]
fn ticking_repeatedly_before_the_target_changes_nothing() {
    let harness = Harness::with_jobs(vec![stored_job(
        "wake-1",
        JobType::KeepAwake,
        TriggerSpec::Duration { minutes: 60 },
        Some(NOW() + Duration::minutes(60)),
    )]);

    for _ in 0..5 {
        harness.scheduler.tick().unwrap();
    }

    assert_eq!(
        harness.repository.status_of("wake-1"),
        Some(JobStatus::Active)
    );
    assert_eq!(
        harness.keep_awake_controller.acquire_count(),
        1,
        "repeated ticks must not re-acquire the assertion"
    );
}

#[test]
fn an_indefinite_keep_awake_job_never_completes_on_its_own() {
    let harness = Harness::with_jobs(vec![stored_job(
        "wake-1",
        JobType::KeepAwake,
        TriggerSpec::Indefinite,
        None,
    )]);

    harness.clock.advance(Duration::days(30));
    harness.scheduler.tick().unwrap();

    assert_eq!(
        harness.repository.status_of("wake-1"),
        Some(JobStatus::Active)
    );
    assert!(harness.scheduler.keep_awake().is_asserting());
}

// ---------------------------------------------------------------------------
// Keep-awake follows the set of active jobs (section 8's coordinator)
// ---------------------------------------------------------------------------

#[test]
fn creating_a_keep_awake_job_takes_the_assertion() {
    let harness = Harness::new();

    harness
        .scheduler
        .create_job(&request(JobType::KeepAwake, TriggerSpec::Indefinite))
        .unwrap();

    assert!(harness.scheduler.keep_awake().is_asserting());
    assert_eq!(harness.keep_awake_controller.acquire_count(), 1);
}

#[test]
fn cancelling_the_last_keep_awake_job_releases_the_assertion() {
    let harness = Harness::new();
    let created = harness
        .scheduler
        .create_job(&request(JobType::KeepAwake, TriggerSpec::Indefinite))
        .unwrap();
    let id = match created {
        CreateOutcome::Created(job) => job.id,
        other => panic!("expected a created job, got {other:?}"),
    };

    harness.scheduler.cancel_job(&id).unwrap();

    harness.keep_awake_controller.assert_not_held();
}

#[test]
fn pausing_a_keep_awake_job_releases_the_assertion_and_resuming_retakes_it() {
    let harness = Harness::with_jobs(vec![stored_job(
        "wake-1",
        JobType::KeepAwake,
        TriggerSpec::Indefinite,
        None,
    )]);
    harness.scheduler.tick().unwrap();
    assert!(harness.scheduler.keep_awake().is_asserting());

    harness.scheduler.pause_job("wake-1").unwrap();
    harness.keep_awake_controller.assert_not_held();

    harness.scheduler.resume_job("wake-1").unwrap();
    assert!(harness.scheduler.keep_awake().is_asserting());
}

#[test]
fn a_power_off_job_does_not_hold_the_display_awake() {
    // Keeping the screen on until a shutdown is a different feature the user did not
    // ask for, and it would drain a battery.
    let harness = Harness::new();

    harness
        .scheduler
        .create_job(&request(
            JobType::PowerOff,
            TriggerSpec::Duration { minutes: 30 },
        ))
        .unwrap();

    harness.keep_awake_controller.assert_not_held();
}

#[test]
fn a_keep_awake_job_left_over_from_a_previous_run_is_reasserted_on_the_first_tick() {
    // The IOKit assertion died with the previous process. Nothing in the database
    // changed, so the first tick has to notice the job and take a fresh assertion.
    let harness = Harness::with_jobs(vec![stored_job(
        "wake-1",
        JobType::KeepAwake,
        TriggerSpec::Indefinite,
        None,
    )]);

    assert!(!harness.scheduler.keep_awake().is_asserting());
    harness.scheduler.tick().unwrap();
    assert!(harness.scheduler.keep_awake().is_asserting());
}

// ---------------------------------------------------------------------------
// One active job per type, replaced only on confirmation (task 10.9)
// ---------------------------------------------------------------------------

#[test]
fn a_second_job_of_the_same_type_asks_before_replacing() {
    let harness = Harness::new();
    harness
        .scheduler
        .create_job(&request(
            JobType::PowerOff,
            TriggerSpec::Duration { minutes: 30 },
        ))
        .unwrap();

    let outcome = harness
        .scheduler
        .create_job(&request(
            JobType::PowerOff,
            TriggerSpec::Duration { minutes: 60 },
        ))
        .unwrap();

    match outcome {
        CreateOutcome::NeedsConfirmation(ConfirmationRequired::ReplaceActiveJob {
            job_type,
            ..
        }) => assert_eq!(job_type, JobType::PowerOff),
        other => panic!("expected a confirmation request, got {other:?}"),
    }

    assert_eq!(
        harness.repository.snapshot().len(),
        1,
        "nothing may be written before the user confirms"
    );
}

#[test]
fn confirming_the_replacement_cancels_the_old_job() {
    let harness = Harness::new();
    harness
        .scheduler
        .create_job(&request(
            JobType::PowerOff,
            TriggerSpec::Duration { minutes: 30 },
        ))
        .unwrap();

    harness
        .scheduler
        .create_job_replacing_active(&request(
            JobType::PowerOff,
            TriggerSpec::Duration { minutes: 60 },
        ))
        .unwrap();

    let active: Vec<_> = harness
        .repository
        .snapshot()
        .into_iter()
        .filter(|job| job.occupies_active_slot())
        .collect();
    assert_eq!(active.len(), 1);
    assert_eq!(
        active[0].trigger,
        TriggerSpec::Duration { minutes: 60 },
        "the new job is the one left active"
    );
}

#[test]
fn the_two_job_types_do_not_compete_for_the_same_slot() {
    // The whole point of the app is being able to run both at once.
    let harness = Harness::new();

    harness
        .scheduler
        .create_job(&request(JobType::KeepAwake, TriggerSpec::Indefinite))
        .unwrap();
    let second = harness
        .scheduler
        .create_job(&request(
            JobType::PowerOff,
            TriggerSpec::Duration { minutes: 120 },
        ))
        .unwrap();

    assert!(matches!(second, CreateOutcome::Created(_)));
    assert!(harness.scheduler.has_active_job().unwrap());
    assert_eq!(harness.repository.snapshot().len(), 2);
}

// ---------------------------------------------------------------------------
// Resolution happens here, never in the web view (task 11.2)
// ---------------------------------------------------------------------------

#[test]
fn the_scheduler_resolves_the_target_instant_itself() {
    let harness = Harness::new();

    let created = harness
        .scheduler
        .create_job(&request(
            JobType::PowerOff,
            TriggerSpec::Duration { minutes: 45 },
        ))
        .unwrap();

    match created {
        CreateOutcome::Created(job) => assert_eq!(
            job.target_instant_utc,
            Some(NOW() + Duration::minutes(45)),
            "the caller supplied no instant; this one came from the resolver"
        ),
        other => panic!("expected a created job, got {other:?}"),
    }
}

#[test]
fn an_invalid_trigger_is_rejected_before_anything_is_written() {
    let harness = Harness::new();

    // Indefinite power-off is meaningless: it would never fire.
    let error = harness
        .scheduler
        .create_job(&request(JobType::PowerOff, TriggerSpec::Indefinite))
        .expect_err("an indefinite power-off must be refused");

    assert!(matches!(error, AppError::Validation { .. }));
    assert!(harness.repository.snapshot().is_empty());
}

#[test]
fn an_unknown_timezone_is_rejected_rather_than_defaulted_to_utc() {
    // Silently falling back to UTC would fire a 22:00 shutdown at the wrong hour.
    let harness = Harness::new();

    let error = harness
        .scheduler
        .create_job(&CreateJobRequest {
            job_type: JobType::PowerOff,
            trigger: TriggerSpec::at_time(22, 30),
            timezone: "Mars/Olympus_Mons".to_string(),
        })
        .expect_err("an unknown timezone must be refused");

    assert!(matches!(error, AppError::Validation { .. }));
    assert!(harness.repository.snapshot().is_empty());
}

// ---------------------------------------------------------------------------
// Timezone changes (task 10.8)
// ---------------------------------------------------------------------------

#[test]
fn a_timezone_change_moves_an_absolute_time_job() {
    let harness = Harness::new();
    let created = harness
        .scheduler
        .create_job(&request(
            JobType::PowerOff,
            TriggerSpec::at_time(22, 30),
        ))
        .unwrap();
    let (id, before) = match created {
        CreateOutcome::Created(job) => (job.id, job.target_instant_utc),
        other => panic!("expected a created job, got {other:?}"),
    };

    let changed = harness
        .scheduler
        .apply_timezone_change("Europe/London")
        .unwrap();

    assert_eq!(changed, 1);
    let after = harness.repository.find_target(&id);
    assert!(after.is_some(), "the job still has a target");
    assert_ne!(
        after, before,
        "22:30 in London is a different instant from 22:30 in Ho Chi Minh City"
    );
}

#[test]
fn a_timezone_change_leaves_a_duration_job_alone() {
    // "In 30 minutes" is 30 minutes from now no matter what the zone is called.
    // Re-resolving it on every timezone change would keep pushing the target away.
    let harness = Harness::new();
    let created = harness
        .scheduler
        .create_job(&request(
            JobType::PowerOff,
            TriggerSpec::Duration { minutes: 30 },
        ))
        .unwrap();
    let (id, before) = match created {
        CreateOutcome::Created(job) => (job.id, job.target_instant_utc),
        other => panic!("expected a created job, got {other:?}"),
    };

    harness.clock.advance(Duration::minutes(10));
    let changed = harness
        .scheduler
        .apply_timezone_change("Europe/London")
        .unwrap();

    assert_eq!(changed, 0);
    assert_eq!(harness.repository.find_target(&id), before);
}

#[test]
fn a_timezone_change_that_strands_a_dated_job_still_moves_the_others() {
    // A zone change can push a dated target into the past, which resolution refuses.
    // That refusal must not abort the sweep: the remaining jobs would silently keep
    // targets computed for the old zone, which is the failure the re-resolution exists
    // to prevent in the first place.
    //
    // Seeded directly rather than via create_job, because creating a job whose target
    // is already in the past is exactly what the resolver refuses to do.
    let stranded = stored_job(
        "dated-1",
        JobType::PowerOff,
        TriggerSpec::on_date(NaiveDate::from_ymd_opt(2026, 7, 30).unwrap(), 12, 30),
        Some(NOW() + Duration::minutes(30)),
    );
    let mut movable = stored_job(
        "undated-1",
        JobType::KeepAwake,
        TriggerSpec::at_time(22, 30),
        Some(NOW() + Duration::hours(9)),
    );
    movable.job_type = JobType::KeepAwake;

    let harness = Harness::with_jobs(vec![stranded.clone(), movable.clone()]);

    // 12:30 on 30 July in Ho Chi Minh City is 05:30Z, an hour and a half before NOW,
    // so re-resolving the dated job against any zone now fails.
    harness.clock.advance(Duration::hours(8));

    let changed = harness
        .scheduler
        .apply_timezone_change("Europe/London")
        .expect("the sweep must not fail because one job could not be re-resolved");

    assert_eq!(
        harness.repository.find_target("dated-1"),
        stranded.target_instant_utc,
        "the stranded job's target must be left exactly as it was, for reconcile to classify"
    );
    assert_eq!(changed, 1, "the other timezone-sensitive job still moved");
    assert_ne!(
        harness.repository.find_target("undated-1"),
        movable.target_instant_utc,
        "22:30 in London is a different instant from 22:30 in Ho Chi Minh City"
    );
}

// ---------------------------------------------------------------------------
// Pause and resume
// ---------------------------------------------------------------------------

#[test]
fn resuming_a_duration_job_gives_back_its_full_remaining_time() {
    // Otherwise a job paused past its target fires the moment it resumes, which is
    // the opposite of what pausing was for.
    let harness = Harness::new();
    let created = harness
        .scheduler
        .create_job(&request(
            JobType::PowerOff,
            TriggerSpec::Duration { minutes: 30 },
        ))
        .unwrap();
    let id = match created {
        CreateOutcome::Created(job) => job.id,
        other => panic!("expected a created job, got {other:?}"),
    };

    harness.scheduler.pause_job(&id).unwrap();
    harness.clock.advance(Duration::hours(5));
    harness.scheduler.resume_job(&id).unwrap();

    let resumed_at = NOW() + Duration::hours(5);
    assert_eq!(
        harness.repository.find_target(&id),
        Some(resumed_at + Duration::minutes(30))
    );

    harness.scheduler.tick().unwrap();
    harness.executor.assert_never_called();
}

#[test]
fn resuming_a_dated_job_whose_instant_has_passed_fails_and_leaves_it_paused() {
    // Resuming re-resolves from the current clock, and a dated trigger cannot roll
    // forward. Rescheduling an irreversible power-off to a later instant the user
    // never chose would be worse than refusing, so the refusal must also not
    // half-apply: the job stays Paused rather than becoming Active with a stale target.
    let harness = Harness::new();
    let created = harness
        .scheduler
        .create_job(&request(
            JobType::PowerOff,
            TriggerSpec::on_date(NaiveDate::from_ymd_opt(2026, 7, 31).unwrap(), 22, 30),
        ))
        .unwrap();
    let (id, target_before) = match created {
        CreateOutcome::Created(job) => (job.id, job.target_instant_utc),
        other => panic!("expected a created job, got {other:?}"),
    };

    harness.scheduler.pause_job(&id).unwrap();
    // Well past 22:30 on the 31st.
    harness.clock.advance(Duration::days(3));

    let error = harness
        .scheduler
        .resume_job(&id)
        .expect_err("a passed dated job must not silently reschedule");

    assert!(matches!(error, AppError::Validation { .. }));
    assert!(
        error.user_message().contains("already passed"),
        "the user is told why: {}",
        error.user_message()
    );
    assert_eq!(
        harness.repository.status_of(&id),
        Some(JobStatus::Paused),
        "a failed resume must not leave the job Active"
    );
    assert_eq!(
        harness.repository.find_target(&id),
        target_before,
        "the stored target must not have moved"
    );
}

#[test]
fn resuming_an_undated_absolute_job_still_rolls_forward() {
    // The other half of the pair: without a date the alarm semantic is unchanged, so
    // a resume after the time has passed schedules the next occurrence as before.
    let harness = Harness::new();
    let created = harness
        .scheduler
        .create_job(&request(JobType::PowerOff, TriggerSpec::at_time(22, 30)))
        .unwrap();
    let (id, before) = match created {
        CreateOutcome::Created(job) => (job.id, job.target_instant_utc),
        other => panic!("expected a created job, got {other:?}"),
    };

    harness.scheduler.pause_job(&id).unwrap();
    harness.clock.advance(Duration::days(3));
    harness.scheduler.resume_job(&id).unwrap();

    assert_eq!(harness.repository.status_of(&id), Some(JobStatus::Active));
    assert_ne!(
        harness.repository.find_target(&id),
        before,
        "the target should have rolled forward to the next 22:30"
    );
}

#[test]
fn acting_on_a_job_that_does_not_exist_is_an_error_not_a_silent_success() {
    let harness = Harness::new();

    assert!(harness.scheduler.pause_job("nope").is_err());
    assert!(harness.scheduler.resume_job("nope").is_err());
    assert!(harness.scheduler.cancel_job("nope").is_err());
}

// ---------------------------------------------------------------------------
// Failure handling
// ---------------------------------------------------------------------------

#[test]
fn a_storage_failure_while_creating_is_reported() {
    let harness = Harness::new();
    harness.repository.fail_writes(AppError::Storage {
        message: "disk full".to_string(),
    });

    let error = harness
        .scheduler
        .create_job(&request(JobType::KeepAwake, TriggerSpec::Indefinite))
        .expect_err("a failed write must surface");

    assert!(matches!(error, AppError::Storage { .. }));
}

#[test]
fn has_active_job_is_false_once_everything_is_finished() {
    // The tray uses this to decide whether quitting needs a warning. A false positive
    // would nag on every quit; a false negative would discard a live job silently.
    let harness = Harness::new();
    assert!(!harness.scheduler.has_active_job().unwrap());

    let created = harness
        .scheduler
        .create_job(&request(JobType::KeepAwake, TriggerSpec::Indefinite))
        .unwrap();
    assert!(harness.scheduler.has_active_job().unwrap());

    let id = match created {
        CreateOutcome::Created(job) => job.id,
        other => panic!("expected a created job, got {other:?}"),
    };
    harness.scheduler.cancel_job(&id).unwrap();
    assert!(!harness.scheduler.has_active_job().unwrap());
}

#[test]
fn concurrent_ticks_do_not_start_two_countdowns_for_one_job() {
    // Two overlapping ticks could each see the same due job. The second countdown
    // would power off after the user had already cancelled the first.
    let harness = Harness::with_jobs(vec![stored_job(
        "power-1",
        JobType::PowerOff,
        TriggerSpec::Duration { minutes: 1 },
        Some(NOW() - Duration::seconds(1)),
    )]);

    let handles: Vec<_> = (0..4)
        .map(|_| {
            let scheduler = harness.scheduler.clone();
            std::thread::spawn(move || scheduler.tick())
        })
        .collect();
    for handle in handles {
        handle.join().unwrap().unwrap();
    }

    assert_eq!(
        harness.executor.call_count(),
        1,
        "the job was due once, so it may power off at most once"
    );
}

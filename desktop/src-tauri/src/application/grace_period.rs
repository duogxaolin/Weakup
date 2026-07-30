//! The mandatory countdown before a power-off.
//!
//! Tasks 10.2 and 10.3 require that no path reaches a power-off without this, and
//! that it cannot be skipped. The way that is enforced here is structural rather
//! than by convention: [`PowerOffGate`] is the only thing in the crate that calls
//! `PowerOffExecutor::power_off`, and it will not do so until the countdown has
//! actually elapsed. There is no `force` parameter and no shorter duration to pass,
//! because an argument that can skip the wait is an argument someone will pass.
//!
//! Cancelling is always possible while the countdown runs. That is the entire point
//! of the countdown: the user gets a real chance to stop an irreversible action.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use crate::core::AppError;
use crate::platform::power_off::PowerOffExecutor;

/// The countdown length. Fixed, not configurable: a user-settable value could be set
/// to zero, which would remove the protection the countdown exists to provide.
pub const GRACE_PERIOD_SECONDS: u64 = 60;

/// How the countdown ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraceOutcome {
    /// The countdown completed and the executor was invoked successfully.
    PoweredOff,
    /// The user cancelled before it elapsed. The executor was not invoked.
    Cancelled,
    /// The countdown completed but the power-off itself failed.
    Failed(AppError),
}

/// A countdown in progress.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraceState {
    pub job_id: String,
    pub seconds_remaining: u64,
}

/// Abstracts waiting, so tests do not spend real time and can observe each tick.
///
/// Without this the only way to test a 60-second countdown is to wait 60 seconds, so
/// nobody tests it, and the one safety-critical delay in the app goes unverified.
pub trait GraceClock: Send + Sync {
    /// Sleeps roughly one second. Called once per remaining second.
    fn tick(&self);
}

/// Sleeps for real. Used in the app.
pub struct RealGraceClock;

impl GraceClock for RealGraceClock {
    fn tick(&self) {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}

/// Counts ticks and returns immediately. Used in tests.
#[derive(Default)]
pub struct InstantGraceClock {
    ticks: AtomicU64,
}

impl InstantGraceClock {
    pub fn tick_count(&self) -> u64 {
        self.ticks.load(Ordering::SeqCst)
    }
}

impl GraceClock for InstantGraceClock {
    fn tick(&self) {
        self.ticks.fetch_add(1, Ordering::SeqCst);
    }
}

/// Runs a closure on each tick, from inside the countdown.
///
/// Tests that need to act *during* the countdown — cancel it, or read the remaining
/// seconds — use this instead of racing a second thread against it. A spin-wait for
/// `is_counting_down` cannot work with an instant clock: the countdown may finish
/// before the other thread is scheduled, and then the waiter spins forever. Running
/// the assertion inside the tick removes the race rather than narrowing it.
pub struct ScriptedGraceClock {
    ticks: AtomicU64,
    #[allow(clippy::type_complexity)]
    script: Box<dyn Fn(u64) + Send + Sync>,
}

impl ScriptedGraceClock {
    /// `script` receives the number of ticks already elapsed, starting at 0.
    pub fn new(script: impl Fn(u64) + Send + Sync + 'static) -> Self {
        Self {
            ticks: AtomicU64::new(0),
            script: Box::new(script),
        }
    }

    pub fn tick_count(&self) -> u64 {
        self.ticks.load(Ordering::SeqCst)
    }
}

impl GraceClock for ScriptedGraceClock {
    fn tick(&self) {
        let elapsed = self.ticks.fetch_add(1, Ordering::SeqCst);
        (self.script)(elapsed);
    }
}

/// The only route to a power-off.
///
/// Holds the executor privately. Nothing else in the crate is given one, so the
/// countdown cannot be routed around — see the test asserting the executor is never
/// invoked when the countdown is cancelled.
pub struct PowerOffGate {
    executor: Arc<dyn PowerOffExecutor>,
    clock: Arc<dyn GraceClock>,
    /// The countdown currently running, if any.
    state: Mutex<Option<GraceState>>,
    cancelled: AtomicBool,
}

impl PowerOffGate {
    pub fn new(executor: Arc<dyn PowerOffExecutor>, clock: Arc<dyn GraceClock>) -> Self {
        Self {
            executor,
            clock,
            state: Mutex::new(None),
            cancelled: AtomicBool::new(false),
        }
    }

    /// Runs the full countdown, then powers off unless cancelled.
    ///
    /// Blocking, and called from a blocking task — the scheduler owns that. It takes
    /// no duration argument on purpose: [`GRACE_PERIOD_SECONDS`] is the only value,
    /// so there is no call site that could shorten it.
    pub fn run(&self, job_id: &str) -> GraceOutcome {
        self.cancelled.store(false, Ordering::SeqCst);

        if let Ok(mut state) = self.state.lock() {
            *state = Some(GraceState {
                job_id: job_id.to_string(),
                seconds_remaining: GRACE_PERIOD_SECONDS,
            });
        }

        for remaining in (0..GRACE_PERIOD_SECONDS).rev() {
            // Checked before each wait and again after the loop, so a cancel landing
            // in the final second is still honoured.
            if self.cancelled.load(Ordering::SeqCst) {
                self.clear();
                return GraceOutcome::Cancelled;
            }

            self.clock.tick();

            if let Ok(mut state) = self.state.lock() {
                if let Some(current) = state.as_mut() {
                    current.seconds_remaining = remaining;
                }
            }
        }

        if self.cancelled.load(Ordering::SeqCst) {
            self.clear();
            return GraceOutcome::Cancelled;
        }

        self.clear();

        // The countdown genuinely elapsed. This is the only call to `power_off` in
        // the crate.
        match self.executor.power_off() {
            Ok(()) => GraceOutcome::PoweredOff,
            Err(error) => GraceOutcome::Failed(error),
        }
    }

    /// Requests cancellation. Safe to call when nothing is counting down.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    /// The countdown in progress, for the UI.
    pub fn state(&self) -> Option<GraceState> {
        self.state.lock().ok().and_then(|state| state.clone())
    }

    pub fn is_counting_down(&self) -> bool {
        self.state().is_some()
    }

    /// Whether the platform can power off at all.
    pub fn is_supported(&self) -> bool {
        self.executor.is_supported()
    }

    fn clear(&self) {
        if let Ok(mut state) = self.state.lock() {
            *state = None;
        }
    }
}

/// Builds a gate whose clock can act on that same gate, one tick at a time.
///
/// The knot is that the gate owns the clock, so a script that wants to call
/// `gate.cancel()` would close a reference cycle. A `Weak` handle, filled in after the
/// gate exists, breaks it.
#[cfg(test)]
pub(crate) fn gate_with_script(
    executor: Arc<dyn PowerOffExecutor>,
    script: impl Fn(&PowerOffGate, u64) + Send + Sync + 'static,
) -> (Arc<PowerOffGate>, Arc<ScriptedGraceClock>) {
    use std::sync::{OnceLock, Weak};

    let handle: Arc<OnceLock<Weak<PowerOffGate>>> = Arc::new(OnceLock::new());

    let clock = Arc::new(ScriptedGraceClock::new({
        let handle = handle.clone();
        move |elapsed| {
            if let Some(gate) = handle.get().and_then(Weak::upgrade) {
                script(&gate, elapsed);
            }
        }
    }));

    let gate = Arc::new(PowerOffGate::new(executor, clock.clone()));
    let _ = handle.set(Arc::downgrade(&gate));
    (gate, clock)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::power_off::FakePowerOffExecutor;

    fn gate(executor: Arc<FakePowerOffExecutor>) -> (PowerOffGate, Arc<InstantGraceClock>) {
        let clock = Arc::new(InstantGraceClock::default());
        (PowerOffGate::new(executor, clock.clone()), clock)
    }

    #[test]
    fn the_countdown_runs_a_full_sixty_seconds_before_powering_off() {
        let executor = Arc::new(FakePowerOffExecutor::succeeding());
        let (gate, clock) = gate(executor.clone());

        assert_eq!(gate.run("job-1"), GraceOutcome::PoweredOff);
        assert_eq!(
            clock.tick_count(),
            GRACE_PERIOD_SECONDS,
            "the countdown must not be shortened"
        );
        assert_eq!(executor.call_count(), 1);
    }

    #[test]
    fn cancelling_prevents_the_power_off_entirely() {
        // The load-bearing test for the whole feature: a cancelled countdown must not
        // reach the executor at all. Cancelled in the first second, the earliest a
        // user could possibly react.
        let executor = Arc::new(FakePowerOffExecutor::succeeding());
        let (gate, _clock) = gate_with_script(executor.clone(), |gate, elapsed| {
            if elapsed == 0 {
                gate.cancel();
            }
        });

        assert_eq!(gate.run("job-1"), GraceOutcome::Cancelled);
        executor.assert_never_called();
    }

    #[test]
    fn a_cancel_partway_through_still_prevents_the_power_off() {
        let executor = Arc::new(FakePowerOffExecutor::succeeding());
        let (gate, clock) = gate_with_script(executor.clone(), |gate, elapsed| {
            if elapsed == 30 {
                gate.cancel();
            }
        });

        assert_eq!(gate.run("job-1"), GraceOutcome::Cancelled);
        executor.assert_never_called();
        assert_eq!(
            clock.tick_count(),
            31,
            "the countdown stops at the cancel rather than running to the end"
        );
    }

    #[test]
    fn a_cancel_in_the_final_second_still_prevents_the_power_off() {
        // The narrowest window there is. Missing it means a user who hits cancel just
        // as the countdown ends loses their work anyway.
        let executor = Arc::new(FakePowerOffExecutor::succeeding());
        let (gate, _clock) = gate_with_script(executor.clone(), |gate, elapsed| {
            if elapsed == GRACE_PERIOD_SECONDS - 1 {
                gate.cancel();
            }
        });

        assert_eq!(gate.run("job-1"), GraceOutcome::Cancelled);
        executor.assert_never_called();
    }

    #[test]
    fn the_remaining_seconds_count_down_and_are_visible_throughout() {
        // The UI reads this. Without it the user sees a shutdown warning with no
        // indication of how long they have to react.
        let executor = Arc::new(FakePowerOffExecutor::succeeding());
        let seen: Arc<Mutex<Vec<u64>>> = Arc::new(Mutex::new(Vec::new()));

        let (gate, _clock) = gate_with_script(executor, {
            let seen = seen.clone();
            move |gate, _elapsed| {
                let state = gate.state().expect("a countdown is in progress");
                assert_eq!(state.job_id, "job-1");
                seen.lock().unwrap().push(state.seconds_remaining);
            }
        });

        assert_eq!(gate.run("job-1"), GraceOutcome::PoweredOff);

        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), GRACE_PERIOD_SECONDS as usize);
        assert_eq!(
            seen[0], GRACE_PERIOD_SECONDS,
            "the first observation is the full countdown, before any second elapsed"
        );
        assert_eq!(*seen.last().unwrap(), 1, "the last observation is one second");
        assert!(
            seen.windows(2).all(|pair| pair[0] > pair[1]),
            "the countdown must only ever decrease: {seen:?}"
        );
    }

    #[test]
    fn the_countdown_state_is_cleared_once_it_ends() {
        let executor = Arc::new(FakePowerOffExecutor::succeeding());
        let (gate, _clock) = gate(executor);

        gate.run("job-1");
        assert!(gate.state().is_none());
        assert!(!gate.is_counting_down());
    }

    #[test]
    fn a_failing_power_off_is_reported_rather_than_swallowed() {
        // Task 10.10. A silent failure leaves the user believing the machine will
        // shut down when it will not.
        let executor = Arc::new(FakePowerOffExecutor::failing(
            AppError::PowerOffPrivilegeDenied {
                detail: Some("no privilege".to_string()),
            },
        ));
        let (gate, _clock) = gate(executor.clone());

        match gate.run("job-1") {
            GraceOutcome::Failed(AppError::PowerOffPrivilegeDenied { .. }) => {}
            other => panic!("expected a reported failure, got {other:?}"),
        }
        assert_eq!(executor.call_count(), 1);
    }

    #[test]
    fn a_second_run_after_a_cancel_is_not_still_cancelled() {
        // The cancel flag is per-run. If it leaked between runs, one cancel would
        // silently disable every future power-off — the app would look like it worked
        // and never shut anything down again.
        let executor = Arc::new(FakePowerOffExecutor::succeeding());
        let cancel_this_run = Arc::new(AtomicBool::new(true));

        let (gate, _clock) = gate_with_script(executor.clone(), {
            let cancel_this_run = cancel_this_run.clone();
            move |gate, _elapsed| {
                if cancel_this_run.load(Ordering::SeqCst) {
                    gate.cancel();
                }
            }
        });

        assert_eq!(gate.run("job-1"), GraceOutcome::Cancelled);
        executor.assert_never_called();

        cancel_this_run.store(false, Ordering::SeqCst);
        assert_eq!(gate.run("job-2"), GraceOutcome::PoweredOff);
        assert_eq!(executor.call_count(), 1);
    }

    #[test]
    fn a_cancel_arriving_before_a_countdown_does_not_disable_the_next_one() {
        // A stale cancel — a click landing after the countdown already ended — must not
        // suppress the *next* power-off. `run` resets the flag for exactly this reason.
        let executor = Arc::new(FakePowerOffExecutor::succeeding());
        let (gate, _clock) = gate(executor.clone());

        gate.cancel();
        assert_eq!(gate.run("job-1"), GraceOutcome::PoweredOff);
        assert_eq!(executor.call_count(), 1);
    }

    #[test]
    fn cancelling_when_nothing_is_counting_down_is_harmless() {
        let executor = Arc::new(FakePowerOffExecutor::succeeding());
        let (gate, _clock) = gate(executor.clone());

        gate.cancel();
        gate.cancel();
        executor.assert_never_called();
        assert!(!gate.is_counting_down());
    }

    #[test]
    fn an_unsupported_platform_is_reported_through_the_gate() {
        let executor = Arc::new(FakePowerOffExecutor::unsupported());
        let (gate, _clock) = gate(executor);

        assert!(!gate.is_supported());
    }
}

//! Test double for the keep-awake assertion.
//!
//! Records every transition so tests can assert on the *sequence*, not just the
//! final state. A controller that acquired twice and released once ends held, which
//! looks correct from the outside and is a leaked assertion.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use crate::core::{AppError, AppResult};
use crate::platform::keep_awake::controller::KeepAwakeController;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeepAwakeEvent {
    Acquired,
    Released,
    /// An acquire that was configured to fail — recorded so tests can tell "never
    /// tried" from "tried and was refused".
    AcquireFailed,
    ReleaseFailed,
}

pub struct FakeKeepAwakeController {
    held: AtomicBool,
    available: AtomicBool,
    acquire_error: Mutex<Option<AppError>>,
    release_error: Mutex<Option<AppError>>,
    events: Mutex<Vec<KeepAwakeEvent>>,
}

impl Default for FakeKeepAwakeController {
    fn default() -> Self {
        Self::working()
    }
}

impl FakeKeepAwakeController {
    /// Acquires and releases successfully.
    pub fn working() -> Self {
        Self {
            held: AtomicBool::new(false),
            available: AtomicBool::new(true),
            acquire_error: Mutex::new(None),
            release_error: Mutex::new(None),
            events: Mutex::new(Vec::new()),
        }
    }

    /// Available, but acquiring fails — the OS refused a mechanism that exists.
    pub fn failing_to_acquire(error: AppError) -> Self {
        let fake = Self::working();
        *fake.acquire_error.lock().unwrap() = Some(error);
        fake
    }

    /// Available and acquires, but releasing fails.
    pub fn failing_to_release(error: AppError) -> Self {
        let fake = Self::working();
        *fake.release_error.lock().unwrap() = Some(error);
        fake
    }

    /// No mechanism on this host at all.
    pub fn unavailable() -> Self {
        let fake = Self::working();
        fake.available.store(false, Ordering::SeqCst);
        *fake.acquire_error.lock().unwrap() = Some(AppError::KeepAwakeUnavailable {
            detail: Some("fake: no mechanism".to_string()),
        });
        fake
    }

    pub fn set_acquire_error(&self, error: Option<AppError>) {
        *self.acquire_error.lock().unwrap() = error;
    }

    pub fn events(&self) -> Vec<KeepAwakeEvent> {
        self.events.lock().unwrap().clone()
    }

    pub fn acquire_count(&self) -> usize {
        self.events()
            .iter()
            .filter(|e| **e == KeepAwakeEvent::Acquired)
            .count()
    }

    pub fn release_count(&self) -> usize {
        self.events()
            .iter()
            .filter(|e| **e == KeepAwakeEvent::Released)
            .count()
    }

    /// Fails the test when an assertion is still held.
    ///
    /// The whole point of task 8.5: the machine must not be left unable to sleep.
    pub fn assert_not_held(&self) {
        assert!(
            !self.is_held(),
            "a keep-awake assertion is still held — the display would never sleep. events: {:?}",
            self.events()
        );
    }

    fn record(&self, event: KeepAwakeEvent) {
        self.events.lock().unwrap().push(event);
    }
}

impl KeepAwakeController for FakeKeepAwakeController {
    fn acquire(&self) -> AppResult<()> {
        if let Some(error) = self.acquire_error.lock().unwrap().clone() {
            self.record(KeepAwakeEvent::AcquireFailed);
            return Err(error);
        }
        self.held.store(true, Ordering::SeqCst);
        self.record(KeepAwakeEvent::Acquired);
        Ok(())
    }

    fn release(&self) -> AppResult<()> {
        if let Some(error) = self.release_error.lock().unwrap().clone() {
            self.record(KeepAwakeEvent::ReleaseFailed);
            return Err(error);
        }
        self.held.store(false, Ordering::SeqCst);
        self.record(KeepAwakeEvent::Released);
        Ok(())
    }

    fn is_held(&self) -> bool {
        self.held.load(Ordering::SeqCst)
    }

    fn is_available(&self) -> bool {
        self.available.load(Ordering::SeqCst)
    }

    fn unavailable_reason(&self) -> Option<String> {
        if self.is_available() {
            None
        } else {
            Some("fake: no mechanism".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fake_records_the_order_of_transitions() {
        let fake = FakeKeepAwakeController::working();
        fake.acquire().unwrap();
        fake.release().unwrap();
        fake.acquire().unwrap();

        assert_eq!(
            fake.events(),
            vec![
                KeepAwakeEvent::Acquired,
                KeepAwakeEvent::Released,
                KeepAwakeEvent::Acquired
            ]
        );
        assert!(fake.is_held());
    }

    #[test]
    fn a_refused_acquire_is_recorded_and_leaves_nothing_held() {
        let fake = FakeKeepAwakeController::failing_to_acquire(AppError::KeepAwakeFailed {
            message: "refused".to_string(),
        });

        assert!(fake.acquire().is_err());
        assert_eq!(fake.events(), vec![KeepAwakeEvent::AcquireFailed]);
        assert!(!fake.is_held());
        assert_eq!(fake.acquire_count(), 0);
    }

    #[test]
    #[should_panic(expected = "the display would never sleep")]
    fn assert_not_held_fails_when_an_assertion_leaked() {
        let fake = FakeKeepAwakeController::working();
        fake.acquire().unwrap();
        fake.assert_not_held();
    }
}

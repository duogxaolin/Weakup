use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use crate::core::{AppError, AppResult};
use crate::platform::power_off::executor::PowerOffExecutor;

/// What the fake should return, and how it should report support.
#[derive(Debug, Clone)]
pub enum FakeResponse {
    /// The command was accepted.
    Success,
    /// The command failed with this error.
    Failure(AppError),
    /// Power-off is categorically impossible here.
    Unsupported,
}

/// Recording [`PowerOffExecutor`] for tests.
///
/// The only executor the test suite may bind (D6). It records calls instead of
/// making them, which is what lets the safety-critical assertions be written as
/// "the executor was NOT called" — the strongest form of the guarantee that a
/// refused power-off really did nothing.
pub struct FakePowerOffExecutor {
    response: Mutex<FakeResponse>,
    calls: AtomicUsize,
}

impl FakePowerOffExecutor {
    /// A fake that accepts the command.
    pub fn succeeding() -> Self {
        Self {
            response: Mutex::new(FakeResponse::Success),
            calls: AtomicUsize::new(0),
        }
    }

    /// A fake that fails with `error`.
    pub fn failing(error: AppError) -> Self {
        Self {
            response: Mutex::new(FakeResponse::Failure(error)),
            calls: AtomicUsize::new(0),
        }
    }

    /// A fake standing in for a platform that cannot power off.
    pub fn unsupported() -> Self {
        Self {
            response: Mutex::new(FakeResponse::Unsupported),
            calls: AtomicUsize::new(0),
        }
    }

    /// Changes the response mid-test, for retry and failure-path cases.
    pub fn set_response(&self, response: FakeResponse) {
        *self.response.lock().expect("fake response lock") = response;
    }

    /// How many times a power-off was requested.
    pub fn call_count(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }

    /// Whether the device would have been powered off.
    pub fn was_called(&self) -> bool {
        self.call_count() > 0
    }

    /// Asserts nothing was attempted. The assertion behind every "must not power
    /// off" rule: overdue beyond tolerance, a cancelled countdown, an unsupported
    /// platform.
    pub fn assert_never_called(&self) {
        assert_eq!(
            self.call_count(),
            0,
            "the device would have been powered off, and must not have been"
        );
    }
}

impl PowerOffExecutor for FakePowerOffExecutor {
    fn power_off(&self) -> AppResult<()> {
        // Counted before the response is read, so a call is recorded even on the
        // failure paths.
        self.calls.fetch_add(1, Ordering::SeqCst);

        match self.response.lock().expect("fake response lock").clone() {
            FakeResponse::Success => Ok(()),
            FakeResponse::Failure(error) => Err(error),
            FakeResponse::Unsupported => Err(AppError::PowerOffUnsupported),
        }
    }

    fn is_supported(&self) -> bool {
        !matches!(
            *self.response.lock().expect("fake response lock"),
            FakeResponse::Unsupported
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fake_records_calls_without_making_them() {
        let fake = FakePowerOffExecutor::succeeding();
        fake.assert_never_called();

        assert!(fake.power_off().is_ok());
        assert_eq!(fake.call_count(), 1);
        assert!(fake.was_called());
    }

    #[test]
    fn a_failing_call_is_still_recorded() {
        // Otherwise "the executor was not called" would pass for a call that was
        // made and rejected — a device that did attempt to shut down.
        let fake = FakePowerOffExecutor::failing(AppError::PowerOffFailed {
            message: "boom".into(),
        });
        assert!(fake.power_off().is_err());
        assert_eq!(fake.call_count(), 1);
    }

    #[test]
    fn the_response_can_change_mid_test() {
        let fake = FakePowerOffExecutor::succeeding();
        assert!(fake.power_off().is_ok());

        fake.set_response(FakeResponse::Failure(AppError::PowerOffPrivilegeDenied {
            detail: Some("no privilege".to_string()),
        }));
        assert!(matches!(
            fake.power_off(),
            Err(AppError::PowerOffPrivilegeDenied { .. })
        ));
        assert_eq!(fake.call_count(), 2);
    }

    #[test]
    fn an_unsupported_fake_reports_unsupported() {
        let fake = FakePowerOffExecutor::unsupported();
        assert!(!fake.is_supported());
        assert!(matches!(
            fake.power_off(),
            Err(AppError::PowerOffUnsupported)
        ));
    }

    #[test]
    #[should_panic(expected = "would have been powered off")]
    fn assert_never_called_fails_when_the_executor_was_called() {
        // The guard itself is tested: a broken assert_never_called would make
        // every "must not power off" test pass vacuously.
        let fake = FakePowerOffExecutor::succeeding();
        let _ = fake.power_off();
        fake.assert_never_called();
    }
}

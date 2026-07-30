//! The keep-awake assertion, as the rest of the app sees it.

use crate::core::AppResult;

/// Holds the OS awake while the app asks it to.
///
/// Implementations are expected to be idempotent: acquiring twice holds one
/// assertion, releasing twice is not an error. The coordinator above this trait
/// already deduplicates, but an implementation that broke on a double call would
/// turn a harmless race into a leaked assertion, and a leaked display assertion
/// is exactly the bug the user would notice — their screen never sleeps again,
/// after the app thinks it stopped.
pub trait KeepAwakeController: Send + Sync {
    /// Asks the OS to keep the display awake. Idempotent.
    fn acquire(&self) -> AppResult<()>;

    /// Releases the assertion. Idempotent, and must succeed as far as the caller
    /// is concerned even if nothing was held.
    fn release(&self) -> AppResult<()>;

    /// Whether an assertion is held right now.
    fn is_held(&self) -> bool;

    /// Whether this host has a working mechanism at all.
    ///
    /// Distinct from a failed acquire: on Linux there may be no D-Bus service and
    /// no `systemd-inhibit`, in which case the honest answer is that keep-awake is
    /// unavailable, not that it failed (task 8.6).
    fn is_available(&self) -> bool;

    /// Why keep-awake is unavailable, when it is. `None` when available.
    fn unavailable_reason(&self) -> Option<String> {
        None
    }
}

/// Used where the platform has no mechanism — Linux without D-Bus or
/// `systemd-inhibit`, and any target not covered by the three desktop OSes.
///
/// `acquire` fails rather than silently doing nothing: a job that believes it holds
/// the screen awake when nothing does is worse than one that reports it could not.
#[derive(Debug, Default)]
pub struct UnavailableKeepAwakeController {
    reason: Option<String>,
}

impl UnavailableKeepAwakeController {
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: Some(reason.into()),
        }
    }
}

impl KeepAwakeController for UnavailableKeepAwakeController {
    fn acquire(&self) -> AppResult<()> {
        Err(crate::core::AppError::KeepAwakeUnavailable {
            detail: self.reason.clone(),
        })
    }

    fn release(&self) -> AppResult<()> {
        // Nothing was ever held, so there is nothing to fail at. Releasing must not
        // error, or cleanup paths would report a problem while tearing down a job
        // that never had an assertion.
        Ok(())
    }

    fn is_held(&self) -> bool {
        false
    }

    fn is_available(&self) -> bool {
        false
    }

    fn unavailable_reason(&self) -> Option<String> {
        self.reason.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::AppError;

    #[test]
    fn the_unavailable_controller_fails_to_acquire_and_never_claims_to_hold() {
        let controller = UnavailableKeepAwakeController::new("no D-Bus, no systemd-inhibit");

        match controller.acquire() {
            Err(AppError::KeepAwakeUnavailable { detail }) => {
                assert_eq!(detail.as_deref(), Some("no D-Bus, no systemd-inhibit"));
            }
            other => panic!("expected unavailable, got {other:?}"),
        }

        assert!(!controller.is_held());
        assert!(!controller.is_available());
    }

    #[test]
    fn releasing_an_unavailable_controller_is_not_an_error() {
        let controller = UnavailableKeepAwakeController::default();
        assert!(controller.release().is_ok());
        assert!(controller.release().is_ok());
    }
}

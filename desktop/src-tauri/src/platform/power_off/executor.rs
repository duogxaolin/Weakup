use crate::core::{AppError, AppResult};

/// Executes a platform power-off.
///
/// Injected everywhere (D6): all scheduling and grace-period logic depends on this
/// trait, never on a concrete implementation, so tests can bind a fake and no test
/// can shut down the machine running it.
///
/// `Send + Sync` because the scheduler holds it across `tokio` tasks.
pub trait PowerOffExecutor: Send + Sync {
    /// Powers off the device.
    ///
    /// `Ok(())` means the command was accepted, not that the machine is off — by
    /// then this process is being torn down. Callers must treat a return as
    /// "shutdown is underway" and do nothing that depends on staying alive.
    fn power_off(&self) -> AppResult<()>;

    /// Whether this platform can power off at all. `false` only where it is
    /// categorically impossible, not where it might fail on permissions.
    fn is_supported(&self) -> bool;
}

/// For targets where power-off is categorically impossible.
///
/// Returns `PowerOffUnsupported` and invokes nothing. On mobile this is the only
/// correct answer: no channel, root helper, or plugin may be added to work around
/// it. Kept in the desktop tree so an unrecognised target degrades honestly rather
/// than silently reporting success.
pub struct UnsupportedPowerOffExecutor;

impl PowerOffExecutor for UnsupportedPowerOffExecutor {
    fn power_off(&self) -> AppResult<()> {
        Err(AppError::PowerOffUnsupported)
    }

    fn is_supported(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_unsupported_executor_reports_unsupported_and_never_succeeds() {
        let executor = UnsupportedPowerOffExecutor;
        assert!(!executor.is_supported());
        assert!(matches!(
            executor.power_off(),
            Err(AppError::PowerOffUnsupported)
        ));
    }
}

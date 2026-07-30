use std::process::Command;

use crate::core::AppResult;
use crate::platform::power_off::classify::classify_windows;
use crate::platform::power_off::executor::{run_shutdown_command, PowerOffExecutor};

/// Windows: `shutdown /s /t 0`.
///
/// `/t 0` means no additional delay — the app's own 60-second grace countdown has
/// already run by this point, and stacking a second timer would leave the OS
/// holding a shutdown the app can no longer cancel.
///
/// Needs `SeShutdownPrivilege`, which an interactive user normally holds. Where
/// policy withholds it, `classify_windows` reports `PowerOffPrivilegeDenied`.
///
/// UNVERIFIED: never compiled or run on Windows. `cargo check` for the Windows
/// target type-checks this file; it does not execute it.
pub struct WindowsPowerOffExecutor;

impl WindowsPowerOffExecutor {
    pub fn new() -> Self {
        Self
    }
}

impl Default for WindowsPowerOffExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl PowerOffExecutor for WindowsPowerOffExecutor {
    fn power_off(&self) -> AppResult<()> {
        let mut command = Command::new("shutdown");
        command.args(["/s", "/t", "0"]);
        run_shutdown_command(command, classify_windows)
    }

    fn is_supported(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_reports_power_off_as_supported() {
        assert!(WindowsPowerOffExecutor::new().is_supported());
    }
}

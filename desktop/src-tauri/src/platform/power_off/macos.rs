use std::process::Command;

use crate::core::AppResult;
use crate::platform::power_off::classify::classify_macos;
use crate::platform::power_off::executor::{run_shutdown_command, PowerOffExecutor};

/// macOS: AppleScript driving System Events.
///
/// Requires the Automation consent declared by `NSAppleEventsUsageDescription` in
/// `Info.plist`. Its absence is what made the Flutter macOS build fail, so the
/// declaration is not optional here.
///
/// `shutdown -h now` is not used: it needs root, which a user-launched GUI app does
/// not have and should not ask for.
pub struct MacOsPowerOffExecutor;

impl MacOsPowerOffExecutor {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MacOsPowerOffExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl PowerOffExecutor for MacOsPowerOffExecutor {
    fn power_off(&self) -> AppResult<()> {
        let mut command = Command::new("osascript");
        command.args(["-e", "tell application \"System Events\" to shut down"]);
        run_shutdown_command(command, classify_macos)
    }

    fn is_supported(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macos_reports_power_off_as_supported() {
        // Deliberately the only test here. Calling `power_off` would shut down the
        // machine running the suite; the classification it depends on is tested
        // exhaustively in `classify_tests`.
        assert!(MacOsPowerOffExecutor::new().is_supported());
    }
}

use std::process::Command;

use crate::core::AppResult;
use crate::platform::power_off::classify::classify_linux;
use crate::platform::power_off::executor::{run_shutdown_command, PowerOffExecutor};

/// Linux: `systemctl poweroff`.
///
/// Goes through logind, so polkit decides. On a typical desktop the active local
/// session is permitted; where it is not, `classify_linux` reports
/// `PowerOffPolicyDenied` — a policy refusal the app cannot and should not try to
/// escalate around.
///
/// `poweroff` and `shutdown -h now` are not used: both need root on most
/// distributions, and a GUI app asking for root to schedule a shutdown is worse
/// than telling the user their policy forbids it.
///
/// UNVERIFIED: never compiled or run on Linux. `cargo check` for the Linux target
/// type-checks this file; it does not execute it.
pub struct LinuxPowerOffExecutor;

impl LinuxPowerOffExecutor {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LinuxPowerOffExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl PowerOffExecutor for LinuxPowerOffExecutor {
    fn power_off(&self) -> AppResult<()> {
        let mut command = Command::new("systemctl");
        command.arg("poweroff");
        run_shutdown_command(command, classify_linux)
    }

    fn is_supported(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linux_reports_power_off_as_supported() {
        assert!(LinuxPowerOffExecutor::new().is_supported());
    }
}

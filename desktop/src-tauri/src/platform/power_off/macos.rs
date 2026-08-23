use std::process::Command;

use crate::core::AppResult;
use crate::platform::power_off::classify::classify_macos;
use crate::platform::power_off::executor::{run_shutdown_command, PowerOffExecutor};

/// The AppleScript [`MacOsPowerOffExecutor::power_off`] sends, kept as a constant
/// so the test below can pin its shape without spawning anything.
///
/// The shape exists because of a real production failure. The original script,
/// `tell application "System Events" to shut down`, asked for a *graceful* logout:
/// every running app receives a quit event, and any one of them may hold or veto
/// it indefinitely. At a scheduled shutdown, Tabby failed to quit and a `node`
/// process sat behind a close dialog; macOS showed its "hasn't shut down" banners,
/// the machine stayed on — and `osascript` still exited 0, because that exit
/// status reports only that the Apple Event was *accepted*, never that the apps
/// complied. The classifier reads what the process reported, and the process had
/// reported success for a shutdown that did not happen.
///
/// Two properties close that hole:
///
/// - `ignoring application responses` sends the `shut down` event without waiting
///   for each app's reply, so a stuck app cannot hold up delivery. This is the
///   documented way to shut down past applications that refuse to quit.
/// - `with timeout of 3 seconds` bounds the whole script, so a wedged Apple Event
///   Manager fails fast (errAEEventTimedOut, `-1712`) instead of hanging the
///   scheduler thread that runs the grace countdown inline. The closer is
///   `end timeout`, not `end with timeout` — AppleScript's compiler accepts only
///   that spelling for this block, and the shape was compile-verified with a
///   benign command before landing.
///
/// Sent as one multi-line `-e` argument; osascript parses multi-line `-e` values
/// as one script, and the `ignoring` block does not survive the one-liner form.
const SHUTDOWN_SCRIPT: &str = r#"with timeout of 3 seconds
    tell application "System Events"
        ignoring application responses
            shut down
        end ignoring
    end tell
end timeout"#;

/// Builds (never runs) the `osascript` invocation, so the argument shape is
/// testable without risking a real shutdown.
fn osascript_shutdown() -> Command {
    let mut command = Command::new("osascript");
    command.args(["-e", SHUTDOWN_SCRIPT]);
    command
}

/// macOS: AppleScript driving System Events.
///
/// Requires the Automation consent declared by `NSAppleEventsUsageDescription` in
/// `Info.plist`. Its absence is what made the Flutter macOS build fail, so the
/// declaration is not optional here.
///
/// `shutdown -h now` is not used: it needs root, which a user-launched GUI app does
/// not have and should not ask for. Why the script ignores application responses:
/// see [`SHUTDOWN_SCRIPT`] — without that, any misbehaving app could veto the
/// scheduled shutdown while the command still reported success.
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
        run_shutdown_command(osascript_shutdown(), classify_macos)
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
        // Calling `power_off` would shut down the machine running the suite; the
        // classification it depends on is tested exhaustively in `classify_tests`.
        assert!(MacOsPowerOffExecutor::new().is_supported());
    }

    #[test]
    fn the_shutdown_script_ignores_application_responses() {
        // The production bug this pins: the plain `shut down` is a graceful logout
        // that any app (Tabby, a node process behind a close dialog) can veto,
        // while osascript still exits 0. `ignoring application responses` is what
        // makes the event fire past apps that refuse to quit.
        assert!(
            SHUTDOWN_SCRIPT.contains("ignoring application responses"),
            "the script must not wait for per-app replies, or a stuck app vetoes the shutdown"
        );
        assert!(
            SHUTDOWN_SCRIPT.contains("shut down"),
            "the script must still request the shutdown itself"
        );
    }

    #[test]
    fn the_shutdown_script_is_time_bounded() {
        // The grace countdown runs inline on the scheduler tick; an unbounded
        // AppleScript could hang that thread forever if the Apple Event Manager
        // wedges. 3 seconds bounds the damage.
        assert!(
            SHUTDOWN_SCRIPT.contains("with timeout of 3 seconds"),
            "the script must carry its own timeout"
        );
        // AppleScript accepts only `end timeout` as the closer for this block;
        // `end with timeout` fails to compile with -2741.
        assert!(
            SHUTDOWN_SCRIPT.contains("end timeout"),
            "the timeout block must be closed with the spelling osascript accepts"
        );
    }

    #[test]
    fn osascript_receives_the_script_as_a_single_dash_e_argument() {
        // The `ignoring ... end ignoring` block does not survive being split into
        // separate -e flags, so the whole script must travel as one argument to a
        // single -e. Building the Command is safe; running it never happens here.
        let command = osascript_shutdown();
        let args: Vec<String> = command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();

        assert_eq!(command.get_program(), "osascript");
        assert_eq!(args.len(), 2, "exactly one -e flag and one script");
        assert_eq!(args[0], "-e");
        assert_eq!(args[1], SHUTDOWN_SCRIPT);
        assert!(
            args[1].contains("end ignoring"),
            "the full block must be inside the one argument, not truncated"
        );
    }
}

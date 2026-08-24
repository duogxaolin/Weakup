use std::process::Command;

use crate::core::AppResult;
use crate::platform::power_off::classify::classify_macos;
use crate::platform::power_off::executor::{run_shutdown_command, PowerOffExecutor};

/// The AppleScript [`MacOsPowerOffExecutor::power_off`] sends, kept as a constant
/// so the test below can pin its shape without spawning anything.
///
/// Why it targets `loginwindow` with a raw Apple Event instead of the obvious
/// `tell application "System Events" to shut down`: that obvious form asks for a
/// *graceful* logout. Every running app receives a quit event, and any one of
/// them may refuse it — at a scheduled shutdown Tabby held a "node is still
/// running. Close?" dialog behind it, macOS showed its "hasn't shut down"
/// banner, and the machine stayed on. Release 0.2.1 shipped a variant wrapped in
/// `ignoring application responses`, which only stops *osascript* from waiting
/// for each app's reply — the apps keep their veto at the loginwindow/session
/// level regardless, and `osascript` still exited 0 because that exit status
/// reports only that the Apple Event was accepted. So the job was marked
/// Completed while the machine stayed on: the classifier reads what the process
/// reported, and the process reported success for a shutdown that never
/// happened. `ignoring application responses` cannot fix this class of failure,
/// because the failure is not in the reply path.
///
/// `«event aevtrsdn»` is the raw four-character Apple Event
/// `kAEReallyBeginShutdown` (`'rsdn'`) — the event loginwindow dispatches when a
/// user picks Shut Down — sent straight to `loginwindow`, which *owns* the
/// logout/shutdown sequence: it quits apps itself, escalates to force-kill the
/// ones that will not quit, and then hands off to launchd (Apple TN2128
/// documents the sequence). Delivered this way the event is the shutdown
/// decision itself, not a per-app quit request any single app can hold off
/// indefinitely. This is the long-established scripted-restart trick
/// (`tell app "loginwindow" to «event aevtrrst»`) and it needs no root, which
/// matters: `shutdown -h now` does need root, and a user-launched GUI app has
/// none to spare.
///
/// Caveats stated honestly: loginwindow's Apple Event interface is undocumented,
/// and how TCC treats it may vary by macOS version. Neither risk can fail
/// silently — a refusal exits non-zero and [`classify_macos`] turns it into a
/// visible `Failed` job plus a notification, and the preflight
/// ([`crate::platform::power_off::preflight`]) targets the same bundle id.
///
/// `with timeout of 3 seconds` bounds the whole script, so a wedged Apple Event
/// Manager fails fast (errAEEventTimedOut, `-1712`) instead of hanging the
/// scheduler thread that runs the grace countdown inline. The closer is
/// `end timeout`, not `end with timeout` — AppleScript's compiler accepts only
/// that spelling for this block. The shape was verified with `osacompile` before
/// landing; executing it here is forbidden, because executing it shuts the build
/// machine down.
const SHUTDOWN_SCRIPT: &str = r#"with timeout of 3 seconds
    tell application "loginwindow" to «event aevtrsdn»
end timeout"#;

/// Builds (never runs) the `osascript` invocation, so the argument shape is
/// testable without risking a real shutdown.
fn osascript_shutdown() -> Command {
    let mut command = Command::new("osascript");
    command.args(["-e", SHUTDOWN_SCRIPT]);
    command
}

/// macOS: AppleScript sending the shutdown Apple Event directly to `loginwindow`.
///
/// Requires the Automation consent declared by `NSAppleEventsUsageDescription` in
/// `Info.plist`. Its absence is what made the Flutter macOS build fail, so the
/// declaration is not optional here.
///
/// `shutdown -h now` is not used: it needs root, which a user-launched GUI app does
/// not have and should not ask for. Why the event goes to `loginwindow` rather
/// than through System Events' graceful `shut down` verb: see [`SHUTDOWN_SCRIPT`] —
/// the graceful form lets any misbehaving app veto the scheduled shutdown while
/// the command still reports success.
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
    fn the_shutdown_event_goes_to_loginwindow_not_the_graceful_path() {
        // The production bug this pins, twice over: the plain `shut down` through
        // System Events is a graceful logout any app (Tabby holding a close
        // dialog) can veto, and 0.2.1's `ignoring application responses` wrapper
        // only stopped osascript from waiting — the veto lives at the
        // loginwindow/session level, so osascript still exited 0 for a shutdown
        // that never happened. `kAEReallyBeginShutdown` ('rsdn') sent to
        // loginwindow IS the shutdown decision; no single app can hold it off.
        assert!(
            SHUTDOWN_SCRIPT.contains(r#"tell application "loginwindow""#),
            "the event must target loginwindow, which owns the shutdown sequence"
        );
        assert!(
            SHUTDOWN_SCRIPT.contains("«event aevtrsdn»"),
            "the raw kAEReallyBeginShutdown event must be sent"
        );
        assert!(
            !SHUTDOWN_SCRIPT.contains("shut down"),
            "the graceful `shut down` verb must be gone — it is the vettable path"
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
        // Building the Command is safe; running it never happens here. The whole
        // script travels as one argument to a single -e so nothing is truncated.
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
            args[1].contains("aevtrsdn"),
            "the delivered script must carry the loginwindow event intact"
        );
    }
}

//! Pure mapping from a command's captured output to an [`AppError`].
//!
//! Deliberately separate from the executors. Classification is decided from what
//! the command *actually reported* — exit status, stderr, stdout — never from a
//! predicted permission state (task 7.7). Predicting is what produces the "we
//! thought we had permission" class of bug; asking the OS and reading the answer
//! does not.
//!
//! All three OS classifiers are compiled and tested on every host. They are string
//! and integer functions with no OS dependency, so testing the Windows one here is
//! both possible and worth doing — the alternative is shipping two of three
//! targets with this logic never executed. Only the process-spawning executors are
//! `#[cfg]`-gated.

use crate::core::AppError;

/// What a shutdown command reported back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutcome {
    /// `None` when the process was terminated by a signal before exiting.
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl CommandOutcome {
    pub fn new(exit_code: Option<i32>, stdout: impl Into<String>, stderr: impl Into<String>) -> Self {
        Self {
            exit_code,
            stdout: stdout.into(),
            stderr: stderr.into(),
        }
    }

    pub fn succeeded(&self) -> bool {
        self.exit_code == Some(0)
    }

    /// Both streams, lowercased, for substring matching.
    ///
    /// Both are needed: `osascript` writes some authorisation errors to stdout,
    /// and a message split across the two would otherwise be missed.
    fn haystack(&self) -> String {
        format!("{} {}", self.stderr, self.stdout).to_lowercase()
    }

    /// The message shown to the user when nothing more specific is known.
    /// Falls back to a description of the exit status when both streams are empty,
    /// so the error is never blank.
    fn failure_message(&self) -> String {
        let combined = format!("{} {}", self.stderr.trim(), self.stdout.trim())
            .trim()
            .to_string();
        if !combined.is_empty() {
            return combined;
        }
        match self.exit_code {
            Some(code) => format!("The shutdown command exited with status {code}."),
            None => "The shutdown command was terminated before it exited.".to_string(),
        }
    }
}

/// macOS: `osascript` driving System Events.
///
/// The interesting failure is TCC consent: the user declined the Automation
/// prompt, or never saw it. `PowerOffConsentDenied` is what tells the UI to name
/// the System Settings path rather than showing a raw AppleScript error.
pub fn classify_macos(outcome: &CommandOutcome) -> Option<AppError> {
    if outcome.succeeded() {
        return None;
    }

    let text = outcome.haystack();

    // -1743 is errAEEventNotPermitted, the code osascript returns for a declined
    // or missing Automation authorisation. Matched alongside the wordings because
    // the numeric code is stable across locales and the English text is not.
    const CONSENT_MARKERS: [&str; 6] = [
        "-1743",
        "not authorized",
        "not authorised",
        "not allowed",
        "access denied",
        "not permitted",
    ];

    if CONSENT_MARKERS.iter().any(|marker| text.contains(marker)) {
        return Some(AppError::PowerOffConsentDenied {
            detail: Some(outcome.failure_message()),
        });
    }

    Some(AppError::PowerOffFailed {
        message: outcome.failure_message(),
    })
}

/// Windows: `shutdown /s /t 0`.
///
/// `ERROR_PRIVILEGE_NOT_HELD` (1314) means the process lacks
/// `SeShutdownPrivilege` — a standard user on a machine where policy withholds it.
/// `ERROR_ACCESS_DENIED` (5) is the same class of problem from the user's side.
pub fn classify_windows(outcome: &CommandOutcome) -> Option<AppError> {
    if outcome.succeeded() {
        return None;
    }

    let text = outcome.haystack();

    // `shutdown.exe` reports the failure in its message text; the exit code is
    // also the Win32 error, so both are checked.
    const PRIVILEGE_MARKERS: [&str; 4] = [
        "1314",
        "error_privilege_not_held",
        "does not have the required privilege",
        "a required privilege is not held",
    ];

    let privileged_exit = matches!(outcome.exit_code, Some(1314) | Some(5));
    let access_denied = text.contains("access is denied") || text.contains("access denied");

    if privileged_exit
        || access_denied
        || PRIVILEGE_MARKERS.iter().any(|marker| text.contains(marker))
    {
        return Some(AppError::PowerOffPrivilegeDenied {
            detail: Some(outcome.failure_message()),
        });
    }

    Some(AppError::PowerOffFailed {
        message: outcome.failure_message(),
    })
}

/// Linux: `systemctl poweroff`.
///
/// polkit refusing the action is a *policy* denial, not a bug and not a missing
/// privilege the app can acquire — distinct from Windows, where the privilege can
/// be held. "Interactive authentication required" is the same refusal arriving as
/// a prompt the app cannot answer from a background timer.
pub fn classify_linux(outcome: &CommandOutcome) -> Option<AppError> {
    if outcome.succeeded() {
        return None;
    }

    let text = outcome.haystack();

    const POLICY_MARKERS: [&str; 6] = [
        "access denied",
        "polkit",
        "interactive authentication required",
        "not authorized",
        "not permitted",
        "permission denied",
    ];

    if POLICY_MARKERS.iter().any(|marker| text.contains(marker)) {
        return Some(AppError::PowerOffPolicyDenied {
            detail: Some(outcome.failure_message()),
        });
    }

    Some(AppError::PowerOffFailed {
        message: outcome.failure_message(),
    })
}

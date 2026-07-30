//! Classification tests for all three desktop OSes.
//!
//! These run on every host. That is the point: Windows and Linux cannot be booted
//! here, so this is the only place their per-OS behaviour is actually executed
//! rather than merely type-checked. The strings are the real messages those tools
//! emit.

use crate::core::AppError;
use crate::platform::power_off::classify::{
    classify_linux, classify_macos, classify_windows, CommandOutcome,
};

fn ok() -> CommandOutcome {
    CommandOutcome::new(Some(0), "", "")
}

fn failed(code: i32, stderr: &str) -> CommandOutcome {
    CommandOutcome::new(Some(code), "", stderr)
}

type Classifier = fn(&CommandOutcome) -> Option<AppError>;

/// Every classifier, for the cases that must hold identically across all three.
const ALL: [(&str, Classifier); 3] = [
    ("macos", classify_macos),
    ("windows", classify_windows),
    ("linux", classify_linux),
];

#[test]
fn exit_zero_is_success_on_every_os() {
    for (name, classify) in ALL {
        assert!(
            classify(&ok()).is_none(),
            "{name} must treat exit 0 as success"
        );
    }
}

#[test]
fn a_nonzero_exit_is_never_silently_successful() {
    // The rule that matters most: a shutdown that did not happen must never be
    // reported as one that did, or the job is marked completed and the machine
    // stays on.
    for (name, classify) in ALL {
        for outcome in [
            failed(1, "something went wrong"),
            failed(1, ""),
            CommandOutcome::new(None, "", ""),
            CommandOutcome::new(Some(255), "", ""),
        ] {
            assert!(
                classify(&outcome).is_some(),
                "{name} reported success for a failed command: {outcome:?}"
            );
        }
    }
}

#[test]
fn an_error_message_is_never_blank() {
    // A blank message would surface as "Failed" with no reason, which is what the
    // user is left staring at when their machine did not shut down.
    for (name, classify) in ALL {
        for outcome in [
            failed(1, ""),
            CommandOutcome::new(None, "", ""),
            CommandOutcome::new(Some(7), "   ", "  "),
        ] {
            let error = classify(&outcome).expect("failure");
            let text = match &error {
                AppError::PowerOffFailed { message } => message.clone(),
                AppError::PowerOffConsentDenied { detail }
                | AppError::PowerOffPrivilegeDenied { detail }
                | AppError::PowerOffPolicyDenied { detail } => detail
                    .clone()
                    .unwrap_or_else(|| panic!("{name} left the detail empty for {outcome:?}")),
                other => panic!("{name}: unexpected variant {other:?}"),
            };
            assert!(
                !text.trim().is_empty(),
                "{name} produced a blank message for {outcome:?}"
            );
        }
    }
}

// ---- macOS ----

#[test]
fn macos_classifies_tcc_consent_denial() {
    // The real osascript output when Automation permission is declined or absent.
    let cases = [
        "execution error: Not authorized to send Apple events to System Events. (-1743)",
        "System Events got an error: weakup is not allowed sending keystrokes. (-1719)",
        "execution error: Access denied. (-1743)",
    ];

    for stderr in cases {
        match classify_macos(&failed(1, stderr)) {
            Some(AppError::PowerOffConsentDenied { detail }) => {
                assert_eq!(detail.as_deref(), Some(stderr));
            }
            other => panic!("expected consent denial for {stderr:?}, got {other:?}"),
        }
    }
}

#[test]
fn macos_classifies_consent_denial_reported_on_stdout() {
    // osascript writes some authorisation errors to stdout, so a classifier
    // reading only stderr would misreport them as a generic failure and the UI
    // would not show the System Settings path.
    let outcome = CommandOutcome::new(Some(1), "Not authorized to send Apple events.", "");
    assert!(matches!(
        classify_macos(&outcome),
        Some(AppError::PowerOffConsentDenied { .. })
    ));
}

#[test]
fn macos_error_code_is_matched_regardless_of_locale() {
    // -1743 is stable; the English text is not. A non-English system must still
    // get the consent path rather than a raw error.
    let outcome = failed(1, "erreur d\u{2019}ex\u{e9}cution: non autoris\u{e9}. (-1743)");
    assert!(matches!(
        classify_macos(&outcome),
        Some(AppError::PowerOffConsentDenied { .. })
    ));
}

#[test]
fn macos_classifies_an_unrelated_failure_as_a_plain_failure() {
    match classify_macos(&failed(1, "syntax error: Expected end of line. (-2741)")) {
        Some(AppError::PowerOffFailed { message }) => {
            assert!(message.contains("syntax error"));
        }
        other => panic!("expected a plain failure, got {other:?}"),
    }
}

// ---- Windows ----

#[test]
fn windows_classifies_privilege_denial_from_the_message() {
    let cases = [
        "shutdown: Access is denied.(5)",
        "A required privilege is not held by the client. (1314)",
        "ERROR_PRIVILEGE_NOT_HELD",
    ];

    for stderr in cases {
        match classify_windows(&failed(1, stderr)) {
            Some(AppError::PowerOffPrivilegeDenied { .. }) => {}
            other => panic!("expected privilege denial for {stderr:?}, got {other:?}"),
        }
    }
}

#[test]
fn windows_classifies_privilege_denial_from_the_exit_code_alone() {
    // shutdown.exe returns the Win32 error as its exit status. With an empty
    // stderr, the code is the only signal available.
    for code in [1314, 5] {
        match classify_windows(&failed(code, "")) {
            Some(AppError::PowerOffPrivilegeDenied { .. }) => {}
            other => panic!("expected privilege denial for exit {code}, got {other:?}"),
        }
    }
}

#[test]
fn windows_classifies_an_unrelated_failure_as_a_plain_failure() {
    match classify_windows(&failed(1, "Unable to abort the system shutdown because no shutdown was in progress.(1116)")) {
        Some(AppError::PowerOffFailed { .. }) => {}
        other => panic!("expected a plain failure, got {other:?}"),
    }
}

// ---- Linux ----

#[test]
fn linux_classifies_polkit_denial() {
    let cases = [
        "Call to PowerOff failed: Access denied",
        "Failed to power off system via logind: Interactive authentication required.",
        "Call to PowerOff failed: Permission denied",
        "polkit: Authorization not available.",
    ];

    for stderr in cases {
        match classify_linux(&failed(1, stderr)) {
            Some(AppError::PowerOffPolicyDenied { .. }) => {}
            other => panic!("expected a policy denial for {stderr:?}, got {other:?}"),
        }
    }
}

#[test]
fn linux_classifies_an_unrelated_failure_as_a_plain_failure() {
    match classify_linux(&failed(1, "Failed to connect to bus: No such file or directory")) {
        Some(AppError::PowerOffFailed { message }) => {
            assert!(message.contains("bus"));
        }
        other => panic!("expected a plain failure, got {other:?}"),
    }
}

// ---- cross-OS ----

#[test]
fn each_os_reports_its_own_denial_variant() {
    // The three denials are genuinely different situations and the UI says
    // different things about each: consent can be granted in System Settings, a
    // Windows privilege is usually held already, and a polkit policy refusal is
    // not the user's to change. Collapsing them would lose that.
    let denial = failed(1, "access denied");

    assert!(matches!(
        classify_macos(&denial),
        Some(AppError::PowerOffConsentDenied { .. })
    ));
    assert!(matches!(
        classify_windows(&denial),
        Some(AppError::PowerOffPrivilegeDenied { .. })
    ));
    assert!(matches!(
        classify_linux(&denial),
        Some(AppError::PowerOffPolicyDenied { .. })
    ));
}

#[test]
fn classification_is_case_insensitive() {
    // Wording and casing vary across versions; the classification must not.
    assert!(matches!(
        classify_macos(&failed(1, "NOT AUTHORIZED TO SEND APPLE EVENTS")),
        Some(AppError::PowerOffConsentDenied { .. })
    ));
    assert!(matches!(
        classify_windows(&failed(1, "ACCESS IS DENIED")),
        Some(AppError::PowerOffPrivilegeDenied { .. })
    ));
    assert!(matches!(
        classify_linux(&failed(1, "INTERACTIVE AUTHENTICATION REQUIRED")),
        Some(AppError::PowerOffPolicyDenied { .. })
    ));
}

#[test]
fn no_classifier_ever_returns_unsupported() {
    // PowerOffUnsupported means "categorically impossible on this platform", which
    // is never true of a desktop OS. A denial must not be reported as impossibility:
    // the user could fix a denial, and would be told not to bother.
    for (name, classify) in ALL {
        for outcome in [
            failed(1, "access denied"),
            failed(1, "not authorized"),
            failed(1, "anything at all"),
            failed(1314, ""),
        ] {
            assert!(
                !matches!(classify(&outcome), Some(AppError::PowerOffUnsupported)),
                "{name} reported a desktop denial as categorically unsupported"
            );
        }
    }
}

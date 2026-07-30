//! Tests for the IPC surface.
//!
//! `#[tauri::command]` functions take a `State<'_, AppState>`, which cannot be
//! constructed without a running app, so the command bodies themselves are thin
//! wrappers around scheduler calls that `application::scheduler_tests` already covers.
//! What is tested here is what those tests cannot reach: the boundary rules that hold
//! across the whole module.

use crate::commands::dto::{CreateJobResponse, TriggerInput};
use crate::domain::{JobType, TriggerSpec};

/// The source of the command module, read at compile time.
///
/// Checking a structural rule against the source is unusual, and it is here because
/// task 11.4 is a claim about the whole file rather than about any one function: no
/// command may reach a power-off directly. A reviewer can verify that by reading, but
/// a reader six months from now adding a command cannot, and this fails their build.
const COMMANDS_SOURCE: &str = include_str!("mod.rs");

#[test]
fn no_command_can_reach_a_power_off_executor() {
    // Task 11.4. If a command ever holds an executor, the 60-second countdown becomes
    // optional, which is the one thing this app must not allow.
    for forbidden in [
        "PowerOffExecutor",
        "power_off()",
        "host_executor",
        "run_shutdown_command",
    ] {
        let uses: Vec<&str> = COMMANDS_SOURCE
            .lines()
            .filter(|line| line.contains(forbidden))
            .filter(|line| {
                // Doc comments naming the type are the point of the module docs.
                let trimmed = line.trim_start();
                !trimmed.starts_with("//") && !trimmed.starts_with("*")
            })
            .collect();

        assert!(
            uses.is_empty(),
            "a command references {forbidden}, which would bypass the grace period: {uses:?}"
        );
    }
}

#[test]
fn the_only_grace_related_command_can_only_cancel_a_shutdown() {
    // There is a command to stop a countdown and none to start one. A "power off now"
    // command is exactly the shortcut that would make the countdown skippable.
    assert!(COMMANDS_SOURCE.contains("pub fn cancel_grace_period"));

    for forbidden in ["pub fn power_off", "pub fn shutdown_now", "pub fn skip_grace"] {
        assert!(
            !COMMANDS_SOURCE.contains(forbidden),
            "found {forbidden}, which would let the UI shut down without a countdown"
        );
    }
}

#[test]
fn no_command_resolves_a_trigger_by_hand() {
    // Task 11.2 in the other direction: resolution must go through TriggerResolver, not
    // through date arithmetic written inline, or the rules diverge from the shared
    // vectors that pin them down.
    let arithmetic: Vec<&str> = COMMANDS_SOURCE
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            !trimmed.starts_with("//")
                && (line.contains("Duration::minutes") || line.contains("Duration::hours"))
        })
        .collect();

    assert!(
        arithmetic.is_empty(),
        "a command computes an instant itself instead of asking the resolver: {arithmetic:?}"
    );
}

#[test]
fn every_command_reports_errors_as_a_readable_message() {
    // A command returning `Result<_, AppError>` would serialise the variant and leave
    // the UI deciding what to say, in JavaScript, away from the code that knows.
    assert!(
        !COMMANDS_SOURCE.contains("-> Result<") || COMMANDS_SOURCE.contains("CommandResult<"),
        "commands should use the CommandResult alias so the error type stays a string"
    );
    assert!(COMMANDS_SOURCE.contains("error.user_message()"));
}

#[test]
fn creating_and_replacing_are_separate_commands() {
    // Task 10.9. A `replace: bool` parameter would make the destructive path reachable
    // by a forgotten argument or a default.
    assert!(COMMANDS_SOURCE.contains("pub fn create_job("));
    assert!(COMMANDS_SOURCE.contains("pub fn create_job_replacing("));

    // Comments are skipped: the doc on `create_job_replacing` explains why the flag was
    // rejected, and naming it there is the point.
    let flag: Vec<&str> = COMMANDS_SOURCE
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .filter(|line| line.contains("replace: bool"))
        .collect();

    assert!(
        flag.is_empty(),
        "replacement must be a separate command, not a flag: {flag:?}"
    );
}

#[test]
fn a_trigger_input_converts_to_the_domain_spec_without_loss() {
    assert_eq!(
        TriggerSpec::from(TriggerInput::Indefinite),
        TriggerSpec::Indefinite
    );
    assert_eq!(
        TriggerSpec::from(TriggerInput::Duration { minutes: 90 }),
        TriggerSpec::Duration { minutes: 90 }
    );
    assert_eq!(
        TriggerSpec::from(TriggerInput::AbsoluteTime { hour: 6, minute: 15 }),
        TriggerSpec::AbsoluteTime { hour: 6, minute: 15 }
    );
}

#[test]
fn an_unknown_job_type_is_rejected_with_a_readable_message() {
    let error = super::parse_job_type("sleep_forever").expect_err("not a job type");

    assert!(error.contains("sleep_forever"), "got: {error}");
}

#[test]
fn both_job_types_parse_from_the_strings_the_ui_sends() {
    assert_eq!(
        super::parse_job_type(JobType::KeepAwake.as_str()).unwrap(),
        JobType::KeepAwake
    );
    assert_eq!(
        super::parse_job_type(JobType::PowerOff.as_str()).unwrap(),
        JobType::PowerOff
    );
}

#[test]
fn the_confirmation_response_serialises_with_a_discriminant_the_ui_can_switch_on() {
    // An untagged union would force the UI to guess which case it received.
    let json = serde_json::to_value(CreateJobResponse::NeedsConfirmation {
        job_type: "power_off".to_string(),
        existing: Box::new(crate::commands::dto::JobView {
            id: "job-1".to_string(),
            job_type: "power_off".to_string(),
            job_type_label: "Shut down".to_string(),
            status: "active".to_string(),
            status_label: "Running".to_string(),
            trigger_label: "For 30 min".to_string(),
            target_instant_utc: None,
            remaining_seconds: None,
            timezone: "UTC".to_string(),
            failure_message: None,
            is_active: true,
        }),
        message: "already scheduled".to_string(),
    })
    .unwrap();

    assert_eq!(json["result"], "needsConfirmation");
    assert_eq!(json["existing"]["id"], "job-1");
}

#[test]
fn the_host_timezone_is_offered_first_and_only_once() {
    // A duplicate would appear twice in the picker; burying the host's own zone in a
    // 600-entry list is the other half of the same problem.
    let zones = super::available_timezones();
    let host = crate::data::default_timezone();

    assert_eq!(zones.first().map(String::as_str), Some(host.as_str()));
    assert_eq!(
        zones.iter().filter(|zone| *zone == &host).count(),
        1,
        "the host timezone appears twice"
    );
    assert!(zones.len() > 100, "the full IANA list should be offered");
}

#[test]
fn the_grace_length_reported_to_the_ui_is_the_one_the_gate_enforces() {
    // Two constants would let the UI draw a 30-second countdown over a 60-second wait.
    assert_eq!(
        super::grace_period_length(),
        crate::application::grace_period::GRACE_PERIOD_SECONDS
    );
}

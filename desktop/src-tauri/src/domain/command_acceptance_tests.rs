//! Tests for the command acceptance rule.
//!
//! Kept in a separate file rather than in a `mod tests` inside `command_acceptance.rs`, for
//! the same reason `remote_command_tests.rs` and `commands/tests.rs` are separate: the
//! structural test below greps the module's source for forbidden names, and those names
//! would otherwise appear in the very file being checked and fail it against itself.

use std::collections::HashSet;

use chrono::{DateTime, Duration, TimeZone, Utc};

use crate::domain::command_acceptance::{
    evaluate_command, CommandAcceptance, CommandTargetState, RejectionReason,
    FRESHNESS_WINDOW_SECONDS, FUTURE_TOLERANCE_SECONDS, NONCE_RETENTION_SECONDS,
};
use crate::domain::command_envelope::CommandEnvelope;
use crate::domain::device_id::DeviceId;
use crate::domain::remote_command::RemoteCommand;

/// The source of the command acceptance module, read at compile time.
const COMMAND_ACCEPTANCE_SOURCE: &str = include_str!("command_acceptance.rs");

fn created() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 8, 6, 12, 0, 0).unwrap()
}

/// An envelope that is authentic and well-formed, so each test can spoil exactly one thing
/// and attribute the refusal to it.
fn envelope(command: RemoteCommand) -> CommandEnvelope {
    CommandEnvelope {
        sender: DeviceId::new("phone-a").expect("non-empty"),
        command,
        created_at: created(),
        nonce: "n-fresh".to_string(),
        signature_verified: true,
    }
}

/// A target state that permits everything, for the same reason.
fn permissive_target() -> CommandTargetState {
    CommandTargetState {
        seen_nonces: HashSet::new(),
        target_can_power_off: true,
        target_is_remote_target: true,
        is_paired: true,
        remote_control_enabled: true,
    }
}

// ---------------------------------------------------------------------------
// The structural guarantee. This is the test that must never be weakened.
// ---------------------------------------------------------------------------

#[test]
fn no_acceptance_path_can_reach_a_power_off() {
    // The mirror of `no_remote_command_type_can_reach_a_power_off`, and it exists for the
    // same reason: privacy would not catch this. The gate is `pub` within the crate, so an
    // acceptance handler added here could legally call it, and the mandatory countdown
    // would become optional for exactly the caller who is not at the machine.
    //
    // Establishing that a command is genuine must not shorten, skip, or bypass the
    // countdown. An accepted command reaches the machine as a job, and by no other route.
    for forbidden in [
        "power_off(",
        "PowerOffGate",
        "PowerOffExecutor",
        "GraceOutcome",
    ] {
        let uses: Vec<&str> = COMMAND_ACCEPTANCE_SOURCE
            .lines()
            .filter(|line| line.contains(forbidden))
            .collect();

        assert!(
            uses.is_empty(),
            "the command acceptance module references {forbidden}, which would let a \
             remote caller bypass the countdown: {uses:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// The no-probing requirement.
// ---------------------------------------------------------------------------

#[test]
fn an_unverified_signature_is_refused_without_reporting_any_permission_reason() {
    // The spec's no-probing requirement, and the reason authenticity is checked first: a
    // caller must not be able to learn the target's permission state by sending
    // unauthenticated commands. Here every permission check would also fail, and none of
    // that may surface.
    let unverified = CommandEnvelope {
        signature_verified: false,
        ..envelope(RemoteCommand::PowerOff)
    };
    let hostile_target = CommandTargetState {
        target_can_power_off: false,
        target_is_remote_target: false,
        is_paired: false,
        remote_control_enabled: false,
        ..permissive_target()
    };

    let decision = evaluate_command(&unverified, &hostile_target, created());

    assert_eq!(
        decision,
        CommandAcceptance::Rejected(RejectionReason::AuthenticityUnverified)
    );
    // Stated as its own assertion rather than implied by the equality above: the property
    // is that no permission reason is reported, and a future reason set could grow.
    assert_ne!(
        decision.rejection_reason(),
        Some(RejectionReason::NotPermitted),
        "an unauthenticated caller must not learn the target's permission state"
    );
}

#[test]
fn an_unverified_signature_hides_replay_and_freshness_state_too() {
    // Every reason after the first leaks something about the target. A caller must not be
    // able to discover which nonces this target has seen, either.
    let unverified = CommandEnvelope {
        signature_verified: false,
        nonce: "n-seen".to_string(),
        ..envelope(RemoteCommand::PowerOff)
    };
    let target = CommandTargetState {
        seen_nonces: HashSet::from(["n-seen".to_string()]),
        ..permissive_target()
    };

    assert_eq!(
        evaluate_command(&unverified, &target, created()),
        CommandAcceptance::Rejected(RejectionReason::AuthenticityUnverified)
    );
}

// ---------------------------------------------------------------------------
// The precedence order. Each case holds two or more reasons at once.
// ---------------------------------------------------------------------------

#[test]
fn replay_is_reported_before_either_time_reason() {
    // A replayed command is evidence of an attack; a stale one is more often a bad network.
    // Reporting the more serious finding when both hold means the record shows an attack as
    // an attack.
    let target = CommandTargetState {
        seen_nonces: HashSet::from(["n-fresh".to_string()]),
        ..permissive_target()
    };

    // Replayed and stale.
    assert_eq!(
        evaluate_command(
            &envelope(RemoteCommand::PowerOff),
            &target,
            created() + Duration::seconds(FRESHNESS_WINDOW_SECONDS + 60)
        ),
        CommandAcceptance::Rejected(RejectionReason::ReplayedNonce)
    );

    // Replayed and future-dated.
    assert_eq!(
        evaluate_command(
            &envelope(RemoteCommand::PowerOff),
            &target,
            created() - Duration::seconds(FUTURE_TOLERANCE_SECONDS + 60)
        ),
        CommandAcceptance::Rejected(RejectionReason::ReplayedNonce)
    );
}

#[test]
fn future_dating_and_staleness_are_each_reported_before_permission() {
    // notPermitted is last because it leaks the most — whether remote control is enabled
    // and what the platform can do — and is reached only by a command already authentic,
    // fresh, and new.
    let forbidden_target = CommandTargetState {
        remote_control_enabled: false,
        ..permissive_target()
    };

    assert_eq!(
        evaluate_command(
            &envelope(RemoteCommand::PowerOff),
            &forbidden_target,
            created() - Duration::seconds(FUTURE_TOLERANCE_SECONDS + 1)
        ),
        CommandAcceptance::Rejected(RejectionReason::FutureDated)
    );

    assert_eq!(
        evaluate_command(
            &envelope(RemoteCommand::PowerOff),
            &forbidden_target,
            created() + Duration::seconds(FRESHNESS_WINDOW_SECONDS + 1)
        ),
        CommandAcceptance::Rejected(RejectionReason::Stale)
    );
}

#[test]
fn every_reason_holding_at_once_still_reports_authenticity() {
    let worst = CommandEnvelope {
        signature_verified: false,
        nonce: "n-seen".to_string(),
        command: RemoteCommand::EnableRemoteControl,
        ..envelope(RemoteCommand::PowerOff)
    };
    let worst_target = CommandTargetState {
        seen_nonces: HashSet::from(["n-seen".to_string()]),
        target_can_power_off: false,
        target_is_remote_target: false,
        is_paired: false,
        remote_control_enabled: false,
    };

    assert_eq!(
        evaluate_command(
            &worst,
            &worst_target,
            created() - Duration::seconds(FUTURE_TOLERANCE_SECONDS + 60)
        ),
        CommandAcceptance::Rejected(RejectionReason::AuthenticityUnverified)
    );
}

// ---------------------------------------------------------------------------
// The boundaries. Each is asserted from both sides.
// ---------------------------------------------------------------------------

#[test]
fn a_command_at_exactly_the_freshness_window_is_accepted() {
    // Inclusive-accept, and asserted against the constant rather than a literal so changing
    // the constant moves the test with it.
    assert_eq!(
        evaluate_command(
            &envelope(RemoteCommand::PowerOff),
            &permissive_target(),
            created() + Duration::seconds(FRESHNESS_WINDOW_SECONDS)
        ),
        CommandAcceptance::Accepted
    );
}

#[test]
fn a_command_one_second_past_the_freshness_window_is_stale() {
    assert_eq!(
        evaluate_command(
            &envelope(RemoteCommand::PowerOff),
            &permissive_target(),
            created() + Duration::seconds(FRESHNESS_WINDOW_SECONDS + 1)
        ),
        CommandAcceptance::Rejected(RejectionReason::Stale)
    );
}

#[test]
fn a_command_at_exactly_the_future_tolerance_is_accepted() {
    // Ordinary clock skew between two honest devices must not refuse a valid command.
    assert_eq!(
        evaluate_command(
            &envelope(RemoteCommand::PowerOff),
            &permissive_target(),
            created() - Duration::seconds(FUTURE_TOLERANCE_SECONDS)
        ),
        CommandAcceptance::Accepted
    );
}

#[test]
fn a_command_one_second_past_the_future_tolerance_is_future_dated_not_stale() {
    // The two indicate different faults, and a user shown "expired" for a clock-skew
    // problem will spend the evening looking in the wrong place.
    assert_eq!(
        evaluate_command(
            &envelope(RemoteCommand::PowerOff),
            &permissive_target(),
            created() - Duration::seconds(FUTURE_TOLERANCE_SECONDS + 1)
        ),
        CommandAcceptance::Rejected(RejectionReason::FutureDated)
    );
}

// ---------------------------------------------------------------------------
// Replay.
// ---------------------------------------------------------------------------

#[test]
fn a_seen_nonce_is_refused_and_a_distinct_one_from_the_same_device_is_not() {
    // Without the second half, an implementation could refuse every command from a device
    // that had ever sent one and still pass the replay case.
    let target = CommandTargetState {
        seen_nonces: HashSet::from(["n-one".to_string(), "n-two".to_string()]),
        ..permissive_target()
    };

    let replayed = CommandEnvelope {
        nonce: "n-two".to_string(),
        ..envelope(RemoteCommand::PowerOff)
    };
    assert_eq!(
        evaluate_command(&replayed, &target, created()),
        CommandAcceptance::Rejected(RejectionReason::ReplayedNonce)
    );

    let distinct = CommandEnvelope {
        nonce: "n-three".to_string(),
        ..envelope(RemoteCommand::PowerOff)
    };
    assert_eq!(
        evaluate_command(&distinct, &target, created()),
        CommandAcceptance::Accepted
    );
}

// ---------------------------------------------------------------------------
// Permission is delegated, not reimplemented.
// ---------------------------------------------------------------------------

#[test]
fn every_remotely_permitted_command_is_accepted_when_nothing_is_wrong() {
    for command in [
        RemoteCommand::PowerOff,
        RemoteCommand::KeepAwake,
        RemoteCommand::CancelJob,
    ] {
        assert_eq!(
            evaluate_command(&envelope(command), &permissive_target(), created()),
            CommandAcceptance::Accepted,
            "{command:?} should be accepted"
        );
    }
}

#[test]
fn the_permission_question_is_delegated_and_its_specific_reason_is_not_disclosed() {
    // Acceptance reports only that the command was not permitted. The authorization rule
    // keeps its own reason set for the question it answers; collapsing them here is what
    // keeps the two decisions separable.
    for target in [
        CommandTargetState {
            remote_control_enabled: false,
            ..permissive_target()
        },
        CommandTargetState {
            is_paired: false,
            ..permissive_target()
        },
        CommandTargetState {
            target_can_power_off: false,
            ..permissive_target()
        },
    ] {
        assert_eq!(
            evaluate_command(&envelope(RemoteCommand::PowerOff), &target, created()),
            CommandAcceptance::Rejected(RejectionReason::NotPermitted)
        );
    }

    // And a command no target accepts remotely, however permissive everything else is.
    assert_eq!(
        evaluate_command(
            &envelope(RemoteCommand::EnableRemoteControl),
            &permissive_target(),
            created()
        ),
        CommandAcceptance::Rejected(RejectionReason::NotPermitted)
    );
}

// ---------------------------------------------------------------------------
// The decision type, the constants, and the messages.
// ---------------------------------------------------------------------------

#[test]
fn a_decision_carries_a_reason_exactly_when_it_is_a_refusal() {
    assert_eq!(CommandAcceptance::Accepted.rejection_reason(), None);
    assert!(CommandAcceptance::Accepted.is_accepted());

    let rejected = CommandAcceptance::Rejected(RejectionReason::Stale);
    assert_eq!(rejected.rejection_reason(), Some(RejectionReason::Stale));
    assert!(!rejected.is_accepted());
}

/// The retention relationship, restated here so this file fails to compile too if it is ever
/// broken — the module's own `const _` is the primary guard.
///
/// A compile-time `const _` rather than an `assert!` inside a `#[test]`: clippy rejects a
/// runtime assertion whose operands are all constants (`assertions_on_constants`), and it is
/// right to — the check belongs at compile time, where the mistake cannot be built rather than
/// merely reported by a test run someone might not do.
const _: () = assert!(
    NONCE_RETENTION_SECONDS > FRESHNESS_WINDOW_SECONDS + FUTURE_TOLERANCE_SECONDS,
    "nonce retention must outlast the window in which a command can still be accepted"
);

#[test]
fn freshness_is_not_coupled_to_the_presence_threshold() {
    // The two answer different questions — "was this issued just now" versus "is this device
    // still there" — and reusing one constant for both would make either unchangeable
    // without the other.
    assert_ne!(
        FRESHNESS_WINDOW_SECONDS,
        crate::domain::presence::ONLINE_THRESHOLD_SECONDS
    );
}

#[test]
fn every_rejection_reason_has_a_distinct_message_a_person_can_act_on() {
    let messages: Vec<String> = [
        RejectionReason::AuthenticityUnverified,
        RejectionReason::ReplayedNonce,
        RejectionReason::FutureDated,
        RejectionReason::Stale,
        RejectionReason::NotPermitted,
    ]
    .into_iter()
    .map(RejectionReason::user_message)
    .collect();

    for message in &messages {
        assert!(!message.trim().is_empty());
        assert!(
            message.chars().any(char::is_lowercase),
            "messages are prose, not identifiers: {message}"
        );
    }

    let unique: std::collections::HashSet<&String> = messages.iter().collect();
    assert_eq!(unique.len(), messages.len(), "two reasons share a message");
}

#[test]
fn reason_strings_round_trip_and_match_the_dart_wire_names() {
    // These exact strings appear in the shared vectors, so they are a cross-language
    // contract rather than an internal detail.
    assert_eq!(
        RejectionReason::AuthenticityUnverified.as_str(),
        "authenticityUnverified"
    );
    assert_eq!(RejectionReason::ReplayedNonce.as_str(), "replayedNonce");
    assert_eq!(RejectionReason::FutureDated.as_str(), "futureDated");
    assert_eq!(RejectionReason::Stale.as_str(), "stale");
    assert_eq!(RejectionReason::NotPermitted.as_str(), "notPermitted");

    for reason in [
        RejectionReason::AuthenticityUnverified,
        RejectionReason::ReplayedNonce,
        RejectionReason::FutureDated,
        RejectionReason::Stale,
        RejectionReason::NotPermitted,
    ] {
        assert_eq!(
            RejectionReason::from_str_value(reason.as_str()),
            Some(reason)
        );
    }
    assert_eq!(RejectionReason::from_str_value("nonsense"), None);
}

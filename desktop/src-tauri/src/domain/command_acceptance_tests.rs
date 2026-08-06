//! Tests for the command acceptance rule.
//!
//! Kept in a separate file rather than in a `mod tests` inside `command_acceptance.rs`, for
//! the same reason `remote_command_tests.rs` and `commands/tests.rs` are separate: the
//! structural test below greps the module's source for forbidden names, and those names
//! would otherwise appear in the very file being checked and fail it against itself.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Duration, TimeZone, Utc};

use crate::domain::command_acceptance::{
    evaluate_command, CommandAcceptance, CommandTargetState, RejectionReason,
    FRESHNESS_WINDOW_SECONDS, FUTURE_TOLERANCE_SECONDS, NONCE_RETENTION_SECONDS,
};
use crate::domain::command_envelope::CommandEnvelope;
use crate::domain::device_id::DeviceId;
use crate::domain::remote_command::RemoteCommand;
use crate::domain::signature::SIGNATURE_BYTES;
use crate::domain::test_signing::TestKeyPair;

/// The source of the command acceptance module, read at compile time.
const COMMAND_ACCEPTANCE_SOURCE: &str = include_str!("command_acceptance.rs");

fn created() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 8, 6, 12, 0, 0).unwrap()
}

fn sender() -> DeviceId {
    DeviceId::new("phone-a").expect("non-empty")
}

/// The key the sending device signs with, and whose public half the target holds.
fn sender_key() -> TestKeyPair {
    TestKeyPair::from_seed(1)
}

/// An envelope that is authentic and well-formed, so each test can spoil exactly one thing
/// and attribute the refusal to it.
///
/// The signature is **real**: produced by signing the canonical encoding with a real key,
/// and checked by the rule with real arithmetic. It is not a placeholder and there is no
/// helper here that makes verification succeed without it. Before this change these tests
/// set `signature_verified: true`, which is precisely the shortcut the change exists to
/// remove — reintroducing it as a test helper would leave every test below passing against
/// an implementation that verified nothing.
fn envelope(command: RemoteCommand) -> CommandEnvelope {
    signed_envelope(command, "n-fresh", created(), &sender_key())
}

/// An envelope signed by a given key, over exactly the fields it carries.
///
/// Every mutation in the tests below goes through here rather than editing a field of an
/// already-signed envelope, so that a case meaning "a valid command with a different nonce"
/// carries a signature valid for *that* nonce. Editing a signed field in place would break
/// the signature and make the test pass for the wrong reason.
fn signed_envelope(
    command: RemoteCommand,
    nonce: &str,
    created_at: DateTime<Utc>,
    key: &TestKeyPair,
) -> CommandEnvelope {
    let sender = sender();
    let signature = key.sign_command(&sender, command.as_str(), created_at, nonce);

    CommandEnvelope {
        sender,
        command,
        created_at,
        nonce: nonce.to_string(),
        signature,
    }
}

/// A target state that permits everything, for the same reason.
fn permissive_target() -> CommandTargetState {
    CommandTargetState {
        seen_nonces: HashSet::new(),
        verifying_keys: HashMap::from([(sender(), sender_key().verifying_key())]),
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
        signature: vec![0x42; SIGNATURE_BYTES],
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
        signature: vec![0x42; SIGNATURE_BYTES],
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
// Authenticity is verified, not asserted.
// ---------------------------------------------------------------------------

#[test]
fn a_command_signed_by_the_wrong_key_is_refused_though_everything_else_is_valid() {
    // The property the previous change specified and could not enforce. This command is
    // fresh, unreplayed, and permitted; only the key differs. Before verification was real,
    // a caller could have set a boolean and had this obeyed.
    let attacker = TestKeyPair::from_seed(9);
    let forged = signed_envelope(RemoteCommand::PowerOff, "n-fresh", created(), &attacker);

    // The target holds the *genuine* sender's key under that device id.
    assert_eq!(
        evaluate_command(&forged, &permissive_target(), created()),
        CommandAcceptance::Rejected(RejectionReason::AuthenticityUnverified)
    );
}

#[test]
fn a_sender_the_target_holds_no_key_for_is_refused_on_authenticity_grounds() {
    // Not a distinct reason, deliberately. Distinguishing "I hold no key for that device"
    // from "the signature did not verify" would tell an attacker which device identifiers
    // this target knows, which is the probing the precedence order exists to prevent.
    let target = CommandTargetState {
        verifying_keys: HashMap::new(),
        ..permissive_target()
    };

    assert_eq!(
        evaluate_command(&envelope(RemoteCommand::PowerOff), &target, created()),
        CommandAcceptance::Rejected(RejectionReason::AuthenticityUnverified)
    );
}

#[test]
fn an_unknown_sender_and_a_bad_signature_are_indistinguishable() {
    // The two failure modes must be reported identically, or the difference is an oracle.
    let unknown_sender_target = CommandTargetState {
        verifying_keys: HashMap::new(),
        ..permissive_target()
    };
    let bad_signature = CommandEnvelope {
        signature: vec![0x42; SIGNATURE_BYTES],
        ..envelope(RemoteCommand::PowerOff)
    };

    assert_eq!(
        evaluate_command(
            &envelope(RemoteCommand::PowerOff),
            &unknown_sender_target,
            created()
        ),
        evaluate_command(&bad_signature, &permissive_target(), created())
    );
}

#[test]
fn a_key_held_for_a_different_device_does_not_authenticate_this_sender() {
    // A target paired with several devices must check the claimed sender's key
    // specifically. Verifying against any key it holds would let one paired device issue
    // commands in another's name.
    let other_device = DeviceId::new("phone-b").expect("non-empty");
    let target = CommandTargetState {
        verifying_keys: HashMap::from([(other_device, sender_key().verifying_key())]),
        ..permissive_target()
    };

    assert_eq!(
        evaluate_command(&envelope(RemoteCommand::PowerOff), &target, created()),
        CommandAcceptance::Rejected(RejectionReason::AuthenticityUnverified)
    );
}

#[test]
fn altering_any_signed_field_after_signing_invalidates_the_command() {
    // Every field the signature covers, varied one at a time against a valid baseline. A
    // field that survives alteration is a field a relay could rewrite in transit — the
    // creation instant to revive a stale command, the nonce to make a replay look new, the
    // command to turn a keep-awake into a shutdown.
    let valid = envelope(RemoteCommand::PowerOff);
    assert_eq!(
        evaluate_command(&valid, &permissive_target(), created()),
        CommandAcceptance::Accepted,
        "the baseline must be accepted or the mutations below prove nothing"
    );

    let target_with_both_keys = CommandTargetState {
        verifying_keys: HashMap::from([
            (sender(), sender_key().verifying_key()),
            (
                DeviceId::new("phone-b").expect("non-empty"),
                sender_key().verifying_key(),
            ),
        ]),
        ..permissive_target()
    };

    // The sender. Signed as phone-a, presented as phone-b — and the target holds a key for
    // phone-b too, so this fails on the signature rather than on a missing key.
    let altered_sender = CommandEnvelope {
        sender: DeviceId::new("phone-b").expect("non-empty"),
        ..valid.clone()
    };
    assert_eq!(
        evaluate_command(&altered_sender, &target_with_both_keys, created()),
        CommandAcceptance::Rejected(RejectionReason::AuthenticityUnverified),
        "the sender is not covered by the signature"
    );

    // The command.
    let altered_command = CommandEnvelope {
        command: RemoteCommand::KeepAwake,
        ..valid.clone()
    };
    assert_eq!(
        evaluate_command(&altered_command, &permissive_target(), created()),
        CommandAcceptance::Rejected(RejectionReason::AuthenticityUnverified),
        "the command is not covered by the signature"
    );

    // The creation instant. Moved by one second, well inside the freshness window, so a
    // refusal can only come from the signature.
    let altered_created_at = CommandEnvelope {
        created_at: created() + Duration::seconds(1),
        ..valid.clone()
    };
    assert_eq!(
        evaluate_command(
            &altered_created_at,
            &permissive_target(),
            created() + Duration::seconds(1)
        ),
        CommandAcceptance::Rejected(RejectionReason::AuthenticityUnverified),
        "the creation instant is not covered by the signature"
    );

    // The nonce. Unseen by the target, so replay cannot be the reason.
    let altered_nonce = CommandEnvelope {
        nonce: "n-altered".to_string(),
        ..valid
    };
    assert_eq!(
        evaluate_command(&altered_nonce, &permissive_target(), created()),
        CommandAcceptance::Rejected(RejectionReason::AuthenticityUnverified),
        "the nonce is not covered by the signature"
    );
}

#[test]
fn a_malformed_signature_is_refused_rather_than_panicking() {
    // These bytes are attacker-controlled: a relay puts whatever it likes here. Every shape
    // must produce a refusal, and none may crash the target.
    for signature in [
        vec![],
        vec![0u8; 1],
        vec![0u8; SIGNATURE_BYTES - 1],
        vec![0u8; SIGNATURE_BYTES],
        vec![0u8; SIGNATURE_BYTES + 1],
        vec![0xffu8; 200],
    ] {
        let malformed = CommandEnvelope {
            signature,
            ..envelope(RemoteCommand::PowerOff)
        };

        assert_eq!(
            evaluate_command(&malformed, &permissive_target(), created()),
            CommandAcceptance::Rejected(RejectionReason::AuthenticityUnverified)
        );
    }
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
        signature: vec![0x42; SIGNATURE_BYTES],
        nonce: "n-seen".to_string(),
        command: RemoteCommand::EnableRemoteControl,
        ..envelope(RemoteCommand::PowerOff)
    };
    let worst_target = CommandTargetState {
        seen_nonces: HashSet::from(["n-seen".to_string()]),
        verifying_keys: HashMap::from([(sender(), sender_key().verifying_key())]),
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

    // Each is signed over its own nonce. Editing the nonce of an already-signed envelope
    // would invalidate the signature, and both cases would then be refused for authenticity
    // rather than for the reason they exist to check.
    let replayed = signed_envelope(RemoteCommand::PowerOff, "n-two", created(), &sender_key());
    assert_eq!(
        evaluate_command(&replayed, &target, created()),
        CommandAcceptance::Rejected(RejectionReason::ReplayedNonce)
    );

    let distinct = signed_envelope(RemoteCommand::PowerOff, "n-three", created(), &sender_key());
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

//! Tests for the remote command authorization rule.
//!
//! Kept in a separate file rather than in a `mod tests` inside `remote_command.rs`, for
//! the same reason `commands/tests.rs` is separate: the structural test below greps the
//! module's source for forbidden names, and the names it greps for would otherwise appear
//! in the very file being checked and fail against itself.

use crate::domain::remote_command::{
    authorize, DenialReason, RemoteCommand, RemoteCommandContext, RemoteCommandDecision,
};

/// The source of the remote command module, read at compile time.
const REMOTE_COMMAND_SOURCE: &str = include_str!("remote_command.rs");

/// A context that is authorized in every respect, so each test can spoil exactly one
/// thing and attribute the refusal to it.
fn permissive(command: RemoteCommand) -> RemoteCommandContext {
    RemoteCommandContext {
        command,
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
fn no_remote_command_type_can_reach_a_power_off() {
    // The mirror of `no_command_can_reach_a_power_off_executor`, and it exists for the
    // same reason that one does: privacy would not catch this. The gate is `pub` within
    // the crate, so a remote handler added here could legally call it, and the mandatory
    // countdown would become optional for exactly the caller who is not at the machine.
    //
    // An authorized remote command must reach the machine as a job, and by no other
    // route. Source text is a blunt instrument — an alias defeats it — but it stops the
    // accident, which is the failure mode that actually happens.
    for forbidden in [
        "power_off(",
        "PowerOffGate",
        "PowerOffExecutor",
        "GraceOutcome",
    ] {
        let uses: Vec<&str> = REMOTE_COMMAND_SOURCE
            .lines()
            .filter(|line| line.contains(forbidden))
            .collect();

        assert!(
            uses.is_empty(),
            "the remote command module references {forbidden}, which would let a remote \
             caller bypass the countdown: {uses:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// The precedence order. Each case holds two or more reasons at once.
// ---------------------------------------------------------------------------

#[test]
fn a_command_that_is_never_remote_is_refused_before_anything_else_is_considered() {
    // Everything else is permissive, so only the command itself can be the reason.
    assert_eq!(
        authorize(&permissive(RemoteCommand::EnableRemoteControl)),
        RemoteCommandDecision::Denied(DenialReason::CommandNotRemotelyAllowed)
    );
}

#[test]
fn the_command_check_wins_over_every_other_reason_at_once() {
    // All four reasons hold. The first in the order surfaces, so the refusal discloses
    // nothing about the target beyond the command being forbidden.
    let context = RemoteCommandContext {
        command: RemoteCommand::EnableRemoteControl,
        target_can_power_off: false,
        target_is_remote_target: false,
        is_paired: false,
        remote_control_enabled: false,
    };

    assert_eq!(
        authorize(&context),
        RemoteCommandDecision::Denied(DenialReason::CommandNotRemotelyAllowed)
    );
}

#[test]
fn a_disabled_setting_is_reported_before_a_missing_pairing() {
    let context = RemoteCommandContext {
        is_paired: false,
        remote_control_enabled: false,
        ..permissive(RemoteCommand::PowerOff)
    };

    assert_eq!(
        authorize(&context),
        RemoteCommandDecision::Denied(DenialReason::RemoteControlDisabled)
    );
}

#[test]
fn a_missing_pairing_is_reported_before_what_the_target_can_do() {
    // Capability is checked last on purpose: an unpaired caller must not be able to
    // learn what the target is capable of by reading the refusal it gets back.
    let context = RemoteCommandContext {
        target_can_power_off: false,
        target_is_remote_target: false,
        is_paired: false,
        ..permissive(RemoteCommand::PowerOff)
    };

    assert_eq!(
        authorize(&context),
        RemoteCommandDecision::Denied(DenialReason::NotPaired)
    );
}

// ---------------------------------------------------------------------------
// Each reason on its own.
// ---------------------------------------------------------------------------

#[test]
fn an_unpaired_requester_is_refused_with_the_pairing_reason() {
    // Same account is not sufficient, and a revoked pairing lands here too: the rule
    // takes "is this device paired" as an input rather than performing a lookup, so
    // revocation needs no separate branch.
    let context = RemoteCommandContext {
        is_paired: false,
        ..permissive(RemoteCommand::PowerOff)
    };

    assert_eq!(
        authorize(&context),
        RemoteCommandDecision::Denied(DenialReason::NotPaired)
    );
}

#[test]
fn remote_control_being_off_is_its_own_reason() {
    let context = RemoteCommandContext {
        remote_control_enabled: false,
        ..permissive(RemoteCommand::PowerOff)
    };

    assert_eq!(
        authorize(&context),
        RemoteCommandDecision::Denied(DenialReason::RemoteControlDisabled)
    );
}

#[test]
fn a_target_that_cannot_power_off_is_refused_for_capability_not_permission() {
    // The distinction the spec insists on: "this machine cannot do that" must not be
    // reported as "you are not allowed to", or the user goes looking for a setting that
    // would not help.
    let context = RemoteCommandContext {
        target_can_power_off: false,
        ..permissive(RemoteCommand::PowerOff)
    };

    assert_eq!(
        authorize(&context),
        RemoteCommandDecision::Denied(DenialReason::PlatformCannotPerform)
    );
}

#[test]
fn a_device_that_cannot_be_a_remote_target_refuses_every_command() {
    // A phone. The OS suspends the app, so a command sent to it could not be received
    // reliably — refused explicitly rather than accepted and silently dropped.
    for command in [
        RemoteCommand::PowerOff,
        RemoteCommand::KeepAwake,
        RemoteCommand::CancelJob,
    ] {
        let context = RemoteCommandContext {
            target_is_remote_target: false,
            target_can_power_off: false,
            ..permissive(command)
        };

        assert_eq!(
            authorize(&context),
            RemoteCommandDecision::Denied(DenialReason::PlatformCannotPerform),
            "{command:?} must be refused by a device that cannot be a target"
        );
    }
}

// ---------------------------------------------------------------------------
// What is accepted.
// ---------------------------------------------------------------------------

#[test]
fn a_paired_enabled_capable_desktop_accepts_every_permitted_command() {
    for command in [
        RemoteCommand::PowerOff,
        RemoteCommand::KeepAwake,
        RemoteCommand::CancelJob,
    ] {
        assert_eq!(
            authorize(&permissive(command)),
            RemoteCommandDecision::Allowed,
            "{command:?} should be accepted"
        );
    }
}

#[test]
fn the_capability_check_is_per_command_rather_than_a_blanket_gate() {
    // Cancelling asks nothing of the shutdown path, so a target that cannot power off
    // still accepts it. A single `target_can_power_off` gate over all commands would
    // refuse this and pass every other test in this file.
    let context = RemoteCommandContext {
        target_can_power_off: false,
        ..permissive(RemoteCommand::CancelJob)
    };

    assert_eq!(authorize(&context), RemoteCommandDecision::Allowed);
    assert!(!RemoteCommand::CancelJob.requires_power_off_capability());
    assert!(!RemoteCommand::KeepAwake.requires_power_off_capability());
    assert!(RemoteCommand::PowerOff.requires_power_off_capability());
}

#[test]
fn enabling_remote_control_is_refused_however_permissive_everything_else_is() {
    // The spec's reason, restated as a test: without this, access to one account would
    // be enough to turn remote control on everywhere and then power off every device on
    // it. Physical presence is the thing account access cannot forge.
    assert!(!RemoteCommand::EnableRemoteControl.is_remotely_permitted());
    assert!(!authorize(&permissive(RemoteCommand::EnableRemoteControl)).is_allowed());
}

// ---------------------------------------------------------------------------
// The decision type and its messages.
// ---------------------------------------------------------------------------

#[test]
fn a_decision_carries_a_reason_exactly_when_it_is_a_refusal() {
    // The nonsense states a `(bool, Option<reason>)` pair would admit, asserted absent.
    assert_eq!(RemoteCommandDecision::Allowed.denial_reason(), None);
    assert!(RemoteCommandDecision::Allowed.is_allowed());

    let denied = RemoteCommandDecision::Denied(DenialReason::NotPaired);
    assert_eq!(denied.denial_reason(), Some(DenialReason::NotPaired));
    assert!(!denied.is_allowed());
}

#[test]
fn every_denial_reason_has_a_distinct_message_a_person_can_act_on() {
    // A generic "refused" is indistinguishable from a bug, and two reasons sharing a
    // message would make them indistinguishable from each other.
    let messages: Vec<String> = [
        DenialReason::NotPaired,
        DenialReason::RemoteControlDisabled,
        DenialReason::PlatformCannotPerform,
        DenialReason::CommandNotRemotelyAllowed,
    ]
    .into_iter()
    .map(DenialReason::user_message)
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
fn the_disabled_message_says_where_the_setting_can_be_turned_on() {
    // Naming the target device is the whole point: a user told only "remote control is
    // off" will look for the switch on the phone in their hand, where it is not.
    let message = DenialReason::RemoteControlDisabled.user_message();

    assert!(message.contains("turned on at that device"), "got: {message}");
}

#[test]
fn reason_and_command_strings_round_trip_and_match_the_dart_wire_names() {
    // These exact strings appear in the shared vectors, so they are a cross-language
    // contract rather than an internal detail.
    assert_eq!(DenialReason::NotPaired.as_str(), "notPaired");
    assert_eq!(
        DenialReason::RemoteControlDisabled.as_str(),
        "remoteControlDisabled"
    );
    assert_eq!(
        DenialReason::PlatformCannotPerform.as_str(),
        "platformCannotPerform"
    );
    assert_eq!(
        DenialReason::CommandNotRemotelyAllowed.as_str(),
        "commandNotRemotelyAllowed"
    );

    for reason in [
        DenialReason::NotPaired,
        DenialReason::RemoteControlDisabled,
        DenialReason::PlatformCannotPerform,
        DenialReason::CommandNotRemotelyAllowed,
    ] {
        assert_eq!(DenialReason::from_str_value(reason.as_str()), Some(reason));
    }
    assert_eq!(DenialReason::from_str_value("nonsense"), None);

    for command in [
        RemoteCommand::PowerOff,
        RemoteCommand::KeepAwake,
        RemoteCommand::CancelJob,
        RemoteCommand::EnableRemoteControl,
    ] {
        assert_eq!(RemoteCommand::from_str_value(command.as_str()), Some(command));
    }
    assert_eq!(RemoteCommand::from_str_value("nonsense"), None);
}

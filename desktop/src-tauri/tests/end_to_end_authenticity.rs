//! The end-to-end tests the previous change named as its blocker.
//!
//! The archived design (D2) recorded that the acceptance rule took `signature_verified` as a
//! caller-supplied boolean, and stated the condition for closing that gap: the change that
//! introduces a transport must verify signatures against a key held only by the paired
//! devices, and must carry an end-to-end test that an invalid signature is rejected. This
//! file is that test.
//!
//! # Why these run through the transport
//!
//! Every case here sends through [`FakeTransport`] and judges what comes out the other side,
//! rather than calling `verify` directly. Calling the verifier proves the verifier works; it
//! does not prove the path from an arriving envelope to a decision actually consults it. The
//! hole being closed was a *composition* failure — a rule that could be handed the answer —
//! so a test that skips the composition would not have caught it.
//!
//! # Why the transport misbehaves
//!
//! A fake that only behaves proves the happy path and nothing else. The four faults below
//! are the ones the design depends on surviving: dropped, delayed, duplicated, reordered.
//! What the fake cannot do is forge, and that is now arithmetic rather than policy.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Duration, TimeZone, Utc};

use weakup_lib::application::{FakeTransport, RemoteTransport, TransportFaults};
use weakup_lib::data::{PairingStore, SqliteJobRepository};
use weakup_lib::domain::{
    evaluate_command, CommandAcceptance, CommandEnvelope, CommandTargetState, DeviceId,
    RejectionReason, RemoteCommand, VerifyingKey, FRESHNESS_WINDOW_SECONDS,
};

mod signing_support;
use signing_support::TestKeyPair;

fn created() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 8, 6, 12, 0, 0).unwrap()
}

fn phone() -> DeviceId {
    DeviceId::new("phone-a").expect("non-empty")
}

fn desktop() -> DeviceId {
    DeviceId::new("desktop-a").expect("non-empty")
}

/// An envelope signed by `key`, over exactly the fields it carries.
fn signed(
    key: &TestKeyPair,
    command: RemoteCommand,
    nonce: &str,
    created_at: DateTime<Utc>,
) -> CommandEnvelope {
    let sender = phone();
    let signature = key.sign_command(&sender, command.as_str(), created_at, nonce);

    CommandEnvelope {
        sender,
        command,
        created_at,
        nonce: nonce.to_string(),
        signature,
    }
}

/// A target that permits everything and holds `key` for the phone.
fn target_holding(key: &VerifyingKey) -> CommandTargetState {
    CommandTargetState {
        seen_nonces: HashSet::new(),
        verifying_keys: HashMap::from([(phone(), key.clone())]),
        target_can_power_off: true,
        target_is_remote_target: true,
        is_paired: true,
        remote_control_enabled: true,
    }
}

/// Sends an envelope and judges whatever the transport delivers.
///
/// The full path: sign, hand to the transport, take what comes out, evaluate. The `now` a
/// target judges by is its own, which is what makes the delayed case bite.
fn round_trip(
    transport: &FakeTransport,
    envelope: &CommandEnvelope,
    target_state: &CommandTargetState,
    now: DateTime<Utc>,
) -> Vec<CommandAcceptance> {
    transport
        .send_envelope(&desktop(), envelope)
        .expect("transport accepts the envelope");

    transport
        .receive_envelopes(&desktop())
        .expect("receive")
        .iter()
        .map(|delivered| evaluate_command(delivered, target_state, now))
        .collect()
}

// ---------------------------------------------------------------------------
// The blocker: a command signed by the wrong key, travelling the full path.
// ---------------------------------------------------------------------------

#[test]
fn a_command_signed_with_one_key_is_refused_by_a_target_holding_another() {
    // Key A signs; the target holds key B for that sender. Everything else about this
    // command is in order — fresh, unseen, permitted — so authenticity is the only thing
    // that can refuse it. Before this change the sender could have set a boolean instead.
    let attacker = TestKeyPair::from_seed(9);
    let genuine = TestKeyPair::from_seed(1);

    let transport = FakeTransport::new();
    let envelope = signed(&attacker, RemoteCommand::PowerOff, "n-1", created());
    let target = target_holding(&genuine.verifying_key());

    let decisions = round_trip(&transport, &envelope, &target, created());

    assert_eq!(
        decisions,
        vec![CommandAcceptance::Rejected(
            RejectionReason::AuthenticityUnverified
        )]
    );
}

#[test]
fn a_command_signed_with_the_key_the_target_holds_is_accepted() {
    // The positive counterpart. Without it, an implementation that refused everything would
    // pass the test above — and a system that never obeys a command is not the goal.
    let genuine = TestKeyPair::from_seed(1);

    let transport = FakeTransport::new();
    let envelope = signed(&genuine, RemoteCommand::PowerOff, "n-1", created());
    let target = target_holding(&genuine.verifying_key());

    let decisions = round_trip(&transport, &envelope, &target, created());

    assert_eq!(decisions, vec![CommandAcceptance::Accepted]);
}

#[test]
fn a_relay_that_swaps_the_signature_cannot_make_a_command_obeyed() {
    // The property the whole architecture rests on: the relay's cooperation is not
    // sufficient. Here the relay substitutes a signature of its own making, which is the
    // most it can do, and the target refuses.
    let genuine = TestKeyPair::from_seed(1);
    let hostile_relay = TestKeyPair::from_seed(7);

    let transport = FakeTransport::new();
    let mut envelope = signed(&genuine, RemoteCommand::PowerOff, "n-1", created());
    envelope.signature = hostile_relay.sign_command(
        &phone(),
        RemoteCommand::PowerOff.as_str(),
        created(),
        "n-1",
    );

    let decisions = round_trip(
        &transport,
        &envelope,
        &target_holding(&genuine.verifying_key()),
        created(),
    );

    assert_eq!(
        decisions,
        vec![CommandAcceptance::Rejected(
            RejectionReason::AuthenticityUnverified
        )]
    );
}

// ---------------------------------------------------------------------------
// The four faults a transport is assumed capable of.
// ---------------------------------------------------------------------------

#[test]
fn a_duplicated_command_is_obeyed_once() {
    // A retry after an ambiguous timeout. The transport delivers the same signed bytes
    // twice; the signature verifies both times, because it is genuinely the same command.
    // What stops the second is the nonce — and this exercises that through the full path
    // rather than by calling the replay check directly.
    let genuine = TestKeyPair::from_seed(1);
    let transport = FakeTransport::with_faults(TransportFaults {
        duplicate_delivery: true,
        ..TransportFaults::default()
    });

    let envelope = signed(&genuine, RemoteCommand::PowerOff, "n-dup", created());
    let mut target = target_holding(&genuine.verifying_key());

    transport
        .send_envelope(&desktop(), &envelope)
        .expect("send");
    let delivered = transport.receive_envelopes(&desktop()).expect("receive");
    assert_eq!(delivered.len(), 2, "the fake must actually duplicate");

    // The target remembers each nonce it acts on, which is what makes the second refusal
    // possible. Recording it here rather than inside the rule keeps the rule pure.
    let mut decisions = Vec::new();
    for envelope in &delivered {
        let decision = evaluate_command(envelope, &target, created());
        if decision.is_accepted() {
            target.seen_nonces.insert(envelope.nonce.clone());
        }
        decisions.push(decision);
    }

    assert_eq!(
        decisions,
        vec![
            CommandAcceptance::Accepted,
            CommandAcceptance::Rejected(RejectionReason::ReplayedNonce),
        ]
    );
}

#[test]
fn a_command_delayed_past_the_freshness_window_is_refused_as_stale() {
    // A backgrounded app, or a relay holding a message. The command's creation instant does
    // not advance while it waits, which is exactly why the freshness rule catches it — and
    // why a power-off cannot arrive an hour late and still be obeyed.
    let genuine = TestKeyPair::from_seed(1);
    let transport = FakeTransport::with_faults(TransportFaults {
        delay_delivery: true,
        ..TransportFaults::default()
    });

    let envelope = signed(&genuine, RemoteCommand::PowerOff, "n-slow", created());
    let target = target_holding(&genuine.verifying_key());

    transport
        .send_envelope(&desktop(), &envelope)
        .expect("send");
    assert!(
        transport
            .receive_envelopes(&desktop())
            .expect("receive")
            .is_empty(),
        "a delayed envelope must not arrive yet"
    );

    transport.release_delayed();
    let late = created() + Duration::seconds(FRESHNESS_WINDOW_SECONDS + 1);
    let decisions: Vec<CommandAcceptance> = transport
        .receive_envelopes(&desktop())
        .expect("receive")
        .iter()
        .map(|delivered| evaluate_command(delivered, &target, late))
        .collect();

    assert_eq!(
        decisions,
        vec![CommandAcceptance::Rejected(RejectionReason::Stale)]
    );
}

#[test]
fn two_reordered_commands_are_each_judged_on_their_own_merits() {
    // Two commands racing through different paths. Arriving out of order is not itself a
    // fault: each carries its own signature and its own nonce, so each stands alone. A rule
    // that required ordering would refuse valid commands for a reason the sender cannot
    // control.
    let genuine = TestKeyPair::from_seed(1);
    let transport = FakeTransport::with_faults(TransportFaults {
        reorder_delivery: true,
        ..TransportFaults::default()
    });

    let first = signed(&genuine, RemoteCommand::KeepAwake, "n-first", created());
    let second = signed(&genuine, RemoteCommand::CancelJob, "n-second", created());
    let target = target_holding(&genuine.verifying_key());

    transport.send_envelope(&desktop(), &first).expect("send");
    transport.send_envelope(&desktop(), &second).expect("send");

    let delivered = transport.receive_envelopes(&desktop()).expect("receive");
    assert_eq!(
        delivered.iter().map(|e| e.nonce.as_str()).collect::<Vec<_>>(),
        vec!["n-second", "n-first"],
        "the fake must actually reorder"
    );

    for envelope in &delivered {
        assert_eq!(
            evaluate_command(envelope, &target, created()),
            CommandAcceptance::Accepted,
            "{} should be judged on its own merits",
            envelope.nonce
        );
    }
}

#[test]
fn a_dropped_command_leaves_the_target_untouched() {
    // An ordinary mobile network. The sender is told nothing is wrong — it cannot tell —
    // and the target simply never hears. Nothing is obeyed and no state moves.
    let genuine = TestKeyPair::from_seed(1);
    let transport = FakeTransport::with_faults(TransportFaults {
        drop_everything: true,
        ..TransportFaults::default()
    });

    let envelope = signed(&genuine, RemoteCommand::PowerOff, "n-lost", created());
    let target = target_holding(&genuine.verifying_key());
    let before = target.clone();

    transport
        .send_envelope(&desktop(), &envelope)
        .expect("the transport accepts it and discards it");

    let delivered = transport.receive_envelopes(&desktop()).expect("receive");

    assert!(delivered.is_empty(), "a dropped command must not arrive");
    assert_eq!(transport.queued_count(&desktop()), 0);
    assert_eq!(target, before, "no decision, no state change");
}

// ---------------------------------------------------------------------------
// Tampering, field by field.
// ---------------------------------------------------------------------------

#[test]
fn altering_any_one_signed_field_in_transit_invalidates_the_command() {
    // A relay's remaining powers, tried one at a time. Each of the four signed fields is
    // something a relay would gain from changing: the command to escalate a keep-awake into
    // a shutdown, the creation instant to revive a stale command, the nonce to replay one,
    // the sender to attribute it elsewhere. All four must fail.
    let genuine = TestKeyPair::from_seed(1);
    let transport = FakeTransport::new();

    let target = CommandTargetState {
        // Both devices' keys are the same genuine key, so an altered *sender* fails on the
        // signature rather than on a missing entry — which is the thing being tested.
        verifying_keys: HashMap::from([
            (phone(), genuine.verifying_key()),
            (
                DeviceId::new("phone-b").expect("non-empty"),
                genuine.verifying_key(),
            ),
        ]),
        ..target_holding(&genuine.verifying_key())
    };

    let pristine = signed(&genuine, RemoteCommand::KeepAwake, "n-tamper", created());

    // Confirm the untampered command is accepted, or the four mutations below prove nothing.
    assert_eq!(
        round_trip(&transport, &pristine, &target, created()),
        vec![CommandAcceptance::Accepted]
    );

    let mutations: Vec<(&str, CommandEnvelope, DateTime<Utc>)> = vec![
        (
            "sender",
            CommandEnvelope {
                sender: DeviceId::new("phone-b").expect("non-empty"),
                ..pristine.clone()
            },
            created(),
        ),
        (
            "command",
            CommandEnvelope {
                command: RemoteCommand::PowerOff,
                ..pristine.clone()
            },
            created(),
        ),
        (
            "created_at",
            CommandEnvelope {
                created_at: created() + Duration::seconds(1),
                ..pristine.clone()
            },
            // Judged at the altered instant, so the command is fresh and only the signature
            // can refuse it.
            created() + Duration::seconds(1),
        ),
        (
            "nonce",
            CommandEnvelope {
                nonce: "n-rewritten".to_string(),
                ..pristine.clone()
            },
            created(),
        ),
    ];

    for (field, tampered, now) in mutations {
        assert_eq!(
            round_trip(&transport, &tampered, &target, now),
            vec![CommandAcceptance::Rejected(
                RejectionReason::AuthenticityUnverified
            )],
            "altering {field} in transit must invalidate the signature"
        );
    }
}

// ---------------------------------------------------------------------------
// The keys come from the pairing store, and revocation reaches the full path.
// ---------------------------------------------------------------------------

/// The target's state, with the key map read from a real pairing store rather than built by
/// this test.
///
/// This is the difference the change makes. Every case above constructs `verifying_keys` by
/// hand, which is exactly what production code no longer does: the map now comes from the
/// pairings the user established, so authenticity answers to the user's decisions rather
/// than to whatever a caller passed.
fn target_from_store(store: &SqliteJobRepository) -> CommandTargetState {
    CommandTargetState {
        seen_nonces: HashSet::new(),
        verifying_keys: store
            .verifying_keys_from_store()
            .expect("the store yields a key map"),
        target_can_power_off: true,
        target_is_remote_target: true,
        is_paired: true,
        remote_control_enabled: true,
    }
}

#[test]
fn a_paired_peers_command_is_accepted_and_a_revoked_ones_is_refused() {
    // The whole point of this change, end to end and in one test so the two halves cannot
    // drift apart: the *same* device, the *same* key, the *same* signing path, differing
    // only in whether the pairing is still in force.
    let peer_key = TestKeyPair::from_seed(1);
    let transport = FakeTransport::new();
    let store = SqliteJobRepository::open_in_memory().expect("in-memory database");

    // Pairing is what confers authority. Before this line the target holds no key for the
    // phone at all.
    store
        .record_pairing(&phone(), &peer_key.verifying_key(), created())
        .expect("pair");

    let accepted = round_trip(
        &transport,
        &signed(&peer_key, RemoteCommand::PowerOff, "n-paired", created()),
        &target_from_store(&store),
        created(),
    );
    assert_eq!(
        accepted,
        vec![CommandAcceptance::Accepted],
        "a command from a paired peer, signed with the key recorded at pairing, must be \
         obeyed — otherwise the store is not supplying keys at all"
    );

    // The owner revokes, locally and with no network involved.
    store
        .revoke_pairing(&phone(), created() + Duration::seconds(1))
        .expect("revoke");

    // The same flow again. A fresh nonce, so replay cannot be what refuses it — the refusal
    // must come from the revocation and nothing else.
    let refused = round_trip(
        &transport,
        &signed(&peer_key, RemoteCommand::PowerOff, "n-revoked", created()),
        &target_from_store(&store),
        created(),
    );
    assert_eq!(
        refused,
        vec![CommandAcceptance::Rejected(
            RejectionReason::AuthenticityUnverified
        )],
        "after revocation the peer's key must be absent from the map, so its command fails \
         as an unknown sender rather than being verified against a de-authorized key"
    );
}

#[test]
fn a_never_paired_device_is_refused_even_with_a_perfectly_valid_signature() {
    // The signature is genuine; the target simply never paired with this device. An empty
    // store must authorize nobody — an implementation defaulting to permissive when it holds
    // no pairings would obey every command it received.
    let stranger = TestKeyPair::from_seed(1);
    let transport = FakeTransport::new();
    let store = SqliteJobRepository::open_in_memory().expect("in-memory database");

    let decisions = round_trip(
        &transport,
        &signed(&stranger, RemoteCommand::PowerOff, "n-1", created()),
        &target_from_store(&store),
        created(),
    );

    assert_eq!(
        decisions,
        vec![CommandAcceptance::Rejected(
            RejectionReason::AuthenticityUnverified
        )]
    );
}

#[test]
fn re_pairing_a_revoked_device_restores_its_authority_end_to_end() {
    // Revocation withdraws authority; it does not blacklist. A user who revokes a phone
    // after mislaying it must be able to pair it again when it turns up, and its commands
    // must work again — through the full path, not merely in the store's own tests.
    let peer_key = TestKeyPair::from_seed(1);
    let transport = FakeTransport::new();
    let store = SqliteJobRepository::open_in_memory().expect("in-memory database");

    store
        .record_pairing(&phone(), &peer_key.verifying_key(), created())
        .expect("pair");
    store
        .revoke_pairing(&phone(), created() + Duration::seconds(1))
        .expect("revoke");
    store
        .record_pairing(
            &phone(),
            &peer_key.verifying_key(),
            created() + Duration::seconds(2),
        )
        .expect("re-pair");

    let decisions = round_trip(
        &transport,
        &signed(&peer_key, RemoteCommand::PowerOff, "n-repaired", created()),
        &target_from_store(&store),
        created(),
    );

    assert_eq!(decisions, vec![CommandAcceptance::Accepted]);
}

#[test]
fn revoking_one_peer_does_not_disturb_another() {
    // Revocation is per pairing. An implementation treating it as a global switch would pass
    // the single-peer cases above and silently de-authorize every device the user still
    // trusts.
    let phone_key = TestKeyPair::from_seed(1);
    let other_key = TestKeyPair::from_seed(2);
    let other = DeviceId::new("laptop-b").expect("non-empty");

    let transport = FakeTransport::new();
    let store = SqliteJobRepository::open_in_memory().expect("in-memory database");
    store
        .record_pairing(&phone(), &phone_key.verifying_key(), created())
        .expect("pair phone");
    store
        .record_pairing(&other, &other_key.verifying_key(), created())
        .expect("pair laptop");

    store
        .revoke_pairing(&phone(), created() + Duration::seconds(1))
        .expect("revoke phone");

    let keys = store.verifying_keys_from_store().expect("keys");
    assert!(!keys.contains_key(&phone()), "the revoked peer is absent");
    assert_eq!(
        keys.get(&other),
        Some(&other_key.verifying_key()),
        "the peer that was not revoked keeps its authority"
    );

    // And the still-paired peer's commands are still obeyed through the full path.
    let envelope = {
        let signature =
            other_key.sign_command(&other, RemoteCommand::PowerOff.as_str(), created(), "n-other");
        CommandEnvelope {
            sender: other.clone(),
            command: RemoteCommand::PowerOff,
            created_at: created(),
            nonce: "n-other".to_string(),
            signature,
        }
    };
    assert_eq!(
        round_trip(&transport, &envelope, &target_from_store(&store), created()),
        vec![CommandAcceptance::Accepted]
    );
}

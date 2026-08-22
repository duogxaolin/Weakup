//! The full arc: two devices pair each other, one signs a command, and the other schedules a
//! job for it with the countdown its origin demands.
//!
//! # Why this file is separate from `end_to_end_authenticity.rs`
//!
//! That suite proves the acceptance rule cannot be talked out of a refusal, and it builds its
//! pairings by calling `record_pairing` directly. That is the right shape for what it tests —
//! a vector case must be writable without an exchange — but it leaves one composition
//! unproven: whether the *pairing flow* actually produces pairings the acceptance rule will
//! honour.
//!
//! Every test here goes through [`accept_pairing_at_issuer`] and
//! [`complete_pairing_at_requester`], the same two functions the Tauri commands call. A test
//! that fabricated a pairing row would prove that a hand-written row works, which nobody
//! doubted, while saying nothing about the flow that creates one. If the flow recorded the
//! wrong key, recorded it under the wrong identifier, or recorded only one side, the tests in
//! the neighbouring file would all still pass.
//!
//! # Why it ends at a job rather than at a decision
//!
//! `evaluate_command` returning `Accepted` is not the property the user cares about. The
//! property is that an accepted remote power-off waits five minutes before it happens. That
//! spans acceptance, job creation, and the countdown, and each of those is a place where the
//! 300 could quietly become 60. So the arc is followed to the end: pair, sign, transport,
//! evaluate, schedule, and then read the countdown the scheduler would actually wait.
//!
//! # The countdown is read from the job, never from a constant this file chose
//!
//! [`remote_countdown_for`] calls [`grace_period_seconds`] with the job's own
//! [`JobOrigin`]. It does **not** compare against `REMOTE_GRACE_PERIOD_SECONDS` as a literal
//! and does not mention 300. That is deliberate and it is the difference between a test that
//! checks the countdown and one that merely agrees with itself: if the origin-to-length
//! mapping were changed to hand a remote job the local 60 seconds, this file must go red.
//! Task 8.4 requires that be demonstrated by trying it, not assumed.

use std::collections::HashSet;

use chrono::{DateTime, Duration, TimeZone, Utc};

use weakup_lib::application::{
    accept_pairing_at_issuer, complete_pairing_at_requester, grace_period_seconds, FakeTransport,
    PairingCode, PairingCodeIssuer, PairingIdentity, PairingOutcome, PairingResponse,
    RemoteTransport, TransportFaults, GRACE_PERIOD_SECONDS,
};
use weakup_lib::data::{JobRepository, PairingStore, SqliteJobRepository};
use weakup_lib::domain::{
    authorize, evaluate_command, CommandAcceptance, CommandEnvelope, CommandTargetState,
    DenialReason, DeviceId, GrantDelivery, Job, JobOrigin, JobStatus, JobType, RejectionReason,
    RemoteCommand, RemoteCommandContext, RemoteCommandDecision, TriggerSpec,
    FRESHNESS_WINDOW_SECONDS,
};

mod signing_support;
use signing_support::TestKeyPair;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn at(seconds: i64) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 8, 7, 12, 0, 0).unwrap() + Duration::seconds(seconds)
}

fn phone_id() -> DeviceId {
    DeviceId::new("phone-a").expect("non-empty")
}

fn desktop_id() -> DeviceId {
    DeviceId::new("desktop-a").expect("non-empty")
}

/// One device: its identity, the keypair it signs with, and the pairings it holds.
///
/// A struct rather than loose locals so a test cannot accidentally check the phone's store for
/// the desktop's pairing — the mistake that makes a one-sided pairing look like a paired one.
struct Device {
    identity: PairingIdentity,
    keys: TestKeyPair,
    store: SqliteJobRepository,
}

impl Device {
    fn new(id: DeviceId, seed: u8) -> Self {
        let keys = TestKeyPair::from_seed(seed);
        Self {
            identity: PairingIdentity {
                device_id: id,
                verifying_key: keys.verifying_key(),
            },
            keys,
            store: SqliteJobRepository::open_in_memory().expect("in-memory database"),
        }
    }

    /// Whether this device holds an *authorizing* pairing with `peer`. A revoked row confers
    /// nothing, so absence from the key map is the only sense of "not paired" that matters.
    fn authorizes(&self, peer: &DeviceId) -> bool {
        self.store
            .verifying_keys_from_store()
            .expect("key map")
            .contains_key(peer)
    }
}

// ---------------------------------------------------------------------------
// The real pairing path
// ---------------------------------------------------------------------------

/// Pairs two devices the way the product does: the target issues a code, the requester
/// presents it back, and each records the other.
///
/// Nothing here writes a pairing row directly. The code is drawn by the issuer, redeemed by
/// the issuer, and the requester records the issuer only on receipt of the issuer's
/// confirmation — design D7's ordering, exercised rather than described.
///
/// The `undo` closure is the compensating revocation. In the product it is a round trip to the
/// other device; here both stores are in this process, so it is a direct call. It is never
/// reached on the happy path, and a test that needed it would be testing something else.
fn pair_through_the_real_flow(
    issuer: &Device,
    requester: &Device,
    now: DateTime<Utc>,
) -> PairingOutcome {
    let mut codes = PairingCodeIssuer::new();

    // Shown at the machine, which is the delivery path the desktop surface uses and the one
    // whose 300-second grant lifetime the pairing module reasons about.
    let code = codes
        .issue(GrantDelivery::AtMachine, now)
        .expect("a code is issued");

    // Through `parse` rather than used directly: that is what a typed code goes through, and
    // it is where normalisation happens. Skipping it would leave the transcription path
    // untested even though this test claims to walk the real one.
    let entered = PairingCode::parse(code.as_str()).expect("a freshly issued code parses");

    let response = accept_pairing_at_issuer(
        &mut codes,
        &issuer.store,
        &issuer.identity,
        &requester.identity,
        &entered,
        now,
    )
    .expect("the issuer's half completes");

    complete_pairing_at_requester(&requester.store, response, now, || {
        weakup_lib::application::withdraw_pairing(&issuer.store, &requester.identity.device_id, now)
    })
    .expect("the requester's half completes")
}

/// The target's state, assembled the way production assembles it.
///
/// The key map comes from the store, so it reflects the pairing the flow above established and
/// nothing else. `is_paired` is likewise derived from the store rather than passed as a
/// hopeful `true`: a test that hardcoded it would keep passing after revocation.
fn target_state(target: &Device, sender: &DeviceId, remote_control_enabled: bool) -> CommandTargetState {
    let verifying_keys = target.store.verifying_keys_from_store().expect("key map");

    CommandTargetState {
        seen_nonces: HashSet::new(),
        is_paired: verifying_keys.contains_key(sender),
        verifying_keys,
        target_can_power_off: true,
        target_is_remote_target: true,
        remote_control_enabled,
    }
}

/// An envelope signed by `sender` over exactly the fields it carries.
fn signed_by(
    sender: &Device,
    command: RemoteCommand,
    nonce: &str,
    created_at: DateTime<Utc>,
) -> CommandEnvelope {
    let signature = sender.keys.sign_command(
        &sender.identity.device_id,
        command.as_str(),
        created_at,
        nonce,
    );

    CommandEnvelope {
        sender: sender.identity.device_id.clone(),
        command,
        created_at,
        nonce: nonce.to_string(),
        signature,
    }
}

// ---------------------------------------------------------------------------
// The arriving-command path: evaluate, then schedule
// ---------------------------------------------------------------------------

/// What the target did with an arriving command.
#[derive(Debug)]
enum Handled {
    /// A job was created. Held as the job itself so the countdown can be read from its origin.
    Scheduled(Job),
    /// Refused, with the reason the acceptance rule gave.
    Refused(RejectionReason),
}

/// The target's side of an arriving command: judge it, and create a job if it stands.
///
/// The job is built with [`JobOrigin::Remote`] because that is what this call site knows and
/// what the countdown is selected by. `JobScheduler::create_job` hardcodes `JobOrigin::Local`
/// — its own comment says a remote path will supply its own origin rather than defaulting —
/// so the origin is set here, at the one place that knows the request came from a peer.
///
/// **No countdown is chosen here.** The length is not a parameter, is not stored on the job,
/// and is not passed to anything. It is derived later from `job.origin`, which is what keeps a
/// remote caller from being able to ask for less.
fn handle_arriving_command(
    target: &Device,
    envelope: &CommandEnvelope,
    state: &CommandTargetState,
    now: DateTime<Utc>,
) -> Handled {
    match evaluate_command(envelope, state, now) {
        CommandAcceptance::Rejected(reason) => Handled::Refused(reason),
        CommandAcceptance::Accepted => {
            let job = Job {
                id: format!("remote-{}", envelope.nonce),
                job_type: JobType::PowerOff,
                trigger: TriggerSpec::Duration { minutes: 1 },
                status: JobStatus::Active,
                target_instant_utc: Some(now + Duration::minutes(1)),
                created_at_utc: now,
                updated_at_utc: now,
                timezone: "UTC".to_string(),
                failure_message: None,
                origin: JobOrigin::Remote,
            };

            target.store.insert(&job).expect("the job is recorded");
            Handled::Scheduled(job)
        }
    }
}

/// The countdown the scheduler would actually wait for this job, taken from its origin.
///
/// This is the assertion 8.4 attacks. The length comes from [`grace_period_seconds`] applied
/// to `job.origin` — the same call `JobScheduler::tick` makes — so a change that mapped a
/// remote origin to the local length makes every caller of this function fail. Reading a
/// constant of this file's own choosing instead would make the test agree with itself no
/// matter what the product did.
fn countdown_for(job: &Job) -> u64 {
    grace_period_seconds(job.origin)
}

/// Sends through the fake and returns whatever it delivers.
fn deliver(transport: &FakeTransport, envelope: &CommandEnvelope) -> Vec<CommandEnvelope> {
    transport
        .send_envelope(&desktop_id(), envelope)
        .expect("the transport accepts the envelope");
    transport
        .receive_envelopes(&desktop_id())
        .expect("the transport delivers")
}

// ---------------------------------------------------------------------------
// 8.1 — the arc, end to end
// ---------------------------------------------------------------------------

#[test]
fn a_command_from_a_device_paired_through_the_real_flow_creates_a_remote_job_with_the_long_countdown()
{
    // The whole feature in one test. Two devices that have never met pair by code, the phone
    // signs a power-off, the fake transport carries it, and the desktop schedules a job that
    // will wait five minutes before doing anything irreversible.
    let phone = Device::new(phone_id(), 1);
    let desktop = Device::new(desktop_id(), 2);
    let transport = FakeTransport::new();

    // Before pairing the desktop holds no key for the phone, so the command below would be
    // refused for authenticity. Asserted rather than assumed: if it were somehow already
    // paired, the rest of this test would prove nothing about pairing.
    assert!(
        !desktop.authorizes(&phone.identity.device_id),
        "the desktop must start with no authority granted to the phone"
    );

    // The desktop shows a code; the phone types it back. This is the only act that confers
    // authority anywhere in this system.
    let outcome = pair_through_the_real_flow(&desktop, &phone, at(0));
    assert_eq!(
        outcome,
        PairingOutcome::Paired {
            peer: desktop.identity.device_id.clone()
        },
        "the exchange must report the desktop as the peer the phone is now paired with"
    );

    // Both sides, per design D7. Checking only one would pass against a half-recorded pairing,
    // which is the failure mode that leaves every command refused with no visible reason.
    assert!(
        desktop.authorizes(&phone.identity.device_id),
        "the desktop must hold the phone's key after the exchange"
    );
    assert!(
        phone.authorizes(&desktop.identity.device_id),
        "the phone must hold the desktop's key after the exchange"
    );

    let envelope = signed_by(&phone, RemoteCommand::PowerOff, "n-arc", at(1));
    let state = target_state(&desktop, &phone.identity.device_id, true);

    let delivered = deliver(&transport, &envelope);
    assert_eq!(delivered.len(), 1, "exactly one command was sent");

    let handled = handle_arriving_command(&desktop, &delivered[0], &state, at(1));

    let Handled::Scheduled(job) = handled else {
        panic!("a command from a paired peer must be scheduled, got {handled:?}");
    };

    // The job is remote-origin, and it is remote-origin *in the store* rather than only in the
    // value this test built. A field that were dropped on the way to the database would give a
    // job that survived a restart the 60-second countdown.
    assert_eq!(job.origin, JobOrigin::Remote);
    let reloaded = desktop
        .store
        .find(&job.id)
        .expect("find")
        .expect("the job was recorded");
    assert_eq!(
        reloaded.origin,
        JobOrigin::Remote,
        "the origin must survive the round trip through storage"
    );

    // And the countdown. Derived from the reloaded job's origin, so this asserts on what a
    // restarted app would wait, not on what this test happened to construct.
    assert_eq!(
        countdown_for(&reloaded),
        grace_period_seconds(JobOrigin::Remote),
        "a remotely created power-off must count down for the remote length"
    );

    // Stated the other way round as well, because the equality above would also hold if both
    // origins mapped to the same number. The remote wait must be strictly the longer one.
    assert!(
        countdown_for(&reloaded) > GRACE_PERIOD_SECONDS,
        "the remote countdown must exceed the local one, or a remote caller has gained a \
         shorter wait than the person standing at the machine"
    );
}

// ---------------------------------------------------------------------------
// 8.2 — pairing revoked first
// ---------------------------------------------------------------------------

#[test]
fn a_command_from_a_peer_whose_pairing_was_revoked_is_refused_for_authenticity() {
    // Same arc, one act inserted: the owner revokes the phone at the desktop before the
    // command arrives.
    //
    // The reason is `AuthenticityUnverified` rather than a pairing reason, and that is not an
    // approximation. Revoking removes the row from the key map entirely, so the desktop holds
    // *no* key for the phone and the sender is unknown to it. There is no de-authorized key to
    // check a signature against and no pairing check to reach. Design D5 chose absence over
    // present-and-rejected precisely so a revoked device gets no cryptographic work done on
    // its behalf and leaks nothing about what the target once knew.
    let phone = Device::new(phone_id(), 1);
    let desktop = Device::new(desktop_id(), 2);
    let transport = FakeTransport::new();

    pair_through_the_real_flow(&desktop, &phone, at(0));
    assert!(
        desktop.authorizes(&phone.identity.device_id),
        "the pairing must have taken effect, or the revocation below proves nothing"
    );

    desktop
        .store
        .revoke_pairing(&phone.identity.device_id, at(1))
        .expect("revoke");

    // A fresh nonce, so replay cannot be what refuses it, and remote control left enabled, so
    // permission cannot be either. Revocation must be the only thing standing in the way.
    let envelope = signed_by(&phone, RemoteCommand::PowerOff, "n-revoked", at(2));
    let state = target_state(&desktop, &phone.identity.device_id, true);

    let delivered = deliver(&transport, &envelope);
    let handled = handle_arriving_command(&desktop, &delivered[0], &state, at(2));

    match handled {
        Handled::Refused(reason) => assert_eq!(
            reason,
            RejectionReason::AuthenticityUnverified,
            "a revoked peer is absent from the key map, so its command fails as an unknown \
             sender rather than as an unpaired one"
        ),
        Handled::Scheduled(job) => {
            panic!("a revoked peer must not be able to schedule anything, got {job:?}")
        }
    }

    // No job was created. The refusal is not merely reported — nothing was written.
    assert!(
        desktop.store.list_all().expect("list").is_empty(),
        "a refused command must leave no job behind"
    );
}

// ---------------------------------------------------------------------------
// 8.3 — remote control disabled at the target
// ---------------------------------------------------------------------------

#[test]
fn a_command_from_a_paired_peer_is_refused_when_remote_control_is_disabled_at_the_target() {
    // The pairing is in force and the signature is genuine; the owner has simply turned remote
    // control off at this machine. That is the "not right now" switch, distinct from revoking.
    let phone = Device::new(phone_id(), 1);
    let desktop = Device::new(desktop_id(), 2);
    let transport = FakeTransport::new();

    pair_through_the_real_flow(&desktop, &phone, at(0));

    let envelope = signed_by(&phone, RemoteCommand::PowerOff, "n-disabled", at(1));
    let state = target_state(&desktop, &phone.identity.device_id, false);

    let delivered = deliver(&transport, &envelope);
    let handled = handle_arriving_command(&desktop, &delivered[0], &state, at(1));

    // `evaluate_command` collapses every permission refusal into `NotPermitted` on purpose:
    // reporting *which* permission failed to a remote sender would let it probe the target's
    // settings. So the outer reason is the deliberately uninformative one.
    match handled {
        Handled::Refused(reason) => assert_eq!(
            reason,
            RejectionReason::NotPermitted,
            "a disabled target must refuse, and must not disclose which permission failed to \
             the sender"
        ),
        Handled::Scheduled(job) => {
            panic!("a disabled target must not schedule anything, got {job:?}")
        }
    }

    // And the specific reason, at the layer that is allowed to know it. The owner at the
    // machine needs to be told remote control is off rather than a generic refusal, so
    // `authorize` names it — and this asserts the named reason is the disabled one rather than
    // any other denial that would also collapse to `NotPermitted`.
    assert_eq!(
        authorize(&RemoteCommandContext {
            command: envelope.command,
            target_can_power_off: state.target_can_power_off,
            target_is_remote_target: state.target_is_remote_target,
            is_paired: state.is_paired,
            remote_control_enabled: state.remote_control_enabled,
        }),
        RemoteCommandDecision::Denied(DenialReason::RemoteControlDisabled),
        "the reason available to the owner must be the disabled one specifically"
    );

    assert!(
        desktop.store.list_all().expect("list").is_empty(),
        "a refused command must leave no job behind"
    );

    // Disabling is not revoking. The pairing is untouched, so re-enabling needs no re-pairing.
    assert!(
        desktop.authorizes(&phone.identity.device_id),
        "disabling remote control must not revoke the pairing"
    );

    // Proven by turning it back on and sending again: the same peer, the same key, obeyed.
    let again = signed_by(&phone, RemoteCommand::PowerOff, "n-reenabled", at(2));
    let enabled = target_state(&desktop, &phone.identity.device_id, true);
    let delivered = deliver(&transport, &again);
    let last = delivered.last().expect("at least one delivery");

    assert!(
        matches!(
            handle_arriving_command(&desktop, last, &enabled, at(2)),
            Handled::Scheduled(_)
        ),
        "re-enabling remote control must restore the peer's authority without re-pairing"
    );
}

// ---------------------------------------------------------------------------
// 8.4 — the four transport faults, over a pairing the flow established
// ---------------------------------------------------------------------------

/*
 * The neighbouring suite already covers these four faults. Repeating them here is not
 * duplication: there, the key map is hand-built, and the question is whether the acceptance
 * rule handles the fault. Here the key map came from a real exchange, and the question is
 * whether that changes any of the four answers. It must not — a pairing established by code
 * has to behave exactly like one written directly, or the flow has introduced a difference
 * nobody asked for.
 */

#[test]
fn a_duplicated_command_from_a_paired_peer_is_obeyed_once() {
    // A retry after an ambiguous timeout. Both copies are genuinely the same signed bytes, so
    // both verify; the nonce is what stops the second, and a second job is what it prevents.
    let phone = Device::new(phone_id(), 1);
    let desktop = Device::new(desktop_id(), 2);
    let transport = FakeTransport::with_faults(TransportFaults {
        duplicate_delivery: true,
        ..TransportFaults::default()
    });

    pair_through_the_real_flow(&desktop, &phone, at(0));

    let envelope = signed_by(&phone, RemoteCommand::PowerOff, "n-dup", at(1));
    let mut state = target_state(&desktop, &phone.identity.device_id, true);

    let delivered = deliver(&transport, &envelope);
    assert_eq!(delivered.len(), 2, "the fake must actually duplicate");

    let mut outcomes = Vec::new();
    for copy in &delivered {
        let handled = handle_arriving_command(&desktop, copy, &state, at(1));
        if matches!(handled, Handled::Scheduled(_)) {
            // Recorded by the caller rather than inside the rule, which is what keeps the rule
            // pure — the same division `end_to_end_authenticity.rs` uses.
            state.seen_nonces.insert(copy.nonce.clone());
        }
        outcomes.push(handled);
    }

    assert!(
        matches!(outcomes[0], Handled::Scheduled(_)),
        "the first copy must be obeyed, got {:?}",
        outcomes[0]
    );
    assert!(
        matches!(outcomes[1], Handled::Refused(RejectionReason::ReplayedNonce)),
        "the second copy must be refused as a replay, got {:?}",
        outcomes[1]
    );

    // One job, not two. The refusal is only interesting because of what it prevented.
    assert_eq!(
        desktop.store.list_all().expect("list").len(),
        1,
        "a duplicated command must not produce two power-off jobs"
    );
}

#[test]
fn a_command_delayed_past_the_freshness_window_is_refused_as_stale() {
    // A relay holding a message, or a phone that went to sleep mid-send. The command's
    // creation instant does not advance while it waits, which is what the freshness rule
    // catches — and why a power-off cannot arrive an hour late and still be obeyed.
    let phone = Device::new(phone_id(), 1);
    let desktop = Device::new(desktop_id(), 2);
    let transport = FakeTransport::with_faults(TransportFaults {
        delay_delivery: true,
        ..TransportFaults::default()
    });

    pair_through_the_real_flow(&desktop, &phone, at(0));

    let envelope = signed_by(&phone, RemoteCommand::PowerOff, "n-slow", at(1));
    let state = target_state(&desktop, &phone.identity.device_id, true);

    transport
        .send_envelope(&desktop_id(), &envelope)
        .expect("send");
    assert!(
        transport
            .receive_envelopes(&desktop_id())
            .expect("receive")
            .is_empty(),
        "a delayed envelope must not arrive yet"
    );

    transport.release_delayed();
    let late = at(1) + Duration::seconds(FRESHNESS_WINDOW_SECONDS + 1);
    let delivered = transport
        .receive_envelopes(&desktop_id())
        .expect("receive after release");

    let handled = handle_arriving_command(&desktop, &delivered[0], &state, late);
    assert!(
        matches!(handled, Handled::Refused(RejectionReason::Stale)),
        "a command older than the freshness window must be refused as stale, got {handled:?}"
    );
    assert!(
        desktop.store.list_all().expect("list").is_empty(),
        "a stale command must leave no job behind"
    );
}

#[test]
fn two_reordered_commands_are_each_judged_on_their_own_merits() {
    // Arriving out of order is not itself a fault: each command carries its own signature and
    // its own nonce, so each stands alone. A rule that required ordering would refuse valid
    // commands for a reason the sender cannot control.
    let phone = Device::new(phone_id(), 1);
    let desktop = Device::new(desktop_id(), 2);
    let transport = FakeTransport::with_faults(TransportFaults {
        reorder_delivery: true,
        ..TransportFaults::default()
    });

    pair_through_the_real_flow(&desktop, &phone, at(0));
    let state = target_state(&desktop, &phone.identity.device_id, true);

    let first = signed_by(&phone, RemoteCommand::KeepAwake, "n-first", at(1));
    let second = signed_by(&phone, RemoteCommand::CancelJob, "n-second", at(1));

    transport
        .send_envelope(&desktop_id(), &first)
        .expect("send");
    transport
        .send_envelope(&desktop_id(), &second)
        .expect("send");

    let delivered = transport
        .receive_envelopes(&desktop_id())
        .expect("receive");
    assert_eq!(
        delivered
            .iter()
            .map(|e| e.nonce.as_str())
            .collect::<Vec<_>>(),
        vec!["n-second", "n-first"],
        "the fake must actually reorder"
    );

    for envelope in &delivered {
        assert_eq!(
            evaluate_command(envelope, &state, at(1)),
            CommandAcceptance::Accepted,
            "{} should be judged on its own merits regardless of arrival order",
            envelope.nonce
        );
    }
}

#[test]
fn a_dropped_command_leaves_the_target_with_nothing_scheduled() {
    // An ordinary mobile network. The sender is told nothing is wrong — it cannot tell — and
    // the target simply never hears. Nothing is obeyed and no job exists.
    let phone = Device::new(phone_id(), 1);
    let desktop = Device::new(desktop_id(), 2);
    let transport = FakeTransport::with_faults(TransportFaults {
        drop_everything: true,
        ..TransportFaults::default()
    });

    pair_through_the_real_flow(&desktop, &phone, at(0));

    let envelope = signed_by(&phone, RemoteCommand::PowerOff, "n-lost", at(1));

    transport
        .send_envelope(&desktop_id(), &envelope)
        .expect("the transport accepts it and discards it");

    assert!(
        transport
            .receive_envelopes(&desktop_id())
            .expect("receive")
            .is_empty(),
        "a dropped command must not arrive"
    );
    assert_eq!(transport.queued_count(&desktop_id()), 0);
    assert!(
        desktop.store.list_all().expect("list").is_empty(),
        "nothing arrived, so nothing may be scheduled"
    );

    // And the pairing is untouched. A dropped command is not evidence about the peer.
    assert!(
        desktop.authorizes(&phone.identity.device_id),
        "a dropped command must not disturb the pairing"
    );
}

// ---------------------------------------------------------------------------
// The pairing flow's refusals, reaching the command path
// ---------------------------------------------------------------------------

#[test]
fn an_expired_code_pairs_nobody_and_the_command_that_follows_is_refused() {
    // The other half of 8.1's claim. Pairing is what confers authority, so a pairing that did
    // not happen must leave the command path exactly as it was — checked here by following it
    // all the way to a refusal rather than by inspecting the store alone.
    let phone = Device::new(phone_id(), 1);
    let desktop = Device::new(desktop_id(), 2);
    let transport = FakeTransport::new();

    let mut codes = PairingCodeIssuer::new();
    let code = codes
        .issue(GrantDelivery::AtMachine, at(0))
        .expect("issue");
    let entered = PairingCode::parse(code.as_str()).expect("parse");

    // Presented after the grant's lifetime. The lifetime comes from the delivery path's own
    // rule, so this test does not restate it.
    let too_late = at(0) + Duration::seconds(GrantDelivery::AtMachine.lifetime_seconds() + 1);

    let response = accept_pairing_at_issuer(
        &mut codes,
        &desktop.store,
        &desktop.identity,
        &phone.identity,
        &entered,
        too_late,
    )
    .expect("the issuer's half returns a verdict");

    assert!(
        matches!(response, PairingResponse::Refused(_)),
        "an expired code must be refused"
    );

    let outcome = complete_pairing_at_requester(&phone.store, response, too_late, || {
        panic!("nothing was recorded, so nothing may be withdrawn")
    })
    .expect("the requester's half completes");
    assert!(!outcome.is_paired());

    // Neither side, per the spec's both-or-neither requirement.
    assert!(!desktop.authorizes(&phone.identity.device_id));
    assert!(!phone.authorizes(&desktop.identity.device_id));

    // And the command that follows is refused, with no job created.
    let envelope = signed_by(&phone, RemoteCommand::PowerOff, "n-unpaired", too_late);
    let state = target_state(&desktop, &phone.identity.device_id, true);
    let delivered = deliver(&transport, &envelope);
    let handled = handle_arriving_command(&desktop, &delivered[0], &state, too_late);

    assert!(
        matches!(
            handled,
            Handled::Refused(RejectionReason::AuthenticityUnverified)
        ),
        "a device that failed to pair is an unknown sender, got {handled:?}"
    );
    assert!(desktop.store.list_all().expect("list").is_empty());
}

#[test]
fn the_countdown_a_remote_job_gets_is_not_the_one_a_local_job_gets() {
    // Guards the mutation task 8.4 requires be demonstrated. Both lengths are read through the
    // same origin-to-length function, so this fails if the mapping is collapsed — whichever
    // direction it is collapsed in.
    //
    // Deliberately free of the number 300. A test naming the figure would keep passing if the
    // *mapping* were broken while the constant stayed, which is exactly the mistake being
    // guarded against.
    let local = grace_period_seconds(JobOrigin::Local);
    let remote = grace_period_seconds(JobOrigin::Remote);

    assert_eq!(
        local, GRACE_PERIOD_SECONDS,
        "a local job counts down for the local length"
    );
    assert!(
        remote > local,
        "a remote job must count down for longer than a local one: the person who will lose \
         their work did not ask for this shutdown and may not be looking at the screen"
    );
}

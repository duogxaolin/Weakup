//! What a compromised relay can and cannot do, tested against a stubbed HTTP layer.
//!
//! # Why these tests route through `evaluate_command`
//!
//! The spec's claim is about the *outcome at the target*, not about the plumbing. A test
//! asserting that the transport returned some particular value proves nothing about whether
//! the machine shuts down — the transport is not what decides that, and a transport that
//! returned nothing at all would pass such a test while proving no property whatsoever.
//!
//! So the hostile cases below take what the relay returned, hand it to the same rule the
//! real receive path uses, and assert on the refusal. That is the sentence the spec actually
//! writes: a relay that returns a command whose signed content it altered, or one it
//! fabricated entirely, changes no outcome.
//!
//! # Why the HTTP layer is stubbed rather than mocked at the network
//!
//! These tests need no network and must not have one. `RelayHttp` is the seam: a stub
//! implementing it returns exactly the bytes a compromised relay could return, and the
//! parsing and the decision then run for real. Nothing here reaches Firestore, and nothing
//! here would pass if the signature check were removed.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use chrono::{DateTime, TimeZone, Utc};

use crate::application::transport::RemoteTransport;
use crate::domain::test_signing::TestKeyPair;
use crate::domain::{
    evaluate_command, CommandAcceptance, CommandEnvelope, CommandTargetState, DeviceId,
    RejectionReason, RemoteCommand,
};
use crate::platform::firebase_transport::{
    envelope_to_document, FirebaseTransport, RelayHttp, RelayRequest,
    PRESENCE_REPORT_INTERVAL_SECONDS,
};

// ---------------------------------------------------------------------------
// A relay that returns whatever it is told to.
// ---------------------------------------------------------------------------

/// An HTTP layer that answers with canned bodies and records what it was asked.
///
/// This is the hostile relay. It is not a mock of Firestore — it is a stand-in for an
/// operator who has been compromised and will return anything at all.
struct StubRelay {
    responses: Mutex<Vec<String>>,
    requests: Mutex<Vec<RelayRequest>>,
}

impl StubRelay {
    fn returning(bodies: Vec<String>) -> Self {
        Self {
            responses: Mutex::new(bodies),
            requests: Mutex::new(Vec::new()),
        }
    }

    fn recorded(&self) -> Vec<RelayRequest> {
        self.requests.lock().expect("stub lock").clone()
    }
}

impl RelayHttp for StubRelay {
    fn send(&self, request: &RelayRequest) -> crate::core::AppResult<String> {
        self.requests
            .lock()
            .expect("stub lock")
            .push(request.clone());

        let mut responses = self.responses.lock().expect("stub lock");
        if responses.is_empty() {
            return Ok("[]".to_string());
        }
        Ok(responses.remove(0))
    }
}

/// A relay whose stubbed bodies are also reachable from the test that built it.
///
/// Two handles to one stub: the transport owns a boxed `RelayHttp`, and the test keeps a
/// reference for its assertions about what was sent.
struct Wired {
    transport: FirebaseTransport,
    relay: std::sync::Arc<StubRelay>,
}

impl RelayHttp for std::sync::Arc<StubRelay> {
    fn send(&self, request: &RelayRequest) -> crate::core::AppResult<String> {
        StubRelay::send(self, request)
    }
}

fn wire(device: &DeviceId, bodies: Vec<String>) -> Wired {
    let relay = std::sync::Arc::new(StubRelay::returning(bodies));
    let transport = FirebaseTransport::new(device.clone(), Box::new(relay.clone()));
    Wired { transport, relay }
}

// ---------------------------------------------------------------------------
// Fixtures.
// ---------------------------------------------------------------------------

fn desktop() -> DeviceId {
    DeviceId::new("desktop-1").expect("non-empty")
}

fn phone() -> DeviceId {
    DeviceId::new("phone-a").expect("non-empty")
}

fn at(second: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 8, 6, 12, 0, second).unwrap()
}

/// A genuine envelope from the phone, signed with the key the desktop holds.
fn genuine_envelope(keys: &TestKeyPair) -> CommandEnvelope {
    let created_at = at(0);
    let nonce = "nonce-1".to_string();
    let signature = keys.sign_command(&phone(), RemoteCommand::PowerOff.as_str(), created_at, &nonce);

    CommandEnvelope {
        sender: phone(),
        command: RemoteCommand::PowerOff,
        created_at,
        nonce,
        signature,
    }
}

/// A target that is paired with the phone, holds its key, and permits remote power-off.
///
/// Everything is set to the permissive value deliberately: a refusal below must come from
/// the signature failing, not from some other check happening to be switched off.
fn target_holding(key: &TestKeyPair) -> CommandTargetState {
    let mut verifying_keys = HashMap::new();
    verifying_keys.insert(phone(), key.verifying_key());

    CommandTargetState {
        seen_nonces: HashSet::new(),
        verifying_keys,
        target_can_power_off: true,
        target_is_remote_target: true,
        is_paired: true,
        remote_control_enabled: true,
    }
}

/// Wraps a document as a `:runQuery` response row, which is what the relay returns.
fn as_query_response(documents: Vec<serde_json::Value>) -> String {
    let rows: Vec<serde_json::Value> = documents
        .into_iter()
        .map(|document| serde_json::json!({ "document": document }))
        .collect();
    serde_json::Value::Array(rows).to_string()
}

// ---------------------------------------------------------------------------
// The three hostile relays.
// ---------------------------------------------------------------------------

#[test]
fn a_relay_that_fabricates_a_command_is_refused_on_authenticity_grounds() {
    // The relay invents a power-off from the phone outright. It has no signing key — that is
    // the entire point of the design — so it puts plausible bytes in the signature field and
    // hopes.
    //
    // Asserted at the *target*, through the same rule the real receive path uses. The
    // transport delivering it is not the property under test; what matters is that the
    // machine does not shut down.
    let key = TestKeyPair::from_seed(1);
    let created_at = at(0);

    let fabricated = CommandEnvelope {
        sender: phone(),
        command: RemoteCommand::PowerOff,
        created_at,
        nonce: "relay-invented-this".to_string(),
        // Bytes of the right length that the relay chose freely.
        signature: vec![0x11; 64],
    };

    let mut document = envelope_to_document(&desktop(), &fabricated);
    document["name"] = serde_json::json!("projects/p/databases/(default)/documents/commands/c1");

    let wired = wire(&desktop(), vec![as_query_response(vec![document])]);

    let delivered = wired
        .transport
        .receive_envelopes(&desktop())
        .expect("the relay's bytes parse; whether they are genuine is not this layer's call");
    assert_eq!(delivered.len(), 1, "the relay did deliver something");

    let decision = evaluate_command(&delivered[0], &target_holding(&key), at(1));

    assert_eq!(
        decision,
        CommandAcceptance::Rejected(RejectionReason::AuthenticityUnverified),
        "a command the named sender did not produce must be refused on authenticity \
         grounds, and the outcome must not depend on anything the relay did or claimed"
    );
}

#[test]
fn a_relay_that_alters_a_genuine_command_is_refused_on_authenticity_grounds() {
    // Harder than fabrication and the more realistic attack: the relay holds a real,
    // correctly signed envelope and changes one field of the signed content. Here it moves
    // the creation instant forward to revive a command that would otherwise be stale.
    //
    // The signature covers sender, command, created_at, and nonce, so altering any of them
    // invalidates it. This is what "the relay stores bytes it cannot usefully modify" means
    // in practice.
    let key = TestKeyPair::from_seed(1);
    let genuine = genuine_envelope(&key);

    let altered = CommandEnvelope {
        created_at: at(30),
        ..genuine.clone()
    };
    assert_ne!(
        altered.created_at, genuine.created_at,
        "the test must actually alter the signed content or it proves nothing"
    );

    let mut document = envelope_to_document(&desktop(), &altered);
    document["name"] = serde_json::json!("projects/p/databases/(default)/documents/commands/c2");

    let wired = wire(&desktop(), vec![as_query_response(vec![document])]);

    let delivered = wired
        .transport
        .receive_envelopes(&desktop())
        .expect("parses");

    let decision = evaluate_command(&delivered[0], &target_holding(&key), at(31));

    assert_eq!(
        decision,
        CommandAcceptance::Rejected(RejectionReason::AuthenticityUnverified),
        "altering signed content must invalidate the signature; a relay that could edit a \
         command and have it obeyed would be able to forge one"
    );
}

#[test]
fn the_same_command_unaltered_is_accepted_so_the_refusals_above_mean_something() {
    // The control. Without this, both tests above would pass against an implementation that
    // refused everything unconditionally — including every genuine command — and the suite
    // would report a working system that can never be commanded at all.
    let key = TestKeyPair::from_seed(1);
    let genuine = genuine_envelope(&key);

    let mut document = envelope_to_document(&desktop(), &genuine);
    document["name"] = serde_json::json!("projects/p/databases/(default)/documents/commands/c3");

    let wired = wire(&desktop(), vec![as_query_response(vec![document])]);

    let delivered = wired
        .transport
        .receive_envelopes(&desktop())
        .expect("parses");

    assert_eq!(
        delivered[0], genuine,
        "a round trip through the relay's document shape must preserve every signed field \
         exactly, including the signature bytes"
    );

    let decision = evaluate_command(&delivered[0], &target_holding(&key), at(1));

    assert_eq!(decision, CommandAcceptance::Accepted);
}

#[test]
fn a_relay_returning_malformed_json_is_an_error_rather_than_a_panic() {
    // A panic on relay-controlled input is a denial of service against the machine being
    // defended: an attacker who can crash the target has stopped it responding to anything,
    // including the user's own commands.
    let wired = wire(&desktop(), vec!["{not json at all".to_string()]);

    let outcome = wired.transport.receive_envelopes(&desktop());

    assert!(
        outcome.is_err(),
        "malformed JSON must surface as an error the caller can report, not as a success \
         carrying nothing and not as a crash"
    );
}

#[test]
fn a_relay_returning_a_document_with_a_missing_field_is_an_error_not_a_default() {
    // Design D8's central claim, tested rather than asserted in prose. A defaulted
    // `createdAt` would make a stale command look fresh; a defaulted nonce would collide
    // with every other defaulted nonce and break replay defence.
    let key = TestKeyPair::from_seed(1);
    let genuine = genuine_envelope(&key);

    for field in ["sender", "command", "createdAt", "nonce", "signature"] {
        let mut document = envelope_to_document(&desktop(), &genuine);
        document["fields"]
            .as_object_mut()
            .expect("fields is an object")
            .remove(field);

        let wired = wire(&desktop(), vec![as_query_response(vec![document])]);

        let outcome = wired.transport.receive_envelopes(&desktop());

        assert!(
            outcome.is_err(),
            "a document missing {field:?} must be an error; a silent default is \
             indistinguishable from a real value and design D8 forbids it"
        );
    }
}

#[test]
fn a_relay_sending_an_unrecognised_command_name_is_an_error_rather_than_a_guess() {
    // Mapping an unknown name onto a plausible one would let a relay steer a command towards
    // a different action than the one that was signed.
    let key = TestKeyPair::from_seed(1);
    let genuine = genuine_envelope(&key);

    let mut document = envelope_to_document(&desktop(), &genuine);
    document["fields"]["command"]["stringValue"] = serde_json::json!("selfDestruct");

    let wired = wire(&desktop(), vec![as_query_response(vec![document])]);

    assert!(wired.transport.receive_envelopes(&desktop()).is_err());
}

#[test]
fn a_signature_that_is_not_valid_base64_is_an_error_rather_than_a_panic() {
    // The decoder sees relay-chosen bytes, so every rejection path must return rather than
    // unwrap.
    let key = TestKeyPair::from_seed(1);
    let genuine = genuine_envelope(&key);

    let mut document = envelope_to_document(&desktop(), &genuine);
    document["fields"]["signature"]["bytesValue"] = serde_json::json!("!!!not base64!!!");

    let wired = wire(&desktop(), vec![as_query_response(vec![document])]);

    assert!(wired.transport.receive_envelopes(&desktop()).is_err());
}

// ---------------------------------------------------------------------------
// Presence is data, reported on a fixed interval, only for oneself.
// ---------------------------------------------------------------------------

#[test]
fn presence_is_reported_every_sixty_seconds() {
    // The number is checked here as well as at compile time because a test names the file
    // to look in when it changes. The compile-time assertion beside the constant catches an
    // interval past the online threshold; this catches a drift to any other value.
    assert_eq!(
        PRESENCE_REPORT_INTERVAL_SECONDS, 60,
        "the presence rule's 90-second online threshold was chosen to tolerate one missed \
         report on a 60-second heartbeat plus jitter"
    );
    assert_eq!(FirebaseTransport::presence_interval().as_secs(), 60);
}

#[test]
fn a_device_reports_only_its_own_presence() {
    // Enforced locally rather than left to the relay's rules, so it holds even against a
    // relay whose rules failed entirely — the same argument the acceptance rule makes about
    // signatures.
    let wired = wire(&desktop(), vec!["{}".to_string()]);

    let outcome = wired.transport.report_presence(&phone(), at(0));

    assert!(
        outcome.is_err(),
        "a transport that could report presence for a peer could make an absent device look \
         present, and a user would send a command to a machine that is not there"
    );
    assert!(
        wired.relay.recorded().is_empty(),
        "the refusal must happen before anything is sent, not after"
    );
}

#[test]
fn reporting_presence_writes_an_instant_and_not_a_state() {
    // The relay stores when a device last spoke. Whether that counts as online is decided by
    // the presence rule at the asking device, so no online-or-not word may appear in what is
    // written.
    let wired = wire(&desktop(), vec!["{}".to_string()]);

    wired
        .transport
        .report_presence(&desktop(), at(0))
        .expect("own presence");

    let requests = wired.relay.recorded();
    let body = requests[0].body.as_deref().expect("a presence write has a body");

    assert!(body.contains("lastReportedAt"));
    assert!(body.contains("timestampValue"));
    assert!(
        !body.contains("online") && !body.contains("offline"),
        "the transport must report an instant rather than a verdict: {body}"
    );
}

#[test]
fn listing_devices_reports_an_instant_and_absence_is_distinguishable_from_staleness() {
    // A device that registered but never reported has no instant at all. `None` rather than
    // a distant past instant: "never spoke" and "spoke long ago" are different facts, and
    // flattening them would make a brand-new device look stale.
    let response = serde_json::json!({
        "documents": [
            {
                "name": "projects/p/databases/(default)/documents/devices/desktop-1",
                "fields": {
                    "deviceId": { "stringValue": "desktop-1" },
                    "displayName": { "stringValue": "Studio Desktop" },
                    "lastReportedAt": { "timestampValue": "2026-08-06T12:00:00.000Z" }
                }
            },
            {
                "name": "projects/p/databases/(default)/documents/devices/phone-a",
                "fields": {
                    "deviceId": { "stringValue": "phone-a" },
                    "displayName": { "stringValue": "Phone A" }
                }
            }
        ]
    })
    .to_string();

    let wired = wire(&desktop(), vec![response]);

    let devices = wired.transport.list_devices().expect("list");

    assert_eq!(devices.len(), 2);
    assert_eq!(devices[0].device_id, desktop());
    assert_eq!(devices[0].last_reported_at, Some(at(0)));
    assert_eq!(devices[1].device_id, phone());
    assert_eq!(
        devices[1].last_reported_at, None,
        "a device that has never reported must carry no instant at all"
    );
}

#[test]
fn a_device_list_with_a_missing_field_is_an_error_not_a_default() {
    let response = serde_json::json!({
        "documents": [
            { "fields": { "deviceId": { "stringValue": "desktop-1" } } }
        ]
    })
    .to_string();

    let wired = wire(&desktop(), vec![response]);

    assert!(wired.transport.list_devices().is_err());
}

#[test]
fn an_empty_account_lists_no_devices_rather_than_failing() {
    // Firestore omits `documents` entirely for an empty collection rather than sending an
    // empty array. Treating that as a parse failure would make a new account look broken.
    let wired = wire(&desktop(), vec!["{}".to_string()]);

    assert_eq!(wired.transport.list_devices().expect("list").len(), 0);
}

// ---------------------------------------------------------------------------
// Sending, and the listener that removes the need to poll.
// ---------------------------------------------------------------------------

#[test]
fn sending_an_envelope_carries_the_signature_bytes_unchanged() {
    // The relay stores opaque content. A transport that re-encoded, truncated, or normalised
    // the signature would break verification for a genuine command and would be doing
    // something to bytes it has no business interpreting.
    let key = TestKeyPair::from_seed(1);
    let genuine = genuine_envelope(&key);

    let wired = wire(&phone(), vec!["{}".to_string()]);

    wired
        .transport
        .send_envelope(&desktop(), &genuine)
        .expect("send");

    let requests = wired.relay.recorded();
    let body = requests[0].body.as_deref().expect("a send has a body");
    let parsed: serde_json::Value = serde_json::from_str(body).expect("valid JSON");

    let round_tripped = crate::platform::firebase_transport::document_to_envelope(&parsed)
        .expect("what was sent must parse back");

    assert_eq!(
        round_tripped, genuine,
        "every signed field must survive the wire exactly, or a genuine command would fail \
         verification at the target for a reason the user could not act on"
    );
}

#[test]
fn the_listener_delivers_a_command_without_a_poll() {
    // Firestore's listen channel is what removes the polling loop: a poll interval short
    // enough to feel immediate would consume the free tier's daily read quota within hours.
    // The payload it delivers is the same document shape a read returns, so an arriving
    // command travels the identical parsing path.
    let key = TestKeyPair::from_seed(1);
    let genuine = genuine_envelope(&key);

    let mut document = envelope_to_document(&desktop(), &genuine);
    document["name"] = serde_json::json!("projects/p/databases/(default)/documents/commands/c9");

    let payload = serde_json::json!({ "documentChange": { "document": document } }).to_string();

    let wired = wire(&desktop(), vec![]);

    let delivered = wired
        .transport
        .envelopes_from_listen_payload(&payload)
        .expect("the channel payload parses");

    assert_eq!(delivered, vec![genuine]);
}

#[test]
fn a_listener_keepalive_is_not_a_command_and_not_an_error() {
    // The channel emits target acknowledgements and keep-alives alongside document changes.
    // Treating those as failures would make a healthy connection look broken; treating them
    // as commands would be worse.
    let wired = wire(&desktop(), vec![]);

    let delivered = wired
        .transport
        .envelopes_from_listen_payload(r#"{"targetChange":{"targetChangeType":"ADD"}}"#)
        .expect("a keep-alive is not a failure");

    assert!(delivered.is_empty());
}

#[test]
fn the_listener_filters_to_this_device_rather_than_reading_every_command() {
    // Scoped at the query so the relay is not asked for envelopes addressed elsewhere. This
    // is quota and hygiene, not a security boundary — the relay's rules are that, and the
    // signature is what actually decides anything.
    let wired = wire(&desktop(), vec![]);

    let request = wired.transport.listen_request();
    let body = request.body.expect("a listen request has a body");

    assert!(body.contains("desktop-1"));
    assert!(body.contains("target"));
}

#[test]
fn a_listener_payload_that_is_malformed_is_an_error_rather_than_a_panic() {
    let wired = wire(&desktop(), vec![]);

    assert!(wired
        .transport
        .envelopes_from_listen_payload("{broken")
        .is_err());
}

// ---------------------------------------------------------------------------
// The relay's own faults, which the target survives.
// ---------------------------------------------------------------------------

#[test]
fn a_relay_that_returns_the_same_command_twice_delivers_it_once_per_session() {
    // Not replay defence — the nonce store in the acceptance rule is that, and it survives a
    // restart. This only avoids pointless duplicate work within one session, and the target
    // would refuse a genuine duplicate regardless.
    let key = TestKeyPair::from_seed(1);
    let genuine = genuine_envelope(&key);

    let mut document = envelope_to_document(&desktop(), &genuine);
    document["name"] = serde_json::json!("projects/p/databases/(default)/documents/commands/dup");

    let response = as_query_response(vec![document]);
    let wired = wire(&desktop(), vec![response.clone(), response]);

    let first = wired.transport.receive_envelopes(&desktop()).expect("first");
    let second = wired.transport.receive_envelopes(&desktop()).expect("second");

    assert_eq!(first.len(), 1);
    assert_eq!(second.len(), 0, "the same document is not delivered twice");
}

#[test]
fn a_relay_returning_a_row_without_a_document_is_not_a_fault() {
    // Firestore's `:runQuery` reports "no more results" as a row with no document. Treating
    // that as malformed would make an empty inbox look like a broken relay.
    let wired = wire(&desktop(), vec![r#"[{"readTime":"2026-08-06T12:00:00Z"}]"#.to_string()]);

    assert_eq!(
        wired.transport.receive_envelopes(&desktop()).expect("empty").len(),
        0
    );
}

#[test]
fn a_relay_that_reports_a_failure_status_surfaces_it_rather_than_swallowing_it() {
    // A relay that is down must produce a visible failure. Reporting success with no
    // envelopes would be indistinguishable from an empty inbox, and the user would conclude
    // their command was received.
    struct FailingRelay;

    impl RelayHttp for FailingRelay {
        fn send(&self, _request: &RelayRequest) -> crate::core::AppResult<String> {
            Err(crate::core::AppError::storage("relay returned status 503"))
        }
    }

    let transport = FirebaseTransport::new(desktop(), Box::new(FailingRelay));

    assert!(transport.receive_envelopes(&desktop()).is_err());
    assert!(transport.list_devices().is_err());
}

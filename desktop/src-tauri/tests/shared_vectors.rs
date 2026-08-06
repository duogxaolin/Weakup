//! Executes the shared cross-language test vectors in `shared/testvectors/`.
//!
//! These same files are executed by the Dart mobile suite. The point is not extra
//! coverage of the Rust code — the unit tests already cover it — but to catch the two
//! implementations drifting apart. A rule changed here and not there fails one side.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use serde::Deserialize;

use weakup_lib::core::AppResult;
use weakup_lib::data::{verifying_keys_from_pairings, PairingRecord};
use weakup_lib::domain::{
    authorize, derive_device_id, encode_signing_payload, evaluate_command, evaluate_grant,
    evaluate_presence, CommandAcceptance, CommandEnvelope, CommandTargetState, DenialReason,
    DeviceId, GrantDelivery, GrantRejection, GrantValidity, JobType, PairingGrant, PresenceState,
    ReconcileOutcome, RejectionReason, RemoteCommand, RemoteCommandContext, RemoteCommandDecision,
    TriggerResolver, TriggerSpec, VerifyingKey,
};

fn vector_dir() -> PathBuf {
    // CARGO_MANIFEST_DIR is desktop/src-tauri, so the shared directory is two levels up.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("shared")
        .join("testvectors")
}

fn load<T: for<'de> Deserialize<'de>>(file: &str) -> T {
    let path = vector_dir().join(file);
    let raw = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("could not read shared vector file {}: {e}", path.display()));
    serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("could not parse shared vector file {}: {e}", path.display()))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VectorSet<C> {
    cases: Vec<C>,
}

// ------------------------------------------------------------------ resolution

/// Distinguishes an absent key from one explicitly set to `null`.
///
/// A plain `Option<Option<T>>` cannot: serde collapses both into the outer `None`. The
/// distinction matters here because `"expectedTargetInstantUtc": null` means "this
/// trigger has no instant by design" while an absent key means "this case expects a
/// rejection instead". Wrapping unconditionally makes an explicit null arrive as
/// `Some(None)`.
fn always_some<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResolutionCase {
    id: String,
    description: String,
    now: DateTime<Utc>,
    timezone: String,
    trigger: TriggerSpec,
    /// Absent when the case expects a rejection. Present-but-`null` means the trigger has
    /// no instant by design, which is why a rejection needs its own key rather than reusing
    /// this one.
    #[serde(default, deserialize_with = "always_some")]
    expected_target_instant_utc: Option<Option<DateTime<Utc>>>,
    #[serde(default)]
    expected_error_contains: Option<String>,
}

#[test]
fn shared_resolution_vectors_all_match() {
    let set: VectorSet<ResolutionCase> = load("resolution.json");
    assert!(
        !set.cases.is_empty(),
        "resolution vector set must not be empty"
    );

    for case in &set.cases {
        let tz: Tz = case
            .timezone
            .parse()
            .unwrap_or_else(|_| panic!("[{}] unknown timezone {}", case.id, case.timezone));

        let actual = TriggerResolver::resolve(&case.trigger, tz, case.now);

        match (&case.expected_target_instant_utc, &case.expected_error_contains) {
            (Some(expected), None) => {
                let resolved =
                    actual.unwrap_or_else(|e| panic!("[{}] resolve failed: {e}", case.id));
                assert_eq!(
                    &resolved, expected,
                    "[{}] {}",
                    case.id, case.description
                );
            }
            (None, Some(needle)) => {
                let error = actual.err().unwrap_or_else(|| {
                    panic!(
                        "[{}] expected a rejection but it resolved: {}",
                        case.id, case.description
                    )
                });
                let message = error.user_message();
                assert!(
                    message.contains(needle),
                    "[{}] error message {message:?} does not contain {needle:?}: {}",
                    case.id,
                    case.description
                );
            }
            // A case with neither expectation asserts nothing and would pass silently; a
            // case with both is ambiguous about what resolve is supposed to do.
            (None, None) => panic!(
                "[{}] sets neither expectedTargetInstantUtc nor expectedErrorContains",
                case.id
            ),
            (Some(_), Some(_)) => panic!(
                "[{}] sets both expectedTargetInstantUtc and expectedErrorContains",
                case.id
            ),
        }
    }

    println!("{} resolution vectors matched", set.cases.len());
}

// -------------------------------------------------------------- reconciliation

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReconciliationCase {
    id: String,
    description: String,
    job_type: JobType,
    target_instant_utc: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
    expected_outcome: ReconcileOutcome,
}

#[test]
fn shared_reconciliation_vectors_all_match() {
    let set: VectorSet<ReconciliationCase> = load("reconciliation.json");
    assert!(
        !set.cases.is_empty(),
        "reconciliation vector set must not be empty"
    );

    for case in &set.cases {
        let actual =
            TriggerResolver::reconcile(case.job_type, case.target_instant_utc, case.now);

        assert_eq!(
            actual, case.expected_outcome,
            "[{}] {}",
            case.id, case.description
        );
    }

    println!("{} reconciliation vectors matched", set.cases.len());
}

#[test]
fn no_shared_vector_expects_an_immediate_power_off() {
    // A vector outcome must never authorize skipping the grace period. If someone adds
    // an outcome variant meaning "power off now", this test is where it gets caught.
    let set: VectorSet<ReconciliationCase> = load("reconciliation.json");
    for case in &set.cases {
        assert!(
            matches!(
                case.expected_outcome,
                ReconcileOutcome::StillPending
                    | ReconcileOutcome::Completed
                    | ReconcileOutcome::ProceedToGracePeriod
                    | ReconcileOutcome::Overdue
            ),
            "[{}] unexpected outcome variant",
            case.id
        );
    }
}

// ------------------------------------------------------------------ validation

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ValidationCase {
    id: String,
    description: String,
    job_type: JobType,
    trigger: TriggerSpec,
    expected_valid: bool,
    #[serde(default)]
    expected_error_contains: Option<String>,
}

#[test]
fn shared_validation_vectors_all_match() {
    let set: VectorSet<ValidationCase> = load("validation.json");
    assert!(
        !set.cases.is_empty(),
        "validation vector set must not be empty"
    );

    for case in &set.cases {
        let result: AppResult<()> = TriggerResolver::validate(&case.trigger, case.job_type);

        match (&result, case.expected_valid) {
            (Ok(()), true) => {}
            (Ok(()), false) => panic!(
                "[{}] expected rejection but it was accepted: {}",
                case.id, case.description
            ),
            (Err(e), true) => panic!(
                "[{}] expected acceptance but it was rejected with {e}: {}",
                case.id, case.description
            ),
            (Err(e), false) => {
                if let Some(needle) = &case.expected_error_contains {
                    let message = e.user_message();
                    assert!(
                        message.contains(needle),
                        "[{}] error message {message:?} does not contain {needle:?}: {}",
                        case.id,
                        case.description
                    );
                }
            }
        }
    }

    println!("{} validation vectors matched", set.cases.len());
}

// -------------------------------------------------------------------- presence

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PresenceCase {
    id: String,
    description: String,
    /// Wrapped so an absent key is distinguishable from an explicit `null`. Absent means
    /// the case forgot to say when the device last reported, which is a broken case
    /// rather than a device that has never reported.
    #[serde(default, deserialize_with = "always_some")]
    last_seen: Option<Option<DateTime<Utc>>>,
    now: DateTime<Utc>,
    expected_state: PresenceState,
    /// Same wrapping, same reason: `null` asserts "there is no age", while an absent key
    /// asserts nothing at all and would let a case pass without checking the elapsed time.
    #[serde(default, deserialize_with = "always_some")]
    expected_elapsed_seconds: Option<Option<i64>>,
}

#[test]
fn shared_presence_vectors_all_match() {
    let set: VectorSet<PresenceCase> = load("presence.json");
    assert!(!set.cases.is_empty(), "presence vector set must not be empty");

    for case in &set.cases {
        let last_seen = case.last_seen.unwrap_or_else(|| {
            panic!("[{}] does not set lastSeen (use null for never reported)", case.id)
        });
        let expected_elapsed = case.expected_elapsed_seconds.unwrap_or_else(|| {
            panic!("[{}] does not set expectedElapsedSeconds", case.id)
        });

        // A case is contradictory when it claims an age for a device that never reported,
        // or claims no age for one that did. Either would pass against an implementation
        // that got the other half wrong.
        assert_eq!(
            last_seen.is_none(),
            expected_elapsed.is_none(),
            "[{}] a device with no lastSeen has no elapsed time, and one with a lastSeen \
             always has one: {}",
            case.id,
            case.description
        );
        if last_seen.is_none() {
            assert_eq!(
                case.expected_state,
                PresenceState::Offline,
                "[{}] a device that never reported can only be offline",
                case.id
            );
        }

        let actual = evaluate_presence(last_seen, case.now);

        assert_eq!(
            actual.state, case.expected_state,
            "[{}] {}",
            case.id, case.description
        );
        assert_eq!(
            actual.elapsed.map(|elapsed| elapsed.num_seconds()),
            expected_elapsed,
            "[{}] elapsed time disagrees: {}",
            case.id,
            case.description
        );
    }

    println!("{} presence vectors matched", set.cases.len());
}

#[test]
fn presence_vectors_cover_both_boundaries_and_both_clock_skew_cases() {
    // The four cases a refactor is most likely to drop, because each looks redundant
    // beside its neighbour until the comparison operator changes. Asserted by id so a
    // parse failure or a quiet deletion cannot skip them silently.
    let set: VectorSet<PresenceCase> = load("presence.json");
    let ids: Vec<&str> = set.cases.iter().map(|case| case.id.as_str()).collect();

    for required in [
        "online-exactly-at-threshold",
        "stale-exactly-at-offline-threshold",
        "online-last-seen-in-the-future-clamps-to-zero",
        "online-last-seen-far-in-the-future-clamps-to-zero",
    ] {
        assert!(
            ids.contains(&required),
            "the {required} case must not be removed"
        );
    }
}

// -------------------------------------------------------- remote authorization

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteAuthorizationCase {
    id: String,
    description: String,
    command: RemoteCommand,
    target_can_power_off: bool,
    target_is_remote_target: bool,
    is_paired: bool,
    remote_control_enabled: bool,
    expected_decision: String,
    #[serde(default)]
    expected_reason: Option<DenialReason>,
}

impl RemoteAuthorizationCase {
    fn context(&self) -> RemoteCommandContext {
        RemoteCommandContext {
            command: self.command,
            target_can_power_off: self.target_can_power_off,
            target_is_remote_target: self.target_is_remote_target,
            is_paired: self.is_paired,
            remote_control_enabled: self.remote_control_enabled,
        }
    }
}

#[test]
fn shared_remote_authorization_vectors_all_match() {
    let set: VectorSet<RemoteAuthorizationCase> = load("remote_authorization.json");
    assert!(
        !set.cases.is_empty(),
        "remote authorization vector set must not be empty"
    );

    for case in &set.cases {
        let actual = authorize(&case.context());

        match (case.expected_decision.as_str(), &case.expected_reason) {
            ("allowed", None) => assert_eq!(
                actual,
                RemoteCommandDecision::Allowed,
                "[{}] {}",
                case.id,
                case.description
            ),
            // The reason is asserted, not merely the refusal: a denial for the wrong
            // reason is the drift these vectors exist to catch, and "it was denied" would
            // pass against an implementation with the precedence order reversed.
            ("denied", Some(expected)) => assert_eq!(
                actual,
                RemoteCommandDecision::Denied(*expected),
                "[{}] {}",
                case.id,
                case.description
            ),
            ("allowed", Some(_)) => panic!(
                "[{}] is allowed but names a refusal reason; a decision has one or the other",
                case.id
            ),
            ("denied", None) => panic!(
                "[{}] is denied but names no reason; every refusal is explainable",
                case.id
            ),
            (other, _) => panic!(
                "[{}] unknown expectedDecision {other:?}, expected \"allowed\" or \"denied\"",
                case.id
            ),
        }
    }

    println!(
        "{} remote authorization vectors matched",
        set.cases.len()
    );
}

#[test]
fn every_remote_authorization_case_names_all_five_context_fields() {
    // A missing boolean would deserialise as a default in a laxer harness and silently
    // change what the case tests — and half of these fields, defaulted the wrong way,
    // authorize a command that should be refused. Checked against the raw JSON so the
    // failure names the case rather than the file.
    let raw: VectorSet<serde_json::Value> = load("remote_authorization.json");

    for case in &raw.cases {
        let object = case
            .as_object()
            .unwrap_or_else(|| panic!("a remote authorization case is not an object: {case}"));
        let id = object
            .get("id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("<no id>");

        for field in [
            "command",
            "targetCanPowerOff",
            "targetIsRemoteTarget",
            "isPaired",
            "remoteControlEnabled",
        ] {
            assert!(
                object.contains_key(field),
                "[{id}] omits the required context field {field}"
            );
        }
    }
}

#[test]
fn remote_authorization_vectors_pin_the_denial_precedence_order() {
    // Each of these sets up two or more simultaneous reasons and names the one that must
    // surface. Without them the vectors would agree with an implementation that checks in
    // any order, and the order is what keeps a refusal from disclosing the target's state.
    let set: VectorSet<RemoteAuthorizationCase> = load("remote_authorization.json");
    let ids: Vec<&str> = set.cases.iter().map(|case| case.id.as_str()).collect();

    for required in [
        "precedence-not-remotely-allowed-beats-remote-control-disabled",
        "precedence-not-remotely-allowed-beats-not-paired",
        "precedence-remote-control-disabled-beats-not-paired",
        "precedence-not-paired-beats-platform-cannot-perform",
        "precedence-all-four-reasons-hold-at-once",
    ] {
        assert!(
            ids.contains(&required),
            "the {required} case must not be removed"
        );
    }

    // Every reason must also appear on its own, or a reason could be dropped from the
    // implementation entirely and only the precedence cases would notice.
    let reasons: std::collections::HashSet<DenialReason> = set
        .cases
        .iter()
        .filter_map(|case| case.expected_reason)
        .collect();
    for reason in [
        DenialReason::CommandNotRemotelyAllowed,
        DenialReason::RemoteControlDisabled,
        DenialReason::NotPaired,
        DenialReason::PlatformCannotPerform,
    ] {
        assert!(
            reasons.contains(&reason),
            "no vector case expects {reason:?}"
        );
    }
}

// ----------------------------------------------------------- command acceptance

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommandAcceptanceCase {
    id: String,
    description: String,
    sender_device_id: String,
    command: RemoteCommand,
    created_at: DateTime<Utc>,
    nonce: String,
    /// The sending device's signature, as lowercase hex.
    ///
    /// Replaces the `signatureVerified` boolean this file used to carry. The rule now
    /// verifies these bytes against `verifyingKeyHex` rather than taking a caller's word,
    /// so a case that expects an authenticity refusal carries a signature that genuinely
    /// fails to verify — made with a different key, not a placeholder.
    signature: String,
    now: DateTime<Utc>,
    seen_nonces: Vec<String>,
    target_can_power_off: bool,
    target_is_remote_target: bool,
    is_paired: bool,
    remote_control_enabled: bool,
    expected_decision: String,
    #[serde(default)]
    expected_reason: Option<RejectionReason>,
}

fn hex_decode(hex: &str) -> Vec<u8> {
    // `% 2` rather than `is_multiple_of`, which is stable only since 1.87 and this crate's
    // MSRV is 1.77.2.
    assert!(hex.len() % 2 == 0, "hex must have even length");
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).expect("valid hex"))
        .collect()
}

impl CommandAcceptanceCase {
    fn envelope(&self) -> CommandEnvelope {
        CommandEnvelope {
            sender: DeviceId::new(&self.sender_device_id)
                .unwrap_or_else(|e| panic!("[{}] invalid senderDeviceId: {e}", self.id)),
            command: self.command,
            created_at: self.created_at,
            nonce: self.nonce.clone(),
            signature: hex_decode(&self.signature),
        }
    }

    /// The target's state, holding `key` for whichever sender the case names.
    ///
    /// The key is registered under the case's own `senderDeviceId` rather than under a
    /// fixed device, so that holding a key and being *paired* stay separate questions —
    /// which they are. `rejected-not-permitted-unpaired-sender` is exactly that case: a
    /// device whose signature verifies but which the target is not paired with, refused on
    /// permission grounds rather than authenticity ones. Registering keys only for paired
    /// devices would collapse the two and let that case pass for the wrong reason.
    ///
    /// Cases expecting an authenticity refusal therefore carry a signature made with a
    /// different key, and fail by arithmetic rather than by an absent entry.
    fn target_state(&self, key: &VerifyingKey) -> CommandTargetState {
        let sender = DeviceId::new(&self.sender_device_id)
            .unwrap_or_else(|e| panic!("[{}] invalid senderDeviceId: {e}", self.id));

        CommandTargetState {
            seen_nonces: self.seen_nonces.iter().cloned().collect(),
            verifying_keys: HashMap::from([(sender, key.clone())]),
            target_can_power_off: self.target_can_power_off,
            target_is_remote_target: self.target_is_remote_target,
            is_paired: self.is_paired,
            remote_control_enabled: self.remote_control_enabled,
        }
    }
}

/// The verifying key every command-acceptance case's target holds, from the top of the
/// vector file.
///
/// Declared once at file level rather than per case: every case's target holds the same
/// genuine key, and 23 copies would invite two of them drifting apart. A case that expects
/// an authenticity refusal carries a signature made with a *different* key, so it fails by
/// arithmetic rather than by a flag.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommandAcceptanceVectorSet {
    verifying_key_hex: String,
    cases: Vec<CommandAcceptanceCase>,
}

#[test]
fn shared_command_acceptance_vectors_all_match() {
    let set: CommandAcceptanceVectorSet = load("command_acceptance.json");
    assert!(
        !set.cases.is_empty(),
        "command acceptance vector set must not be empty"
    );

    let key = VerifyingKey::from_bytes(&hex_decode(&set.verifying_key_hex))
        .expect("verifyingKeyHex must be a valid Ed25519 public key");

    for case in &set.cases {
        let actual = evaluate_command(&case.envelope(), &case.target_state(&key), case.now);

        match (case.expected_decision.as_str(), &case.expected_reason) {
            ("accepted", None) => assert_eq!(
                actual,
                CommandAcceptance::Accepted,
                "[{}] {}",
                case.id,
                case.description
            ),
            // The reason is asserted, not merely the refusal. "It was rejected" would pass
            // against an implementation with the precedence order reversed, and the order
            // is what keeps a refusal from disclosing the target's state.
            ("rejected", Some(expected)) => assert_eq!(
                actual,
                CommandAcceptance::Rejected(*expected),
                "[{}] {}",
                case.id,
                case.description
            ),
            ("accepted", Some(_)) => panic!(
                "[{}] is accepted but names a refusal reason; a decision has one or the other",
                case.id
            ),
            ("rejected", None) => panic!(
                "[{}] is rejected but names no reason; every refusal is explainable",
                case.id
            ),
            (other, _) => panic!(
                "[{}] unknown expectedDecision {other:?}, expected \"accepted\" or \"rejected\"",
                case.id
            ),
        }
    }

    println!("{} command acceptance vectors matched", set.cases.len());
}

#[test]
fn every_command_acceptance_case_names_every_envelope_and_target_field() {
    // A missing field would deserialise as a default in a laxer harness and silently change
    // what the case tests. An omitted `signature` would be an empty one, which fails
    // verification — so the case would still be rejected, but for a reason it did not mean
    // to test, and an accepted case would flip silently.
    let raw: VectorSet<serde_json::Value> = load("command_acceptance.json");

    for case in &raw.cases {
        let object = case
            .as_object()
            .unwrap_or_else(|| panic!("a command acceptance case is not an object: {case}"));
        let id = object
            .get("id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("<no id>");

        for field in [
            "senderDeviceId",
            "command",
            "createdAt",
            "nonce",
            "signature",
            "now",
            "seenNonces",
            "targetCanPowerOff",
            "targetIsRemoteTarget",
            "isPaired",
            "remoteControlEnabled",
        ] {
            assert!(
                object.contains_key(field),
                "[{id}] omits the required field {field}"
            );
        }

        // The field this change removed. Its presence anywhere would mean a case can once
        // again assert authenticity instead of demonstrating it.
        assert!(
            !object.contains_key("signatureVerified"),
            "[{id}] carries signatureVerified, which no longer exists: authenticity is \
             verified from the signature, never asserted"
        );
    }
}

#[test]
fn command_acceptance_vectors_pin_every_boundary_and_the_precedence_order() {
    // The cases a refactor is most likely to drop, because each looks redundant beside its
    // neighbour until a comparison operator changes or a check is reordered. Asserted by id
    // so a parse failure or a quiet deletion cannot skip them.
    let set: CommandAcceptanceVectorSet = load("command_acceptance.json");
    let ids: Vec<&str> = set.cases.iter().map(|case| case.id.as_str()).collect();

    for required in [
        // Both halves of both time boundaries. Losing either half of a pair leaves the
        // boundary decided by whichever operator an implementation happens to use.
        "fresh-exactly-at-window",
        "stale-one-second-past-window",
        "future-exactly-at-tolerance",
        "future-one-second-past-tolerance",
        // Replay, and the case proving a device is not banned after its first command.
        "replay-of-seen-nonce",
        "distinct-nonce-same-device-accepted",
        // Every adjacent pair in the precedence order, plus the all-at-once case.
        "precedence-authenticity-beats-replay",
        "precedence-authenticity-beats-not-permitted",
        "precedence-replay-beats-future-dated",
        "precedence-replay-beats-stale",
        "precedence-future-dated-beats-not-permitted",
        "precedence-stale-beats-not-permitted",
        "precedence-all-five-reasons-hold-at-once",
    ] {
        assert!(
            ids.contains(&required),
            "the {required} case must not be removed"
        );
    }

    // Every reason must also appear on its own, or a reason could be dropped from the
    // implementation entirely and only the precedence cases would notice.
    let reasons: std::collections::HashSet<RejectionReason> = set
        .cases
        .iter()
        .filter_map(|case| case.expected_reason)
        .collect();
    for reason in [
        RejectionReason::AuthenticityUnverified,
        RejectionReason::ReplayedNonce,
        RejectionReason::FutureDated,
        RejectionReason::Stale,
        RejectionReason::NotPermitted,
    ] {
        assert!(
            reasons.contains(&reason),
            "no vector case expects {reason:?}"
        );
    }

    // One accepted case per remotely-permitted command, so a rule that accepted only
    // power-off could not pass.
    let accepted: std::collections::HashSet<RemoteCommand> = set
        .cases
        .iter()
        .filter(|case| case.expected_decision == "accepted")
        .map(|case| case.command)
        .collect();
    for command in [
        RemoteCommand::PowerOff,
        RemoteCommand::KeepAwake,
        RemoteCommand::CancelJob,
    ] {
        assert!(
            accepted.contains(&command),
            "no vector case accepts {command:?}"
        );
    }
}

// -------------------------------------------------------------- signing payload

/// Unlike every other case type here, this one pins an *encoding* rather than a decision.
///
/// Both implementations can agree that a signature is invalid while disagreeing about what
/// bytes a valid one covers. That disagreement is invisible to a decision-level test and
/// surfaces only when a phone and a desktop are paired, which is the normal case.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SigningPayloadCase {
    id: String,
    description: String,
    sender: String,
    command: String,
    created_at_millis: i64,
    nonce: String,
    expected_bytes_hex: String,
}

fn hex_encode(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut out, byte| {
        let _ = write!(out, "{byte:02x}");
        out
    })
}

#[test]
fn shared_signing_payload_vectors_all_match() {
    let set: VectorSet<SigningPayloadCase> = load("signing_payload.json");
    assert!(
        !set.cases.is_empty(),
        "signing payload vector set must not be empty"
    );

    for case in &set.cases {
        let sender = DeviceId::new(&case.sender)
            .unwrap_or_else(|e| panic!("[{}] invalid sender: {e}", case.id));
        let created_at = DateTime::from_timestamp_millis(case.created_at_millis)
            .unwrap_or_else(|| panic!("[{}] createdAtMillis out of range", case.id));

        let actual = encode_signing_payload(&sender, &case.command, created_at, &case.nonce);

        assert_eq!(
            hex_encode(&actual),
            case.expected_bytes_hex,
            "[{}] {}",
            case.id,
            case.description
        );
    }

    println!("{} signing payload vectors matched", set.cases.len());
}

#[test]
fn signing_payload_vectors_pin_the_unambiguous_boundary_and_the_delimiter_case() {
    // Asserted by id rather than by iterating, so a quiet deletion cannot skip them.
    let set: VectorSet<SigningPayloadCase> = load("signing_payload.json");
    let ids: Vec<&str> = set.cases.iter().map(|case| case.id.as_str()).collect();

    for required in [
        // The pair that proves distinct field values cannot encode identically.
        "boundary-ambiguity-sender-ab-nonce-c",
        "boundary-ambiguity-sender-a-nonce-bc",
        // A nonce full of the delimiters a naive encoder would have joined fields with.
        "nonce-contains-naive-delimiters",
        // The length prefix counts bytes, not characters.
        "sender-multibyte-utf8",
        // The instant is unpadded decimal millis.
        "created-at-epoch-zero",
    ] {
        assert!(
            ids.contains(&required),
            "the {required} case must not be removed"
        );
    }

    // The whole argument for length prefixes, as an assertion. If these two encode
    // identically the encoding is forgeable: an attacker who can choose a device id or a
    // nonce could move bytes across a field boundary and keep the signature valid.
    let find = |id: &str| {
        set.cases
            .iter()
            .find(|case| case.id == id)
            .unwrap_or_else(|| panic!("missing required case {id}"))
    };
    let ab_c = find("boundary-ambiguity-sender-ab-nonce-c");
    let a_bc = find("boundary-ambiguity-sender-a-nonce-bc");
    assert_ne!(
        ab_c.expected_bytes_hex, a_bc.expected_bytes_hex,
        "sender 'ab' + nonce 'c' must not encode to the same bytes as sender 'a' + nonce \
         'bc'; if they match, the encoding is forgeable and the vectors are wrong"
    );
}

// ------------------------------------------------------- cross-language proof

/// One implementation's genuinely-produced signature, for the other to verify.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CrossLanguageEntry {
    verifying_key_hex: String,
    signature_hex: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SharedPayload {
    sender: String,
    command: String,
    created_at_millis: i64,
    nonce: String,
    payload_hex: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CrossLanguageSignatures {
    shared_payload: SharedPayload,
    rust_produced: CrossLanguageEntry,
    dart_produced: CrossLanguageEntry,
}

#[test]
fn a_signature_produced_by_the_dart_implementation_verifies_here() {
    // The property the whole design rests on, proven by execution rather than by both sides
    // passing their own tests. These bytes were emitted by `package:cryptography` in the
    // mobile suite; nothing in this crate produced them.
    //
    // The Dart side runs the mirror of this test against the Rust-produced entry, and the
    // two entries use different seeds, so neither can pass by verifying its own signature.
    let fixtures: CrossLanguageSignatures = load("cross_language_signatures.json");

    // First: both implementations must encode the payload identically. A signature over a
    // different encoding would fail below for a reason that has nothing to do with crypto,
    // so this is asserted separately to keep the two failures distinguishable.
    let sender = DeviceId::new(&fixtures.shared_payload.sender).expect("valid sender");
    let created_at = DateTime::from_timestamp_millis(fixtures.shared_payload.created_at_millis)
        .expect("in range");
    let payload = encode_signing_payload(
        &sender,
        &fixtures.shared_payload.command,
        created_at,
        &fixtures.shared_payload.nonce,
    );
    assert_eq!(
        hex_encode(&payload),
        fixtures.shared_payload.payload_hex,
        "the two implementations do not agree on the signed bytes"
    );

    let key = VerifyingKey::from_bytes(&hex_decode(&fixtures.dart_produced.verifying_key_hex))
        .expect("the Dart verifying key must be valid here");

    assert!(
        weakup_lib::domain::verify(
            &payload,
            &hex_decode(&fixtures.dart_produced.signature_hex),
            &key
        ),
        "a signature made by the Dart implementation must verify in Rust; if this fails, a \
         phone and a desktop cannot command each other"
    );
}

#[test]
fn the_rust_produced_fixture_still_matches_this_implementation() {
    // Guards the other half of the pair: if this crate's signing or encoding changes, the
    // committed fixture the Dart suite verifies goes stale, and the Dart test would fail
    // with no indication of why. Failing here names the cause.
    let fixtures: CrossLanguageSignatures = load("cross_language_signatures.json");

    let sender = DeviceId::new(&fixtures.shared_payload.sender).expect("valid sender");
    let created_at = DateTime::from_timestamp_millis(fixtures.shared_payload.created_at_millis)
        .expect("in range");
    let payload = encode_signing_payload(
        &sender,
        &fixtures.shared_payload.command,
        created_at,
        &fixtures.shared_payload.nonce,
    );

    let key = VerifyingKey::from_bytes(&hex_decode(&fixtures.rust_produced.verifying_key_hex))
        .expect("the Rust verifying key must be valid");

    assert!(
        weakup_lib::domain::verify(
            &payload,
            &hex_decode(&fixtures.rust_produced.signature_hex),
            &key
        ),
        "the committed Rust-produced fixture no longer verifies against this implementation; \
         regenerate it, and expect the Dart suite to have failed too"
    );
}

// --------------------------------------------------------------- pairing grant
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PairingGrantCase {
    id: String,
    description: String,
    delivery: GrantDelivery,
    issued_at: DateTime<Utc>,
    redeemed: bool,
    recognised: bool,
    now: DateTime<Utc>,
    expected_validity: String,
    #[serde(default)]
    expected_reason: Option<GrantRejection>,
}

impl PairingGrantCase {
    fn grant(&self) -> PairingGrant {
        PairingGrant {
            delivery: self.delivery,
            issued_at: self.issued_at,
            redeemed: self.redeemed,
            recognised: self.recognised,
        }
    }
}

#[test]
fn shared_pairing_grant_vectors_all_match() {
    let set: VectorSet<PairingGrantCase> = load("pairing_grant.json");
    assert!(
        !set.cases.is_empty(),
        "pairing grant vector set must not be empty"
    );

    for case in &set.cases {
        let actual = evaluate_grant(&case.grant(), case.now);

        match (case.expected_validity.as_str(), &case.expected_reason) {
            ("valid", None) => assert_eq!(
                actual,
                GrantValidity::Valid,
                "[{}] {}",
                case.id,
                case.description
            ),
            ("invalid", Some(expected)) => assert_eq!(
                actual,
                GrantValidity::Invalid(*expected),
                "[{}] {}",
                case.id,
                case.description
            ),
            ("valid", Some(_)) => panic!(
                "[{}] is valid but names a refusal reason; a decision has one or the other",
                case.id
            ),
            ("invalid", None) => panic!(
                "[{}] is invalid but names no reason; every refusal is explainable",
                case.id
            ),
            (other, _) => panic!(
                "[{}] unknown expectedValidity {other:?}, expected \"valid\" or \"invalid\"",
                case.id
            ),
        }
    }

    println!("{} pairing grant vectors matched", set.cases.len());
}

#[test]
fn pairing_grant_vectors_pin_both_lifetimes_and_the_delivery_split() {
    // Both exact-lifetime cases and both halves of the splits-by-delivery pair. The pair is
    // the only thing proving the two lifetimes are actually distinct: an implementation
    // using one lifetime for both passes every other case in the file.
    let set: VectorSet<PairingGrantCase> = load("pairing_grant.json");
    let ids: Vec<&str> = set.cases.iter().map(|case| case.id.as_str()).collect();

    for required in [
        "at-machine-exactly-at-lifetime",
        "at-machine-one-second-past",
        "out-of-band-exactly-at-lifetime",
        "out-of-band-one-second-past",
        "age-between-the-two-lifetimes-splits-by-delivery-at-machine",
        "age-between-the-two-lifetimes-splits-by-delivery-out-of-band",
        "already-redeemed-before-expiry",
        "already-redeemed-after-expiry",
        "unrecognised-grant",
    ] {
        assert!(
            ids.contains(&required),
            "the {required} case must not be removed"
        );
    }

    let reasons: std::collections::HashSet<GrantRejection> = set
        .cases
        .iter()
        .filter_map(|case| case.expected_reason)
        .collect();
    for reason in [
        GrantRejection::Expired,
        GrantRejection::AlreadyUsed,
        GrantRejection::NoSuchGrant,
    ] {
        assert!(
            reasons.contains(&reason),
            "no vector case expects {reason:?}"
        );
    }
}

// --------------------------------------------------------- device id derivation

/// Unlike the decision vectors, this one pins a *derivation*: a verifying key in, an
/// identifier out.
///
/// Two implementations can each be internally consistent while disagreeing about the answer,
/// and the disagreement surfaces only when a phone and a desktop try to recognise each other
/// — by which point each has recorded the other under an id the other does not answer to.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeviceIdDerivationCase {
    id: String,
    description: String,
    verifying_key_hex: String,
    expected_device_id: String,
}

#[test]
fn shared_device_id_derivation_vectors_all_match() {
    let set: VectorSet<DeviceIdDerivationCase> = load("device_id_derivation.json");
    assert!(
        !set.cases.is_empty(),
        "device id derivation vector set must not be empty"
    );

    for case in &set.cases {
        let key_bytes = hex_decode(&case.verifying_key_hex);
        assert_eq!(
            key_bytes.len(),
            32,
            "[{}] verifyingKeyHex must be 32 bytes",
            case.id
        );

        let actual = derive_device_id(&key_bytes);

        assert_eq!(
            actual.as_str(),
            case.expected_device_id,
            "[{}] {}",
            case.id,
            case.description
        );
    }

    println!("{} device id derivation vectors matched", set.cases.len());
}

#[test]
fn device_id_derivation_vectors_prove_the_whole_key_is_hashed() {
    // The three one-byte-difference cases are the only thing standing between this
    // derivation and one that hashes a prefix, or skips the hash and truncates the key
    // directly. Each differs from the all-zeroes key in exactly one byte — at the start,
    // past the 16-byte truncation point, and at the very end.
    let set: VectorSet<DeviceIdDerivationCase> = load("device_id_derivation.json");
    let ids: Vec<&str> = set.cases.iter().map(|case| case.id.as_str()).collect();

    for required in [
        "all-zeroes-key",
        "all-ff-key",
        "known-good-key",
        "one-byte-difference-first-byte",
        "one-byte-difference-middle-byte",
        "one-byte-difference-last-byte",
    ] {
        assert!(
            ids.contains(&required),
            "the {required} case must not be removed"
        );
    }

    // The argument itself, as an assertion. If any two of these expected ids match, the
    // derivation does not depend on the whole key and the vectors are wrong — a device could
    // then present a different key under a paired device's identifier, which is exactly what
    // deriving the id from the key exists to prevent.
    let find = |id: &str| {
        set.cases
            .iter()
            .find(|case| case.id == id)
            .unwrap_or_else(|| panic!("missing required case {id}"))
    };

    let group = [
        "all-zeroes-key",
        "one-byte-difference-first-byte",
        "one-byte-difference-middle-byte",
        "one-byte-difference-last-byte",
    ];
    let mut seen: std::collections::HashMap<&str, &str> = std::collections::HashMap::new();
    for id in group {
        let case = find(id);
        if let Some(other) = seen.insert(case.expected_device_id.as_str(), id) {
            panic!(
                "{id} and {other} derive the same device id ({}); these keys differ by one \
                 byte, so a derivation mapping them together ignores part of the key",
                case.expected_device_id
            );
        }
    }

    // Every id is the full 32 hex characters of a 16-byte value. A shorter one would mean a
    // leading zero byte was dropped somewhere, which collides two devices that differ.
    for case in &set.cases {
        assert_eq!(
            case.expected_device_id.len(),
            32,
            "[{}] a device id is 16 bytes as 32 hex characters",
            case.id
        );
    }
}

// ------------------------------------------------------------------ pairing store

/// One pairing a target holds, as the vector file states it.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PairingStoreEntry {
    peer_device_id: String,
    verifying_key_ref: String,
    revoked: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PairingStoreCase {
    id: String,
    description: String,
    pairings: Vec<PairingStoreEntry>,
    sender: String,
    /// The name of the expected key, or `null` for absent. Absent is what a revoked pairing
    /// yields — see design D5 and the README.
    expected_key_ref: Option<String>,
}

/// Keys are named at file level so two cases referring to the same key cannot drift apart.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PairingStoreVectorSet {
    keys: HashMap<String, String>,
    cases: Vec<PairingStoreCase>,
}

#[test]
fn shared_pairing_store_vectors_all_match() {
    let set: PairingStoreVectorSet = load("pairing_store.json");
    assert!(
        !set.cases.is_empty(),
        "pairing store vector set must not be empty"
    );

    let key_of = |name: &str| -> VerifyingKey {
        let hex = set
            .keys
            .get(name)
            .unwrap_or_else(|| panic!("no key named {name} in the file-level keys object"));
        VerifyingKey::from_bytes(&hex_decode(hex))
            .unwrap_or_else(|| panic!("key {name} is not a valid Ed25519 public key"))
    };

    for case in &set.cases {
        // Build the store's rows exactly as the case states them, then ask the production
        // function which keys a target would actually check signatures against.
        let pairings: Vec<PairingRecord> = case
            .pairings
            .iter()
            .map(|entry| PairingRecord {
                peer: DeviceId::new(&entry.peer_device_id)
                    .unwrap_or_else(|e| panic!("[{}] invalid peerDeviceId: {e}", case.id)),
                verifying_key: key_of(&entry.verifying_key_ref),
                revoked: entry.revoked,
            })
            .collect();

        let sender = DeviceId::new(&case.sender)
            .unwrap_or_else(|e| panic!("[{}] invalid sender: {e}", case.id));

        let map = verifying_keys_from_pairings(&pairings);
        let actual = map.get(&sender);

        match &case.expected_key_ref {
            Some(name) => {
                let expected = key_of(name);
                let actual = actual.unwrap_or_else(|| {
                    panic!(
                        "[{}] expected key {name} but the sender is absent from the map: {}",
                        case.id, case.description
                    )
                });
                assert_eq!(
                    actual, &expected,
                    "[{}] the wrong key was returned: {}",
                    case.id, case.description
                );
            }
            // Absent, not present-and-rejected-later. A key that appeared here would be
            // verified against before anything refused it, which is the behaviour D5 rules
            // out.
            None => assert!(
                actual.is_none(),
                "[{}] expected no key but one was returned: {}",
                case.id,
                case.description
            ),
        }
    }

    println!("{} pairing store vectors matched", set.cases.len());
}

#[test]
fn pairing_store_vectors_pin_revocation_re_pairing_and_the_per_pairing_split() {
    // Asserted by id rather than by iterating, so a quiet deletion cannot skip them.
    let set: PairingStoreVectorSet = load("pairing_store.json");
    let ids: Vec<&str> = set.cases.iter().map(|case| case.id.as_str()).collect();

    for required in [
        // The ordinary case, and the two ways a key can be absent.
        "paired-peer-yields-its-key",
        "unpaired-peer-is-absent",
        // D5: revoked is absent, not present-and-rejected-later.
        "revoked-peer-is-absent-not-rejected-later",
        // Revocation withdraws authority; it does not blacklist.
        "re-paired-formerly-revoked-peer-yields-its-key-again",
        // The pair proving revocation is per-pairing rather than a global switch.
        "revocation-is-per-pairing-not-global-revoked-one",
        "revocation-is-per-pairing-not-global-active-one",
        // One peer's key must not stand in for another's.
        "a-peer-is-checked-against-its-own-key-not-another-peers",
        // The first-run state.
        "an-empty-store-yields-absent",
    ] {
        assert!(
            ids.contains(&required),
            "the {required} case must not be removed"
        );
    }

    let find = |id: &str| {
        set.cases
            .iter()
            .find(|case| case.id == id)
            .unwrap_or_else(|| panic!("missing required case {id}"))
    };

    // The per-pairing split, as an assertion rather than as prose. The two cases must hold
    // identical pairings and differ only in which peer is asked about — otherwise they do
    // not isolate the property, and an implementation treating revocation as a global switch
    // could pass both.
    let revoked_side = find("revocation-is-per-pairing-not-global-revoked-one");
    let active_side = find("revocation-is-per-pairing-not-global-active-one");
    assert_eq!(
        revoked_side.pairings.len(),
        active_side.pairings.len(),
        "the per-pairing split cases must hold the same pairings"
    );
    assert_ne!(
        revoked_side.sender, active_side.sender,
        "the per-pairing split cases must ask about different peers"
    );
    assert!(
        revoked_side.expected_key_ref.is_none() && active_side.expected_key_ref.is_some(),
        "the per-pairing split must have opposite outcomes: the revoked peer absent, the \
         active one present. Without that, revoking one pairing could disable the whole \
         store and both cases would still pass"
    );

    // At least one case must expect absent for a *revoked* peer specifically, or the D5 rule
    // is untested however many other cases the file grows.
    assert!(
        set.cases.iter().any(|case| {
            case.expected_key_ref.is_none()
                && case
                    .pairings
                    .iter()
                    .any(|entry| entry.peer_device_id == case.sender && entry.revoked)
        }),
        "no case asks about a peer whose pairing is revoked; the rule that revoked means \
         absent would be unverified"
    );
}

// --------------------------------------------------------------- meta coverage

#[test]
fn every_vector_case_has_a_unique_id_and_a_description() {
    // Duplicate ids would let a case silently shadow another when a harness indexes
    // by id, and a case without a description is unmaintainable once it fails.
    fn check_ids(label: &str, ids: Vec<(String, String)>) {
        let mut seen = std::collections::HashSet::new();
        for (id, description) in ids {
            assert!(!id.trim().is_empty(), "{label}: found a case with empty id");
            assert!(
                !description.trim().is_empty(),
                "{label}: case {id} has no description"
            );
            assert!(seen.insert(id.clone()), "{label}: duplicate case id {id}");
        }
    }

    let resolution: VectorSet<ResolutionCase> = load("resolution.json");
    check_ids(
        "resolution",
        resolution
            .cases
            .iter()
            .map(|c| (c.id.clone(), c.description.clone()))
            .collect(),
    );

    let reconciliation: VectorSet<ReconciliationCase> = load("reconciliation.json");
    check_ids(
        "reconciliation",
        reconciliation
            .cases
            .iter()
            .map(|c| (c.id.clone(), c.description.clone()))
            .collect(),
    );

    let validation: VectorSet<ValidationCase> = load("validation.json");
    check_ids(
        "validation",
        validation
            .cases
            .iter()
            .map(|c| (c.id.clone(), c.description.clone()))
            .collect(),
    );

    let presence: VectorSet<PresenceCase> = load("presence.json");
    check_ids(
        "presence",
        presence
            .cases
            .iter()
            .map(|c| (c.id.clone(), c.description.clone()))
            .collect(),
    );

    let authorization: VectorSet<RemoteAuthorizationCase> = load("remote_authorization.json");
    check_ids(
        "remote authorization",
        authorization
            .cases
            .iter()
            .map(|c| (c.id.clone(), c.description.clone()))
            .collect(),
    );

    let acceptance: VectorSet<CommandAcceptanceCase> = load("command_acceptance.json");
    check_ids(
        "command acceptance",
        acceptance
            .cases
            .iter()
            .map(|c| (c.id.clone(), c.description.clone()))
            .collect(),
    );

    let grants: VectorSet<PairingGrantCase> = load("pairing_grant.json");
    check_ids(
        "pairing grant",
        grants
            .cases
            .iter()
            .map(|c| (c.id.clone(), c.description.clone()))
            .collect(),
    );

    let derivation: VectorSet<DeviceIdDerivationCase> = load("device_id_derivation.json");
    check_ids(
        "device id derivation",
        derivation
            .cases
            .iter()
            .map(|c| (c.id.clone(), c.description.clone()))
            .collect(),
    );

    let store: PairingStoreVectorSet = load("pairing_store.json");
    check_ids(
        "pairing store",
        store
            .cases
            .iter()
            .map(|c| (c.id.clone(), c.description.clone()))
            .collect(),
    );
}

#[test]
fn vectors_cover_both_dst_anomalies_and_the_tolerance_boundary() {
    // These are the cases most likely to be quietly dropped in a future refactor, so
    // their presence is asserted rather than trusted.
    let resolution: VectorSet<ResolutionCase> = load("resolution.json");
    let ids: Vec<&str> = resolution.cases.iter().map(|c| c.id.as_str()).collect();
    assert!(
        ids.contains(&"absolute-spring-forward-gap"),
        "the spring-forward gap case must not be removed"
    );
    assert!(
        ids.contains(&"absolute-fall-back-overlap-takes-earlier"),
        "the fall-back overlap case must not be removed"
    );
    // Supplying a date must not open a second DST-resolution path, so both anomalies are
    // pinned in the dated form too.
    assert!(
        ids.contains(&"absolute-dated-spring-forward-gap"),
        "the dated spring-forward gap case must not be removed"
    );
    assert!(
        ids.contains(&"absolute-dated-fall-back-overlap-takes-earlier"),
        "the dated fall-back overlap case must not be removed"
    );
    // The pair that pins the dated and undated semantics apart. Losing either half leaves
    // the difference untested.
    assert!(
        ids.contains(&"absolute-past-rolls-to-tomorrow"),
        "the undated roll-forward case must not be removed"
    );
    assert!(
        ids.contains(&"absolute-dated-does-not-roll-forward"),
        "the dated no-roll-forward case must not be removed"
    );

    let reconciliation: VectorSet<ReconciliationCase> = load("reconciliation.json");
    let ids: Vec<&str> = reconciliation.cases.iter().map(|c| c.id.as_str()).collect();
    assert!(
        ids.contains(&"power-off-overdue-exactly-15-min-proceeds"),
        "the inclusive tolerance boundary case must not be removed"
    );
    assert!(
        ids.contains(&"power-off-overdue-16-min-refused"),
        "the just-past-tolerance case must not be removed"
    );
}

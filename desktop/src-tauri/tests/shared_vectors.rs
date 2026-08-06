//! Executes the shared cross-language test vectors in `shared/testvectors/`.
//!
//! These same files are executed by the Dart mobile suite. The point is not extra
//! coverage of the Rust code — the unit tests already cover it — but to catch the two
//! implementations drifting apart. A rule changed here and not there fails one side.

use std::fs;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use serde::Deserialize;

use weakup_lib::core::AppResult;
use weakup_lib::domain::{
    authorize, evaluate_command, evaluate_grant, evaluate_presence, CommandAcceptance,
    CommandEnvelope, CommandTargetState, DenialReason, DeviceId, GrantDelivery, GrantRejection,
    GrantValidity, JobType, PairingGrant, PresenceState, ReconcileOutcome, RejectionReason,
    RemoteCommand, RemoteCommandContext, RemoteCommandDecision, TriggerResolver, TriggerSpec,
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
    signature_verified: bool,
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

impl CommandAcceptanceCase {
    fn envelope(&self) -> CommandEnvelope {
        CommandEnvelope {
            sender: DeviceId::new(&self.sender_device_id)
                .unwrap_or_else(|e| panic!("[{}] invalid senderDeviceId: {e}", self.id)),
            command: self.command,
            created_at: self.created_at,
            nonce: self.nonce.clone(),
            signature_verified: self.signature_verified,
        }
    }

    fn target_state(&self) -> CommandTargetState {
        CommandTargetState {
            seen_nonces: self.seen_nonces.iter().cloned().collect(),
            target_can_power_off: self.target_can_power_off,
            target_is_remote_target: self.target_is_remote_target,
            is_paired: self.is_paired,
            remote_control_enabled: self.remote_control_enabled,
        }
    }
}

#[test]
fn shared_command_acceptance_vectors_all_match() {
    let set: VectorSet<CommandAcceptanceCase> = load("command_acceptance.json");
    assert!(
        !set.cases.is_empty(),
        "command acceptance vector set must not be empty"
    );

    for case in &set.cases {
        let actual = evaluate_command(&case.envelope(), &case.target_state(), case.now);

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
    // what the case tests. `signatureVerified` defaulted to true would accept a forged
    // command, which is the whole property this file exists to pin.
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
            "signatureVerified",
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
    }
}

#[test]
fn command_acceptance_vectors_pin_every_boundary_and_the_precedence_order() {
    // The cases a refactor is most likely to drop, because each looks redundant beside its
    // neighbour until a comparison operator changes or a check is reordered. Asserted by id
    // so a parse failure or a quiet deletion cannot skip them.
    let set: VectorSet<CommandAcceptanceCase> = load("command_acceptance.json");
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

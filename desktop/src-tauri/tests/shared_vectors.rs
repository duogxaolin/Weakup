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
use weakup_lib::domain::{JobType, ReconcileOutcome, TriggerResolver, TriggerSpec};

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

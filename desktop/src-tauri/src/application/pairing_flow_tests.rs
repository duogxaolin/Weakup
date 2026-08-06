//! The pairing exchange's behaviour, and the both-or-neither guarantee design D7 turns on.
//!
//! # Why this file exists separately
//!
//! The tests below include source-text checks over `pairing_flow.rs`. Put inline as a
//! `mod tests` in that file, each test's own string literals would be part of the source it
//! greps and it would fail against itself — the trap that already put `transport_tests.rs`,
//! `device_identity_tests.rs`, and `firebase_transport_source_tests.rs` in their own files.
//!
//! # The four cases the spec names
//!
//! A code redeemed inside its lifetime pairs both sides; an expired code pairs neither; a code
//! presented twice pairs once; and an exchange that fails partway pairs neither. The last one
//! **asserts on both stores**, which is the whole point: a test that checked only the
//! requester's store would pass just as happily against an implementation where the issuer
//! silently recorded nothing, and that is exactly the bug D7 exists to prevent.

use chrono::{DateTime, Duration, TimeZone, Utc};

use crate::application::pairing_flow::{
    accept_pairing_at_issuer, complete_pairing_at_requester, generate_pairing_code,
    withdraw_pairing, PairingCode, PairingCodeIssuer, PairingIdentity, PairingOutcome,
    PairingResponse, CONFUSABLE_GLYPHS, PAIRING_CODE_ALPHABET, PAIRING_CODE_LENGTH,
};
use crate::core::{AppError, AppResult};
use crate::data::{PairingRecord, PairingStore, SqliteJobRepository};
use crate::domain::{
    DeviceId, GrantDelivery, GrantRejection, VerifyingKey, AT_MACHINE_GRANT_LIFETIME_SECONDS,
};

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// The public halves of the fixed test seeds the rest of the suite uses.
fn key(seed: u8) -> VerifyingKey {
    let hex = match seed {
        1 => "8a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c",
        2 => "8139770ea87d175f56a35466c34c7ecccb8d8a91b4ee37a25df60f5b8fc9b394",
        _ => "ed4928c628d1c2c6eae90338905995612959273a5c63f93636c14614ac8737d1",
    };
    let bytes: Vec<u8> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).expect("valid hex"))
        .collect();
    VerifyingKey::from_bytes(&bytes).expect("a valid public key")
}

fn phone() -> PairingIdentity {
    PairingIdentity {
        device_id: DeviceId::new("phone-a").expect("non-empty"),
        verifying_key: key(1),
    }
}

fn laptop() -> PairingIdentity {
    PairingIdentity {
        device_id: DeviceId::new("laptop-b").expect("non-empty"),
        verifying_key: key(2),
    }
}

fn at(seconds: i64) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 8, 7, 12, 0, 0).unwrap() + Duration::seconds(seconds)
}

fn store() -> SqliteJobRepository {
    SqliteJobRepository::open_in_memory().expect("in-memory database")
}

/// Whether `store` holds an *authorizing* pairing with `peer` — the only sense of "believes it
/// is paired" that matters, since a revoked row confers nothing.
fn believes_paired(store: &dyn PairingStore, peer: &DeviceId) -> bool {
    store
        .verifying_keys_from_store()
        .expect("keys")
        .contains_key(peer)
}

/// A store whose `record_pairing` fails, standing in for a device whose disk is full or whose
/// database is locked at the moment the exchange completes.
///
/// Only the write fails. Reads work, so a test can still ask what it believes — which is the
/// question case 4 has to answer for *both* devices.
struct FailingWriteStore {
    inner: SqliteJobRepository,
}

impl FailingWriteStore {
    fn new() -> Self {
        Self { inner: store() }
    }
}

impl PairingStore for FailingWriteStore {
    fn record_pairing(
        &self,
        _peer: &DeviceId,
        _verifying_key: &VerifyingKey,
        _paired_at: DateTime<Utc>,
    ) -> AppResult<()> {
        Err(AppError::Storage {
            message: "the database is locked".to_string(),
        })
    }

    fn list_pairings(&self) -> AppResult<Vec<PairingRecord>> {
        self.inner.list_pairings()
    }

    fn revoke_pairing(&self, peer: &DeviceId, revoked_at: DateTime<Utc>) -> AppResult<()> {
        self.inner.revoke_pairing(peer, revoked_at)
    }
}

// ---------------------------------------------------------------------------
// The code: design D6
// ---------------------------------------------------------------------------

#[test]
fn a_generated_code_is_six_characters_from_the_alphabet() {
    // Drawn repeatedly rather than once: a single draw could satisfy both properties by luck,
    // and the interesting failure is a rejection-sampling bug that occasionally emits a byte
    // from outside the alphabet.
    for _ in 0..200 {
        let code = generate_pairing_code().expect("entropy");
        assert_eq!(
            code.as_str().chars().count(),
            PAIRING_CODE_LENGTH,
            "a code must be exactly {PAIRING_CODE_LENGTH} characters: {code}"
        );
        for c in code.as_str().chars() {
            assert!(
                PAIRING_CODE_ALPHABET.contains(&(c as u8)),
                "code {code} contains {c:?}, which is not in the alphabet"
            );
        }
    }
}

#[test]
fn no_generated_code_contains_a_confusable_glyph() {
    // Design D6, and the reason it is not cosmetic: a user who mistypes sees NoSuchGrant,
    // which is indistinguishable to them from Expired, so they retry until the grant really
    // has expired. This is asserted over many draws rather than over the alphabet constant
    // alone, so a generator that ignored the alphabet would still be caught.
    for _ in 0..200 {
        let code = generate_pairing_code().expect("entropy");
        for confusable in CONFUSABLE_GLYPHS {
            assert!(
                !code.as_str().contains(*confusable),
                "code {code} contains the confusable glyph {confusable:?}"
            );
        }
    }
}

#[test]
fn the_alphabet_itself_excludes_every_confusable_glyph() {
    // The other half of the check above. The generator could stop using the constant; the
    // constant could grow a bad glyph. Both are asserted, so neither alone can regress
    // silently.
    for confusable in CONFUSABLE_GLYPHS {
        assert!(
            !PAIRING_CODE_ALPHABET.contains(&(*confusable as u8)),
            "the alphabet contains {confusable:?}"
        );
    }
}

#[test]
fn two_generated_codes_differ() {
    // A generator returning a constant would pass every shape assertion above. This is a
    // probabilistic check by nature — 31^6 makes a genuine collision vanishingly unlikely
    // across this many draws, so a repeat means the generator is not drawing.
    let codes: std::collections::HashSet<String> = (0..50)
        .map(|_| generate_pairing_code().expect("entropy").as_str().to_string())
        .collect();

    assert!(
        codes.len() > 45,
        "50 draws produced only {} distinct codes, which suggests the generator is not \
         actually random",
        codes.len()
    );
}

#[test]
fn a_typed_code_is_accepted_regardless_of_case_and_spacing() {
    // What a person actually types: lowercase, or in groups with a space or hyphen. None of
    // that changes which code was meant, so none of it is a refusal.
    let canonical = PairingCode::parse("ABC234").expect("valid");
    assert_eq!(canonical.as_str(), "ABC234");

    for entered in ["abc234", " ABC234 ", "ABC 234", "abc-234", "AbC 2 3 4"] {
        assert_eq!(
            PairingCode::parse(entered).expect("valid").as_str(),
            "ABC234",
            "{entered:?} should normalise to ABC234"
        );
    }
}

#[test]
fn a_code_of_the_wrong_length_or_with_a_foreign_glyph_is_refused_at_parse() {
    // Refused here rather than looked up and reported as unrecognised. A genuinely mistyped
    // character must not consume the single use the grant has.
    for entered in ["ABC23", "ABC2345", ""] {
        assert!(
            PairingCode::parse(entered).is_err(),
            "{entered:?} is the wrong length and should be refused"
        );
    }

    // A confusable glyph is a foreign glyph: not silently mapped to its lookalike, because
    // accepting a code as something other than what was typed is harder to reason about.
    for entered in ["ABC23O", "ABC23I", "ABC23l", "ABC2!4"] {
        assert!(
            PairingCode::parse(entered).is_err(),
            "{entered:?} contains a glyph outside the alphabet and should be refused"
        );
    }
}

// ---------------------------------------------------------------------------
// Case 1: a code presented within its lifetime pairs both sides
// ---------------------------------------------------------------------------

#[test]
fn a_code_presented_within_its_lifetime_pairs_both_sides() {
    // The spec's first scenario. Both stores are asserted, because "the pairing succeeded"
    // means each device recorded the *other* — a single-sided success is the D7 failure.
    let issuer_store = store();
    let requester_store = store();
    let mut issuer = PairingCodeIssuer::new();

    let code = issuer
        .issue(GrantDelivery::AtMachine, at(0))
        .expect("issue a code");

    let response = accept_pairing_at_issuer(
        &mut issuer,
        &issuer_store,
        &laptop(),
        &phone(),
        &code,
        at(30),
    )
    .expect("the issuer accepts");

    let outcome = complete_pairing_at_requester(&requester_store, response, at(30), || {
        panic!("undo must not run when nothing failed")
    })
    .expect("the requester records");

    assert!(outcome.is_paired(), "the exchange should have paired");
    assert_eq!(
        outcome,
        PairingOutcome::Paired {
            peer: laptop().device_id
        }
    );

    // The issuer recorded the requester...
    assert!(
        believes_paired(&issuer_store, &phone().device_id),
        "the issuer must hold an authorizing pairing with the requester"
    );
    // ...and the requester recorded the issuer. Both, or the exchange did not complete.
    assert!(
        believes_paired(&requester_store, &laptop().device_id),
        "the requester must hold an authorizing pairing with the issuer"
    );

    // And each recorded the peer's *key*, not merely its name — a pairing storing an
    // identifier alone would authorize whoever presented that name.
    assert_eq!(
        issuer_store.verifying_keys_from_store().expect("keys")[&phone().device_id],
        key(1)
    );
    assert_eq!(
        requester_store.verifying_keys_from_store().expect("keys")[&laptop().device_id],
        key(2)
    );
}

#[test]
fn a_code_presented_at_exactly_its_lifetime_still_pairs() {
    // The boundary the domain rule pins as inclusive-accept. Asserted through the flow so the
    // flow cannot introduce a stricter comparison of its own.
    let issuer_store = store();
    let requester_store = store();
    let mut issuer = PairingCodeIssuer::new();

    let code = issuer
        .issue(GrantDelivery::AtMachine, at(0))
        .expect("issue a code");

    let boundary = at(AT_MACHINE_GRANT_LIFETIME_SECONDS);
    let response = accept_pairing_at_issuer(
        &mut issuer,
        &issuer_store,
        &laptop(),
        &phone(),
        &code,
        boundary,
    )
    .expect("the issuer accepts");

    let outcome =
        complete_pairing_at_requester(&requester_store, response, boundary, || Ok(()))
            .expect("the requester records");

    assert!(outcome.is_paired());
    assert!(believes_paired(&issuer_store, &phone().device_id));
    assert!(believes_paired(&requester_store, &laptop().device_id));
}

// ---------------------------------------------------------------------------
// Case 2: an expired code pairs neither
// ---------------------------------------------------------------------------

#[test]
fn an_expired_code_pairs_neither_side() {
    // The spec's second scenario. Both stores again: an implementation that refused at the
    // requester but had already written at the issuer would pass a one-sided check.
    let issuer_store = store();
    let requester_store = store();
    let mut issuer = PairingCodeIssuer::new();

    let code = issuer
        .issue(GrantDelivery::AtMachine, at(0))
        .expect("issue a code");

    let expired = at(AT_MACHINE_GRANT_LIFETIME_SECONDS + 1);
    let response = accept_pairing_at_issuer(
        &mut issuer,
        &issuer_store,
        &laptop(),
        &phone(),
        &code,
        expired,
    )
    .expect("the issuer answers");

    assert_eq!(response, PairingResponse::Refused(GrantRejection::Expired));

    let outcome = complete_pairing_at_requester(&requester_store, response, expired, || {
        panic!("there is nothing to undo when the issuer recorded nothing")
    })
    .expect("the requester handles a refusal");

    assert_eq!(outcome, PairingOutcome::Refused(GrantRejection::Expired));
    assert!(!outcome.is_paired());

    assert!(
        !believes_paired(&issuer_store, &phone().device_id),
        "an expired code must leave the issuer with no pairing"
    );
    assert!(
        !believes_paired(&requester_store, &laptop().device_id),
        "an expired code must leave the requester with no pairing"
    );
    // Not merely unauthorized — nothing was written at all.
    assert!(issuer_store.list_pairings().expect("list").is_empty());
    assert!(requester_store.list_pairings().expect("list").is_empty());
}

#[test]
fn a_code_that_was_never_issued_is_refused_as_unrecognised_rather_than_expired() {
    // The distinction design D6 exists to preserve. A mistyped code that reported "expired"
    // would send the user looking for a new code they already have.
    let issuer_store = store();
    let mut issuer = PairingCodeIssuer::new();
    issuer
        .issue(GrantDelivery::AtMachine, at(0))
        .expect("issue a code");

    let invented = PairingCode::parse("ABC234").expect("valid shape");
    let response = accept_pairing_at_issuer(
        &mut issuer,
        &issuer_store,
        &laptop(),
        &phone(),
        &invented,
        at(30),
    )
    .expect("the issuer answers");

    assert_eq!(
        response,
        PairingResponse::Refused(GrantRejection::NoSuchGrant)
    );
    assert!(issuer_store.list_pairings().expect("list").is_empty());
}

#[test]
fn a_code_is_only_accepted_by_the_device_that_issued_it() {
    // One of the three properties the ~33-bit code length depends on. A second device asked
    // about a code it did not issue does not recognise it, because codes live in the issuing
    // device's memory and are never sent anywhere.
    let mut issuer = PairingCodeIssuer::new();
    let code = issuer
        .issue(GrantDelivery::AtMachine, at(0))
        .expect("issue a code");

    let other_store = store();
    let mut other_device = PairingCodeIssuer::new();

    let response = accept_pairing_at_issuer(
        &mut other_device,
        &other_store,
        &phone(),
        &laptop(),
        &code,
        at(30),
    )
    .expect("the other device answers");

    assert_eq!(
        response,
        PairingResponse::Refused(GrantRejection::NoSuchGrant),
        "a device that did not issue a code must not accept it"
    );
    assert!(other_store.list_pairings().expect("list").is_empty());
}

// ---------------------------------------------------------------------------
// Case 3: a code presented twice pairs only once
// ---------------------------------------------------------------------------

#[test]
fn a_code_presented_twice_pairs_only_once() {
    // The spec's second scenario, redemption half. Single use is what makes a code observed in
    // transit worthless after the fact — so the second presentation is refused even though it
    // is well within the lifetime.
    let issuer_store = store();
    let mut issuer = PairingCodeIssuer::new();

    let code = issuer
        .issue(GrantDelivery::AtMachine, at(0))
        .expect("issue a code");

    let first = accept_pairing_at_issuer(
        &mut issuer,
        &issuer_store,
        &laptop(),
        &phone(),
        &code,
        at(10),
    )
    .expect("first presentation");
    assert!(matches!(first, PairingResponse::Accepted { .. }));

    // A different device presenting the same observed code, still inside the lifetime.
    let second_requester = PairingIdentity {
        device_id: DeviceId::new("tablet-c").expect("non-empty"),
        verifying_key: key(3),
    };
    let second = accept_pairing_at_issuer(
        &mut issuer,
        &issuer_store,
        &laptop(),
        &second_requester,
        &code,
        at(20),
    )
    .expect("second presentation");

    assert_eq!(
        second,
        PairingResponse::Refused(GrantRejection::AlreadyUsed),
        "the second presentation must be refused as already used, not accepted and not \
         reported as expired"
    );

    // Exactly one pairing exists: the first requester's. The second device gained nothing.
    let keys = issuer_store.verifying_keys_from_store().expect("keys");
    assert!(keys.contains_key(&phone().device_id));
    assert!(
        !keys.contains_key(&second_requester.device_id),
        "a code presented twice must pair only once"
    );
    assert_eq!(issuer_store.list_pairings().expect("list").len(), 1);
}

#[test]
fn a_spent_code_reports_already_used_rather_than_unrecognised() {
    // The reason a redeemed code is retained rather than dropped the instant it is used. The
    // two refusals tell the person different things: one means check the code, the other means
    // this code is spent and a new one is needed.
    let issuer_store = store();
    let mut issuer = PairingCodeIssuer::new();
    let code = issuer
        .issue(GrantDelivery::AtMachine, at(0))
        .expect("issue a code");

    accept_pairing_at_issuer(&mut issuer, &issuer_store, &laptop(), &phone(), &code, at(10))
        .expect("first presentation");

    let again =
        accept_pairing_at_issuer(&mut issuer, &issuer_store, &laptop(), &phone(), &code, at(11))
            .expect("second presentation");

    assert_eq!(again, PairingResponse::Refused(GrantRejection::AlreadyUsed));
}

// ---------------------------------------------------------------------------
// Case 4: an exchange that fails partway leaves NEITHER side paired
// ---------------------------------------------------------------------------

#[test]
fn an_exchange_that_fails_at_the_requester_leaves_neither_side_paired() {
    // **Design D7, and the load-bearing test of this change.**
    //
    // The issuer has recorded the requester and confirmed it. The requester's own write then
    // fails. The wrong behaviour is to leave the issuer's record standing: the issuer would
    // list a peer that holds no pairing with it, the user would see an apparently healthy
    // pairing, and — in the mirror-image case — commands would be refused with no visible
    // reason.
    //
    // **Both stores are asserted.** A version of this test that checked only the requester's
    // store would pass against an implementation that never withdrew the issuer's record,
    // which is precisely the bug this exists to catch.
    let issuer_store = store();
    let requester_store = FailingWriteStore::new();
    let mut issuer = PairingCodeIssuer::new();

    let code = issuer
        .issue(GrantDelivery::AtMachine, at(0))
        .expect("issue a code");

    let response = accept_pairing_at_issuer(
        &mut issuer,
        &issuer_store,
        &laptop(),
        &phone(),
        &code,
        at(30),
    )
    .expect("the issuer accepts");

    // The issuer has written. This is the half-recorded state, mid-exchange.
    assert!(
        believes_paired(&issuer_store, &phone().device_id),
        "precondition: the issuer recorded the requester before the requester's write failed"
    );

    let result = complete_pairing_at_requester(&requester_store, response, at(30), || {
        // The compensating round trip back to the issuer, which in the real system is a
        // request the issuer serves.
        withdraw_pairing(&issuer_store, &phone().device_id, at(31))
    });

    assert!(
        result.is_err(),
        "a failed exchange must be reported as an error, never as a pairing"
    );

    // NEITHER side believes it is paired.
    assert!(
        !believes_paired(&issuer_store, &phone().device_id),
        "the issuer's record must be withdrawn when the requester could not reciprocate: \
         otherwise the issuer lists a peer that holds no pairing with it"
    );
    assert!(
        !believes_paired(&requester_store, &laptop().device_id),
        "the requester must hold no pairing after its own write failed"
    );
}

#[test]
fn a_failed_exchange_leaves_the_withdrawn_record_visible_but_unauthorizing() {
    // What "withdrawn" means concretely, and why it is a revocation rather than a deletion.
    // Design D4: the store never deletes, so the record that an exchange was attempted
    // survives for anyone investigating, while conferring nothing.
    let issuer_store = store();
    let requester_store = FailingWriteStore::new();
    let mut issuer = PairingCodeIssuer::new();
    let code = issuer
        .issue(GrantDelivery::AtMachine, at(0))
        .expect("issue a code");

    let response = accept_pairing_at_issuer(
        &mut issuer,
        &issuer_store,
        &laptop(),
        &phone(),
        &code,
        at(30),
    )
    .expect("the issuer accepts");

    let _ = complete_pairing_at_requester(&requester_store, response, at(30), || {
        withdraw_pairing(&issuer_store, &phone().device_id, at(31))
    });

    let pairings = issuer_store.list_pairings().expect("list");
    assert_eq!(pairings.len(), 1, "the row is retained for the record");
    assert!(
        pairings[0].revoked,
        "the retained row must confer no authority"
    );
}

#[test]
fn a_failure_of_both_the_write_and_its_withdrawal_is_reported_in_full() {
    // The one state that cannot be repaired from the requesting device. Reported rather than
    // flattened into either error alone, because the user needs to know a stale record may
    // exist at the other device and can be revoked there.
    let requester_store = FailingWriteStore::new();
    let mut issuer = PairingCodeIssuer::new();
    let issuer_store = store();
    let code = issuer
        .issue(GrantDelivery::AtMachine, at(0))
        .expect("issue a code");

    let response = accept_pairing_at_issuer(
        &mut issuer,
        &issuer_store,
        &laptop(),
        &phone(),
        &code,
        at(30),
    )
    .expect("the issuer accepts");

    let error = complete_pairing_at_requester(&requester_store, response, at(30), || {
        Err(AppError::Storage {
            message: "the other device is unreachable".to_string(),
        })
    })
    .expect_err("both halves failed");

    let message = error.user_message();
    assert!(
        message.contains("revoke it there"),
        "the message must tell the user where the stale record is and what to do: {message}"
    );
    assert!(
        message.contains("the other device is unreachable"),
        "the withdrawal failure must be named, not swallowed: {message}"
    );
}

#[test]
fn a_refusal_never_runs_the_withdrawal() {
    // A refusal already leaves both sides with nothing, so compensating would be a revocation
    // of a pairing that does not exist — which the store reports as an error, turning a clean
    // refusal into a failure the user cannot act on.
    let requester_store = store();

    let outcome = complete_pairing_at_requester(
        &requester_store,
        PairingResponse::Refused(GrantRejection::NoSuchGrant),
        at(30),
        || panic!("withdrawal must not run for a refusal"),
    )
    .expect("a refusal is not an error");

    assert_eq!(outcome, PairingOutcome::Refused(GrantRejection::NoSuchGrant));
    assert!(requester_store.list_pairings().expect("list").is_empty());
}

#[test]
fn an_issuer_whose_own_write_fails_never_confirms_to_the_requester() {
    // The mirror-image direction of D7. If the issuer cannot record, it must not return an
    // identity — because the identity *is* the confirmation, and a requester that received one
    // would record a peer holding no pairing with it.
    let issuer_store = FailingWriteStore::new();
    let requester_store = store();
    let mut issuer = PairingCodeIssuer::new();
    let code = issuer
        .issue(GrantDelivery::AtMachine, at(0))
        .expect("issue a code");

    let result = accept_pairing_at_issuer(
        &mut issuer,
        &issuer_store,
        &laptop(),
        &phone(),
        &code,
        at(30),
    );

    assert!(
        result.is_err(),
        "an issuer that could not record must report an error rather than confirming"
    );

    // And so nothing reaches the requester's store either.
    assert!(!believes_paired(&issuer_store, &phone().device_id));
    assert!(!believes_paired(&requester_store, &laptop().device_id));
}

// ---------------------------------------------------------------------------
// Reuse rather than reimplementation
// ---------------------------------------------------------------------------

/// The flow's source, read at compile time.
const PAIRING_FLOW_SOURCE: &str = include_str!("pairing_flow.rs");

#[test]
fn the_flow_delegates_expiry_and_single_use_rather_than_reimplementing_them() {
    // The rules are vector-verified in `domain/pairing_grant.rs`. A second implementation here
    // would be a second source of truth, and the one that drifted would be whichever the
    // vectors do not pin. So the flow must *call* the rule.
    assert!(
        PAIRING_FLOW_SOURCE.contains("evaluate_grant("),
        "the pairing flow must delegate to evaluate_grant rather than deciding validity itself"
    );

    // And it must not compute the lifetime comparison itself. `lifetime_seconds` is read once,
    // to sweep stale codes — a *second* comparison against it would be the reimplementation
    // this guards against.
    let comparisons: Vec<&str> = PAIRING_FLOW_SOURCE
        .lines()
        .filter(|line| line.contains("lifetime_seconds"))
        .filter(|line| {
            let t = line.trim_start();
            !t.starts_with("//") && !t.starts_with("///")
        })
        .collect();

    assert!(
        comparisons.len() <= 1,
        "the flow compares against lifetime_seconds in more than one place, which duplicates \
         the expiry rule evaluate_grant already owns: {comparisons:?}"
    );
}

#[test]
fn the_flow_never_names_the_power_off_path() {
    // The same check every other remote module carries. A remote request becomes a job and the
    // existing scheduler runs it, which is what keeps the countdown mandatory; a pairing module
    // reaching the gate or the executor would be a second power-off path with no countdown in
    // front of it.
    for forbidden in [
        "power_off(",
        "PowerOffGate",
        "PowerOffExecutor",
        "GraceOutcome",
    ] {
        let uses: Vec<&str> = PAIRING_FLOW_SOURCE
            .lines()
            .filter(|line| line.contains(forbidden))
            .collect();

        assert!(
            uses.is_empty(),
            "pairing_flow.rs names {forbidden:?}. Pairing confers authority; it does not \
             execute anything: {uses:?}"
        );
    }
}

#[test]
fn the_sweep_forgets_codes_long_past_their_lifetime_and_keeps_recent_ones() {
    // Housekeeping, but it must not sweep so eagerly that AlreadyUsed becomes NoSuchGrant
    // while the user is still looking at the screen.
    let mut issuer = PairingCodeIssuer::new();
    issuer
        .issue(GrantDelivery::AtMachine, at(0))
        .expect("issue a code");
    assert_eq!(issuer.outstanding_count(), 1);

    // Just past expiry: still retained, so a second presentation can report AlreadyUsed or
    // Expired rather than NoSuchGrant.
    issuer.forget_stale(at(AT_MACHINE_GRANT_LIFETIME_SECONDS + 1));
    assert_eq!(
        issuer.outstanding_count(),
        1,
        "a just-expired code must stay reportable"
    );

    issuer.forget_stale(at(AT_MACHINE_GRANT_LIFETIME_SECONDS * 2 + 1));
    assert_eq!(
        issuer.outstanding_count(),
        0,
        "a long-stale code must be forgotten"
    );
}

#[test]
fn a_refusal_carries_the_domains_own_prose() {
    // The surface shows what the domain already wrote, so the two cannot disagree about what
    // an expired code means.
    let outcome = PairingOutcome::Refused(GrantRejection::Expired);
    assert_eq!(outcome.user_message(), GrantRejection::Expired.user_message());
}

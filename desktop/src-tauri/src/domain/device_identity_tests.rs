//! The structural guarantee for the device identity: the private key cannot be read back.
//!
//! # Why this file exists separately
//!
//! The test below greps `device_identity.rs` for member names that must never appear in it.
//! Put inline as a `mod tests` in that file, the test's own string literals would be part of
//! the source it searches, and it would fail against itself. That is not hypothetical — this
//! codebase has hit it before, which is why `transport_tests.rs`, `remote_command_tests.rs`,
//! and `command_acceptance_tests.rs` are also separate files.
//!
//! # Why source text rather than review
//!
//! Because the failure is additive. Someone adds an innocuous-looking `private_key()` while
//! debugging why pairing fails, or swaps the hand-written `Debug` for a derived one because
//! the manual version looks like boilerplate. Every existing test still passes, and the key
//! is now one `{:?}` away from a log file. Source text is a blunt instrument, but it is the
//! mechanism already used here for exactly this class of rule.

use crate::core::AppError;
use crate::domain::device_identity::{
    derive_device_id, load_or_generate, DeviceIdentity, DeviceIdentityRecord,
    DeviceIdentityRecordStore, DEVICE_ID_BYTES, IDENTITY_SECRET_NAME,
};
use crate::domain::signature::verify;
use crate::platform::secret_store::{InMemorySecretStore, SecretStore};

/// The source of the identity module, read at compile time.
const IDENTITY_SOURCE: &str = include_str!("device_identity.rs");

// ---------------------------------------------------------------------------
// The structural guarantee. This is the test that must never be weakened.
// ---------------------------------------------------------------------------

#[test]
fn the_identity_exposes_no_member_that_returns_private_key_material() {
    // A key that can be read will eventually be read — into a log line, an error message, a
    // crash report, or a debugging aid added under time pressure. Because such a member
    // would be *added* rather than break anything existing, its absence is asserted rather
    // than assumed.
    //
    // `fn ` is part of each needle so that prose mentioning "the private key" in a doc
    // comment does not trip the test: what is forbidden is a *member*, not the words.
    for forbidden in [
        "fn private_key",
        "fn export",
        "fn secret_key",
        "fn signing_key",
        "fn to_signing_key",
        "fn into_signing_key",
        "fn key_bytes",
        "fn secret_bytes",
    ] {
        let uses: Vec<&str> = IDENTITY_SOURCE
            .lines()
            .filter(|line| line.contains(forbidden))
            .collect();

        assert!(
            uses.is_empty(),
            "device_identity.rs contains {forbidden:?}, which suggests a member returning \
             private key material. The identity signs; it never hands the key out: {uses:?}"
        );
    }
}

#[test]
fn the_identity_does_not_derive_debug_or_serialize() {
    // A derived `Debug` on a struct holding key bytes renders the private key into any
    // `{:?}`, panic message, or log line a future contributor writes — which is precisely
    // the accident the hand-written implementation exists to prevent. `Serialize` is the
    // same hazard through a different door: it would put the key in JSON.
    //
    // Checked as source text rather than by calling `{:?}`, because a *derived* Debug that
    // happened to print something harmless today would still be a derive, and the next field
    // added to the struct would be rendered without anyone noticing.
    //
    // The exemption is resolved by looking at what the attribute is *attached to* rather
    // than at the attribute line itself: `DeviceIdentityRecord` holds only public material
    // and may derive freely. Anything else — above all the struct holding the signing key —
    // may not.
    let lines: Vec<&str> = IDENTITY_SOURCE.lines().collect();

    for forbidden in ["derive(Debug", "Debug, ", ", Debug", "Serialize", "Deserialize"] {
        for (index, line) in lines.iter().enumerate() {
            if !line.trim_start().starts_with("#[") || !line.contains(forbidden) {
                continue;
            }

            // The first following line that declares an item is what this attribute applies
            // to. Skipping further attributes and doc comments in between.
            let attached_to = lines[index + 1..]
                .iter()
                .find(|next| {
                    let t = next.trim_start();
                    !t.starts_with("#[") && !t.starts_with("///") && !t.is_empty()
                })
                .copied()
                .unwrap_or("");

            assert!(
                attached_to.contains("DeviceIdentityRecord"),
                "device_identity.rs derives {forbidden:?} on {attached_to:?}. Only \
                 DeviceIdentityRecord, which holds public material, may derive these — the \
                 struct holding the signing key must not, because a derived Debug renders \
                 the key and Serialize would write it to JSON. Its Debug is written by hand \
                 to print <sealed>."
            );
        }
    }
}

#[test]
fn the_hand_written_debug_prints_sealed_rather_than_the_key() {
    // The other half of the guarantee above: not merely that `Debug` is not derived, but
    // that what it prints is safe. Asserted by rendering a real identity and checking no
    // byte of the key appears.
    let identity = DeviceIdentity::from_signing_key_bytes(&[7u8; 32]).expect("valid seed");

    let rendered = format!("{identity:?}");

    assert!(
        rendered.contains("<sealed>"),
        "the Debug output must say the key is sealed: {rendered}"
    );
    assert!(
        rendered.contains(identity.device_id().as_str()),
        "the Debug output should still identify the device: {rendered}"
    );
    // The seed was all 7s; its hex would appear as a run of "07". A derived Debug would
    // print the key bytes as a decimal array, so both forms are checked.
    assert!(
        !rendered.contains("07070707"),
        "the Debug output must not contain key material: {rendered}"
    );
    assert!(
        !rendered.contains("7, 7, 7"),
        "the Debug output must not contain key material: {rendered}"
    );
}

// ---------------------------------------------------------------------------
// Derivation.
// ---------------------------------------------------------------------------

#[test]
fn the_device_id_is_sixteen_bytes_of_hex() {
    let id = derive_device_id(&[0u8; 32]);

    assert_eq!(id.as_str().len(), DEVICE_ID_BYTES * 2);
    assert!(id.as_str().chars().all(|c| c.is_ascii_hexdigit()));
    assert!(
        id.as_str().chars().all(|c| !c.is_ascii_uppercase()),
        "the id is lowercase hex; the two implementations must agree on case"
    );
}

#[test]
fn the_same_key_always_derives_the_same_id() {
    assert_eq!(derive_device_id(&[3u8; 32]), derive_device_id(&[3u8; 32]));
}

#[test]
fn different_keys_derive_different_ids() {
    assert_ne!(derive_device_id(&[3u8; 32]), derive_device_id(&[4u8; 32]));
}

#[test]
fn a_one_byte_change_anywhere_in_the_key_changes_the_id() {
    // The property the shared vectors also pin, asserted here across every byte position
    // rather than the three the vectors name. An implementation hashing a prefix passes the
    // vectors' first case and fails here.
    let base = derive_device_id(&[0u8; 32]);

    for position in 0..32 {
        let mut key = [0u8; 32];
        key[position] = 1;

        assert_ne!(
            derive_device_id(&key),
            base,
            "changing byte {position} must change the derived id; if it does not, the \
             derivation ignores part of the key and two devices could claim one identifier"
        );
    }
}

// ---------------------------------------------------------------------------
// Load or generate.
// ---------------------------------------------------------------------------

/// An in-memory stand-in for the database's identity record.
struct FakeRecordStore {
    record: std::sync::Mutex<Option<DeviceIdentityRecord>>,
}

impl FakeRecordStore {
    fn empty() -> Self {
        Self {
            record: std::sync::Mutex::new(None),
        }
    }
}

impl DeviceIdentityRecordStore for FakeRecordStore {
    fn load_identity_record(&self) -> crate::core::AppResult<Option<DeviceIdentityRecord>> {
        Ok(self.record.lock().expect("not poisoned").clone())
    }

    fn save_identity_record(
        &self,
        record: &DeviceIdentityRecord,
    ) -> crate::core::AppResult<()> {
        *self.record.lock().expect("not poisoned") = Some(record.clone());
        Ok(())
    }
}

#[test]
fn the_first_run_generates_an_identity_and_records_both_halves() {
    let records = FakeRecordStore::empty();
    let secrets = InMemorySecretStore::new();

    let identity = load_or_generate(&records, &secrets).expect("first run");

    // The public half is recorded in ordinary storage, so the device can say who it is
    // without unlocking anything.
    let record = records
        .load_identity_record()
        .expect("load")
        .expect("a record was written");
    assert_eq!(&record.device_id, identity.device_id());
    assert_eq!(&record.verifying_key, identity.verifying_key());

    // The private half went to the secret store.
    assert!(secrets.get(IDENTITY_SECRET_NAME).expect("get").is_found());
}

#[test]
fn a_later_run_reuses_the_same_identity() {
    // The property that makes a pairing survive a restart: the id a peer recorded must still
    // be this device's id tomorrow.
    let records = FakeRecordStore::empty();
    let secrets = InMemorySecretStore::new();

    let first = load_or_generate(&records, &secrets).expect("first run");
    let second = load_or_generate(&records, &secrets).expect("second run");

    assert_eq!(first.device_id(), second.device_id());
    assert_eq!(first.verifying_key(), second.verifying_key());
}

#[test]
fn two_devices_generate_different_identities() {
    // Independent installations must not collide, or one device's pairings would authorize
    // another's commands.
    let a = load_or_generate(&FakeRecordStore::empty(), &InMemorySecretStore::new())
        .expect("device a");
    let b = load_or_generate(&FakeRecordStore::empty(), &InMemorySecretStore::new())
        .expect("device b");

    assert_ne!(a.device_id(), b.device_id());
}

#[test]
fn a_record_without_a_key_is_an_error_and_generates_nothing() {
    // The scenario the spec names explicitly. The record says an identity exists but the
    // secret store cannot produce it. Generating a replacement would give this device a new
    // id, making it a stranger to every peer it is paired with — so this must fail instead.
    let records = FakeRecordStore::empty();
    let secrets = InMemorySecretStore::new();

    let original = load_or_generate(&records, &secrets).expect("first run");
    let original_id = original.device_id().clone();

    // Something removed the key: a keychain reset, a migration, a user clearing credentials.
    secrets.delete(IDENTITY_SECRET_NAME).expect("delete");

    let result = load_or_generate(&records, &secrets);

    assert!(
        result.is_err(),
        "a missing key under an existing record must be an error, never a silent regeneration"
    );
    assert!(matches!(result, Err(AppError::Storage { .. })));

    // Nothing was generated: the record still names the original device, and no new key was
    // written. This is the assertion that would fail if the error were reported *after*
    // regenerating, which would be just as destructive.
    assert_eq!(
        records
            .load_identity_record()
            .expect("load")
            .expect("record")
            .device_id,
        original_id,
        "the recorded identity must be untouched"
    );
    assert!(
        !secrets.get(IDENTITY_SECRET_NAME).expect("get").is_found(),
        "no replacement key may be written"
    );
}

#[test]
fn an_unreachable_store_is_an_error_and_generates_nothing() {
    // The recoverable case, which must not be converted into an unrecoverable one. A locked
    // keychain comes back; a regenerated identity does not.
    let records = FakeRecordStore::empty();
    let secrets = InMemorySecretStore::new();

    let original = load_or_generate(&records, &secrets).expect("first run");
    let original_id = original.device_id().clone();

    secrets.fail_with("the keychain is locked");

    let result = load_or_generate(&records, &secrets);
    assert!(result.is_err(), "an unreachable store must be an error");

    // And when the store comes back, the original identity is still there — which is exactly
    // what regenerating would have destroyed.
    secrets.recover();
    let recovered = load_or_generate(&records, &secrets).expect("after recovery");
    assert_eq!(
        recovered.device_id(),
        &original_id,
        "the identity survived the outage; regenerating would have thrown it away"
    );
}

#[test]
fn a_key_that_does_not_match_the_record_is_an_error() {
    // The two halves must describe the same device. If they disagree, one was replaced
    // independently, and proceeding would sign commands under an identifier that peers do
    // not associate with this key — every one of which would be refused, with nothing
    // explaining why.
    let records = FakeRecordStore::empty();
    let secrets = InMemorySecretStore::new();
    load_or_generate(&records, &secrets).expect("first run");

    // A different key under the same record.
    secrets.set(IDENTITY_SECRET_NAME, &[9u8; 32]).expect("set");

    let result = load_or_generate(&records, &secrets);

    assert!(matches!(result, Err(AppError::Storage { .. })));
}

#[test]
fn a_stored_key_of_the_wrong_length_is_an_error_rather_than_a_panic() {
    // The stored bytes come from outside this module and may be corrupt. A panic here would
    // take the app down at startup.
    let records = FakeRecordStore::empty();
    let secrets = InMemorySecretStore::new();
    load_or_generate(&records, &secrets).expect("first run");

    secrets.set(IDENTITY_SECRET_NAME, &[1, 2, 3]).expect("set");

    assert!(load_or_generate(&records, &secrets).is_err());
}

// ---------------------------------------------------------------------------
// Signing.
// ---------------------------------------------------------------------------

#[test]
fn a_signature_from_sign_verifies_against_the_published_verifying_key() {
    // The spec scenario: what the identity signs, a peer holding its published key can
    // check. This is the whole point of the component.
    let identity = load_or_generate(&FakeRecordStore::empty(), &InMemorySecretStore::new())
        .expect("identity");

    let payload = b"a canonical signing payload";
    let signature = identity.sign(payload);

    assert!(
        verify(payload, &signature, identity.verifying_key()),
        "a signature this device produced must verify against the key it publishes"
    );
}

#[test]
fn a_signature_does_not_verify_over_different_content() {
    let identity = load_or_generate(&FakeRecordStore::empty(), &InMemorySecretStore::new())
        .expect("identity");

    let signature = identity.sign(b"the original payload");

    assert!(!verify(b"a different payload", &signature, identity.verifying_key()));
}

#[test]
fn one_devices_signature_does_not_verify_against_anothers_key() {
    let a = load_or_generate(&FakeRecordStore::empty(), &InMemorySecretStore::new())
        .expect("device a");
    let b = load_or_generate(&FakeRecordStore::empty(), &InMemorySecretStore::new())
        .expect("device b");

    let signature = a.sign(b"payload");

    assert!(!verify(b"payload", &signature, b.verifying_key()));
}

#[test]
fn the_device_id_matches_the_derivation_of_its_own_verifying_key() {
    // The self-certifying property from D3, as an internal consistency check: the identifier
    // this device reports is the one its key derives to. If these could differ, the id would
    // be an assignable label again.
    let identity = load_or_generate(&FakeRecordStore::empty(), &InMemorySecretStore::new())
        .expect("identity");

    assert_eq!(
        identity.device_id(),
        &derive_device_id(&identity.verifying_key().to_bytes())
    );
}

#[test]
fn the_public_identity_is_readable_without_consulting_the_secret_store() {
    // The spec scenario: a device reports who it is from ordinary storage, without needing
    // the private key. Proven by reading the record while the secret store is unreachable.
    let records = FakeRecordStore::empty();
    let secrets = InMemorySecretStore::new();
    let identity = load_or_generate(&records, &secrets).expect("first run");
    let expected_id = identity.device_id().clone();

    secrets.fail_with("the keychain is locked");

    let record = records
        .load_identity_record()
        .expect("the record is in ordinary storage")
        .expect("a record exists");

    assert_eq!(record.device_id, expected_id);
}

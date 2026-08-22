//! Signing, for tests only.
//!
//! # Why this exists and why it is `#[cfg(test)]`
//!
//! The production crate verifies signatures and cannot produce one: `signature.rs` exposes
//! no signing key type, and the `ed25519-dalek` dependency is declared without the feature
//! that enables keypair generation (design D6). That is the guarantee, and it stays intact —
//! this module is compiled only under `cfg(test)` and no production path can reach it.
//!
//! Tests need it because the alternative is worse. Before this change, tests asserted
//! authenticity by setting a boolean to `true`; the whole point of the change is that no
//! caller can do that any more. A test helper that faked verification would reintroduce
//! exactly the hole being closed, one layer down, and every test would pass against an
//! implementation that verified nothing. So tests sign for real, with a real key, over the
//! real canonical encoding, and the rule checks the arithmetic exactly as it would in
//! production.
//!
//! # No key generation happens here either
//!
//! The keys below come from fixed seeds. Ed25519 signing is deterministic, so a fixed seed
//! yields a fixed key and a fixed signature — no randomness, no generator, and cases that
//! reproduce byte for byte on every machine. `SigningKey::from_bytes` is not behind the
//! `rand_core` feature precisely because it needs no entropy.

use chrono::{DateTime, Utc};
use ed25519_dalek::{Signer, SigningKey};

use crate::domain::device_id::DeviceId;
use crate::domain::signature::VerifyingKey;
use crate::domain::signing_payload::encode_signing_payload;

/// A keypair a test can sign with, built from a fixed seed.
pub struct TestKeyPair(SigningKey);

impl TestKeyPair {
    /// The keypair for a given seed byte.
    ///
    /// Distinct seeds give distinct keys, which is what the wrong-key tests need: key A
    /// signs, the target holds key B, and the signature must fail.
    pub fn from_seed(seed: u8) -> Self {
        Self(SigningKey::from_bytes(&[seed; 32]))
    }

    /// The public half, as the domain's verifying key type.
    pub fn verifying_key(&self) -> VerifyingKey {
        VerifyingKey::from_bytes(&self.0.verifying_key().to_bytes())
            .expect("a key derived from a signing key is valid")
    }

    /// Signs the canonical encoding of a command's signed content.
    ///
    /// Deliberately goes through [`encode_signing_payload`] rather than signing bytes the
    /// test supplies: a test that signed a different encoding than the rule verifies would
    /// pass or fail for reasons unrelated to what it claims to check.
    pub fn sign_command(
        &self,
        sender: &DeviceId,
        command: &str,
        created_at: DateTime<Utc>,
        nonce: &str,
    ) -> Vec<u8> {
        let payload = encode_signing_payload(sender, command, created_at, nonce);
        self.0.sign(&payload).to_bytes().to_vec()
    }
}

//! Signing, for the integration test suites only.
//!
//! The production crate verifies and cannot sign: `domain/signature.rs` exposes no signing
//! key type, and `ed25519-dalek` is declared without the feature that enables keypair
//! generation (design D6). This file is compiled only as part of `tests/`, so nothing
//! shipped can reach it.
//!
//! Tests sign for real because the alternative is worse. A helper that faked verification
//! would reintroduce the exact hole this change closes — every test would then pass against
//! an implementation that verified nothing.
//!
//! Keys come from fixed seeds. Ed25519 signing is deterministic, so a seed yields a fixed
//! key and a fixed signature: no randomness, no generator, and byte-identical results on
//! every machine. `SigningKey::from_bytes` needs no entropy and is not behind the
//! `rand_core` feature.
//!
//! Mirrors `src/domain/test_signing.rs`, which serves the unit tests. Two copies rather than
//! one because a `#[cfg(test)]` module is not visible to an integration test, and exposing a
//! signing type from the library — even behind a feature — would put a signer in the crate
//! whose guarantee is that it only checks arithmetic.
//!
//! Each integration test compiles this module separately and uses a different subset of it, so
//! the members are marked `allow(dead_code)`: an unused helper here means "this particular test
//! binary did not need it", not "nothing needs it". Without the allow, adding a test that uses
//! only half the API breaks the build for every other one.
#![allow(dead_code)]

use chrono::{DateTime, Utc};
use ed25519_dalek::{Signer, SigningKey};

use weakup_lib::domain::{encode_signing_payload, DeviceId, VerifyingKey};

/// A keypair a test can sign with, built from a fixed seed.
pub struct TestKeyPair(SigningKey);

impl TestKeyPair {
    /// The keypair for a given seed byte. Distinct seeds give distinct keys, which is what
    /// the wrong-key tests need.
    pub fn from_seed(seed: u8) -> Self {
        Self(SigningKey::from_bytes(&[seed; 32]))
    }

    /// The public half, as the domain's verifying key type.
    pub fn verifying_key(&self) -> VerifyingKey {
        VerifyingKey::from_bytes(&self.0.verifying_key().to_bytes())
            .expect("a key derived from a signing key is valid")
    }

    /// The public half as raw bytes, for writing into a vector file or a fixture.
    pub fn verifying_key_bytes(&self) -> [u8; 32] {
        self.0.verifying_key().to_bytes()
    }

    /// Signs the canonical encoding of a command's signed content.
    ///
    /// Goes through [`encode_signing_payload`] rather than signing bytes the caller
    /// supplies: signing a different encoding than the rule verifies would make a test pass
    /// or fail for reasons unrelated to what it claims to check.
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

    /// Signs arbitrary bytes, for the cross-language fixture.
    pub fn sign_bytes(&self, payload: &[u8]) -> Vec<u8> {
        self.0.sign(payload).to_bytes().to_vec()
    }
}

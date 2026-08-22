//! Ed25519 signature verification: the arithmetic that makes authenticity a fact.
//!
//! This module is the reason `CommandEnvelope` no longer carries a `signature_verified`
//! boolean. Authenticity used to be a value a caller supplied; here it is something the
//! target computes from bytes it was given. A relay can still prevent a command from
//! arriving. It cannot cause one to be obeyed.
//!
//! # This module verifies and does not sign
//!
//! There is no signing function, no keypair generation, and no key storage — the
//! `ed25519-dalek` dependency is declared without the feature that would enable any of
//! them (design D6). The rules take a [`VerifyingKey`] they are given; pairing is what
//! delivers one, and the change that ships to a real device is what decides where a private
//! key lives. Stated plainly, because it is the largest remaining gap: a verified signature
//! proves a command came from *whoever holds that key*, not yet that the key is held only
//! by the paired devices.
//!
//! Test keys are constructed from raw bytes via [`VerifyingKey::from_bytes`], which is the
//! same door a pairing record will eventually come through. Nothing here needs a generator.
//!
//! # Malformed input returns false and never panics
//!
//! Both the signature and the key arrive from outside. An attacker chooses those bytes, so
//! every rejection path — wrong length, non-canonical encoding, a point not on the curve,
//! a small-order point, all zeroes — returns `false` rather than unwrapping. A panic on
//! attacker-controlled input is a denial of service against the machine being defended, and
//! `evaluate_command` is pure precisely so that it cannot fail in ways a caller must handle.
//!
//! Note that the two rejections happen at different steps: a malformed *length* is caught
//! when parsing, whereas a structurally valid but degenerate key — the all-zeroes one, for
//! instance — parses fine and is refused by strict verification instead. Both end at
//! `false`, which is the only thing a caller sees.
//!
//! The Dart implementation in `mobile/lib/domain/signature.dart` mirrors this file, and
//! `shared/testvectors/signing_payload.json` pins the bytes both sides sign over.

use ed25519_dalek::{Signature, VerifyingKey as DalekVerifyingKey};

/// The length of an Ed25519 public key in bytes.
pub const VERIFYING_KEY_BYTES: usize = 32;

/// The length of an Ed25519 signature in bytes.
pub const SIGNATURE_BYTES: usize = 64;

/// A public key a target holds for one paired sender.
///
/// A newtype rather than the dalek type used directly, so that the crypto library is named
/// in one file and the rest of the domain depends on this module instead of on a particular
/// crate version. It also keeps the domain's public surface free of a signing type: there
/// is no `SigningKey` here at all, so no rule can accidentally acquire the ability to
/// produce a signature it is supposed to be checking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyingKey(DalekVerifyingKey);

impl VerifyingKey {
    /// Builds a key from its 32 raw bytes, or `None` if they are not a valid public key.
    ///
    /// `None` rather than a panic for the reason in the module comment: these bytes may
    /// have come from a pairing record a relay handed over. A wrong length, a point off the
    /// curve, and all zeroes are all ordinary refusals.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let fixed: [u8; VERIFYING_KEY_BYTES] = bytes.try_into().ok()?;
        DalekVerifyingKey::from_bytes(&fixed).ok().map(Self)
    }

    /// The key's raw 32 bytes, for storing or transmitting it.
    pub fn to_bytes(&self) -> [u8; VERIFYING_KEY_BYTES] {
        self.0.to_bytes()
    }
}

/// Whether `signature` is a genuine signature over `payload` by the holder of `key`.
///
/// `payload` must be the canonical encoding from
/// [`encode_signing_payload`](crate::domain::encode_signing_payload). Verifying over
/// anything else would verify a different message than the one the target is about to act
/// on.
///
/// Uses dalek's *strict* verification, which additionally rejects signatures that are
/// malleable — variants that verify against the same key and message as an existing valid
/// signature. Malleability matters here because the nonce is what prevents a command being
/// obeyed twice: a second, differently-encoded signature over an identical payload would
/// still be caught by the nonce check, but accepting one at all would mean a relay could
/// alter bytes in transit without invalidating the command, which is precisely the power
/// this change exists to deny it.
///
/// Returns `false` for every failure, including malformed input. It never panics.
pub fn verify(payload: &[u8], signature: &[u8], key: &VerifyingKey) -> bool {
    // A wrong-length signature is refused here rather than reaching the curve arithmetic.
    let Ok(parsed) = Signature::from_slice(signature) else {
        return false;
    };

    key.0.verify_strict(payload, &parsed).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A valid Ed25519 public key, as raw bytes.
    ///
    /// Hardcoded rather than generated: this crate does not carry a key generator, and a
    /// test that needed one would be evidence the dependency was declared too broadly. The
    /// value is the public half of the fixed test keypair the end-to-end tests use, and its
    /// validity is asserted below rather than assumed.
    const VALID_KEY_HEX: &str = "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a";

    fn from_hex(hex: &str) -> Vec<u8> {
        (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).expect("valid hex"))
            .collect()
    }

    fn valid_key() -> VerifyingKey {
        VerifyingKey::from_bytes(&from_hex(VALID_KEY_HEX)).expect("a valid public key")
    }

    #[test]
    fn a_valid_key_parses_and_round_trips() {
        let key = valid_key();

        assert_eq!(key.to_bytes().to_vec(), from_hex(VALID_KEY_HEX));
    }

    #[test]
    fn a_key_of_the_wrong_length_is_refused_rather_than_panicking() {
        assert!(VerifyingKey::from_bytes(&[]).is_none());
        assert!(VerifyingKey::from_bytes(&[0u8; 31]).is_none());
        assert!(VerifyingKey::from_bytes(&[0u8; 33]).is_none());
        assert!(VerifyingKey::from_bytes(&[0u8; 64]).is_none());
    }

    #[test]
    fn a_malformed_signature_is_false_and_does_not_panic() {
        // Arbitrary bytes of the right length. Overwhelmingly not a valid signature, and
        // the point is that it is refused by arithmetic rather than by a crash.
        let signature = [0x42u8; SIGNATURE_BYTES];

        assert!(!verify(b"payload", &signature, &valid_key()));
    }

    #[test]
    fn a_wrong_length_signature_is_false_and_does_not_panic() {
        // The length check must come before anything that would index into the buffer.
        for length in [0usize, 1, 32, 63, 65, 128] {
            let signature = vec![0x42u8; length];

            assert!(
                !verify(b"payload", &signature, &valid_key()),
                "a {length}-byte signature must be refused"
            );
        }
    }

    #[test]
    fn an_all_zeroes_signature_is_false_and_does_not_panic() {
        // The degenerate case, and the one a naive implementation is most likely to accept:
        // an all-zero R component is a small-order point, which strict verification refuses.
        let signature = [0u8; SIGNATURE_BYTES];

        assert!(!verify(b"payload", &signature, &valid_key()));
    }

    #[test]
    fn an_all_zeroes_key_verifies_nothing() {
        // The all-zeroes key *parses*: it is a decompressible point, so `from_bytes` accepts
        // it. Rejection happens one step later — it is a small-order point, and strict
        // verification refuses those, which is why `verify_strict` is used rather than
        // `verify`. Asserted here because the security property lives at the verification
        // step rather than at the parsing step, and a reader could reasonably assume the
        // opposite.
        let Some(key) = VerifyingKey::from_bytes(&[0u8; VERIFYING_KEY_BYTES]) else {
            // If a future dalek version refuses it at parse time instead, that is also
            // correct and this test has nothing left to prove.
            return;
        };

        assert!(!verify(b"payload", &[0u8; SIGNATURE_BYTES], &key));
        assert!(!verify(b"payload", &[0x42u8; SIGNATURE_BYTES], &key));
    }

    #[test]
    fn small_order_keys_verify_nothing_in_either_language() {
        // The degenerate points, which let a signature verify against a key nobody holds the
        // private half of. `verify_strict` refuses them here.
        //
        // This matters as a *cross-language* property rather than a local one. The Dart
        // `cryptography` package implements the permissive RFC 8032 check and has no strict
        // variant, so it would accept some of these; `mobile/lib/domain/signature.dart`
        // therefore refuses them explicitly against this same list. Two implementations
        // disagreeing about whether a command is genuine is exactly the drift the shared
        // vectors exist to catch, so the stricter answer is the one both give.
        //
        // Any entry that fails to parse is refused even earlier, which is equally correct.
        for hex in [
            // The identity, and the point of order 2.
            "0000000000000000000000000000000000000000000000000000000000000000",
            "0100000000000000000000000000000000000000000000000000000000000000",
            // The order-4 pair.
            "ecffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff7f",
            "edffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff7f",
            // Non-canonical encodings decoding to the same points.
            "eeffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff7f",
            "daffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
            // Not valid encodings at all; rejected when the key is parsed.
            "e0eb7a7c3b41b8ae1656e3faf19fc46ada098deb9c32b1fd866205165f49b800",
            "5f9c95bca3508c24b1d0b1559c83ef5b04445cc4581c8e86d8224eddd09f1157",
        ] {
            let Some(key) = VerifyingKey::from_bytes(&from_hex(hex)) else {
                continue;
            };

            assert!(
                !verify(b"payload", &[0u8; SIGNATURE_BYTES], &key),
                "{hex} must not verify an all-zeroes signature"
            );
            assert!(
                !verify(b"payload", &[0x42u8; SIGNATURE_BYTES], &key),
                "{hex} must not verify arbitrary bytes"
            );
        }
    }

    #[test]
    fn random_bytes_as_a_signature_never_panic() {
        // The adversarial case stated as its own test rather than assumed from the ones
        // above: an attacker supplies the signature, so `verify` must be total over every
        // possible input. A cheap deterministic generator, so a failure is reproducible.
        let mut state = 0x2545_f491_4f6c_dd1du64;
        let mut next_byte = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state & 0xff) as u8
        };

        for _ in 0..256 {
            let length = (next_byte() % 130) as usize;
            let signature: Vec<u8> = (0..length).map(|_| next_byte()).collect();
            let payload: Vec<u8> = (0..(next_byte() % 64)).map(|_| next_byte()).collect();

            // The assertion is that this returns at all. A panic here fails the test by
            // unwinding, which is the behaviour being ruled out.
            assert!(!verify(&payload, &signature, &valid_key()));
        }
    }

    #[test]
    fn random_bytes_as_a_key_never_panic() {
        // The key is attacker-influenced too — it may arrive in a pairing record — so
        // parsing it must be equally total.
        let mut state = 0x853c_49e6_748f_ea9bu64;
        let mut next_byte = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state & 0xff) as u8
        };

        for _ in 0..256 {
            let length = (next_byte() % 40) as usize;
            let bytes: Vec<u8> = (0..length).map(|_| next_byte()).collect();

            // Most will be refused; any that parse must then verify without panicking.
            if let Some(key) = VerifyingKey::from_bytes(&bytes) {
                assert!(!verify(b"payload", &[0x42u8; SIGNATURE_BYTES], &key));
            }
        }
    }
}

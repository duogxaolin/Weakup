//! The canonical byte encoding of a command's signed content.
//!
//! A signature is only interoperable if both sides agree on what bytes it covers. Two
//! implementations that encode the same command differently produce different signatures
//! over what is logically the same message, and each then rejects the other's genuine
//! commands while both test suites stay green. That failure appears the first time a phone
//! and a desktop are paired, which is the normal case rather than an edge one. So this
//! encoding is pinned by `shared/testvectors/signing_payload.json` and mirrored byte for
//! byte by `mobile/lib/domain/signing_payload.dart`.
//!
//! # The format
//!
//! Four fields, in this fixed order:
//!
//! ```text
//! sender || command || created_at_millis || nonce
//! ```
//!
//! Each is emitted as a 4-byte big-endian unsigned length prefix followed by exactly that
//! many bytes: UTF-8 for the three strings, ASCII decimal digits for the instant. The length
//! counts **bytes, not characters** — an implementation prefixing a character count agrees
//! on every ASCII case and diverges only where it matters.
//!
//! # Why length prefixes and not a delimiter
//!
//! Because a delimiter is forgeable, and this is a real attack rather than a tidiness
//! concern. Suppose the fields were joined with `|`:
//!
//! ```text
//! sender "x|y" + command "z"   encodes to   x|y|z
//! sender "x"   + command "y|z" encodes to   x|y|z
//! ```
//!
//! Two different commands, one byte string, and therefore one signature valid for both. An
//! attacker who can choose a device identifier or a nonce could move bytes across a field
//! boundary and keep the signature intact. Length prefixing makes every boundary explicit,
//! so no two distinct field sets can encode identically — the vectors carry a
//! `boundary-ambiguity-*` pair that fails if this property is ever lost.
//!
//! # Why the instant is millis and not a formatted string
//!
//! A string encoding would put timezone rendering and fractional-second precision inside
//! the signature, and two implementations reliably disagree about both. Milliseconds since
//! the Unix epoch have exactly one spelling per instant.

use chrono::{DateTime, Utc};

use crate::domain::device_id::DeviceId;

/// The width of each field's length prefix, in bytes.
///
/// Four bytes big-endian. Fixed rather than variable-length so that the prefix itself
/// cannot be the thing two implementations disagree about.
const LENGTH_PREFIX_BYTES: usize = 4;

/// Appends one length-prefixed field.
///
/// Private on purpose: a caller that could append a field without its prefix could produce
/// bytes this module's own contract forbids.
fn push_field(out: &mut Vec<u8>, field: &[u8]) {
    // A field longer than u32::MAX cannot occur — a device id, a command name, and a nonce
    // are all bounded far below it — but truncating silently would be a forgery primitive,
    // so the conversion is explicit rather than an `as` cast.
    let length = u32::try_from(field.len())
        .expect("a signed field longer than 4 GiB cannot occur and must not be truncated");
    out.extend_from_slice(&length.to_be_bytes());
    out.extend_from_slice(field);
}

/// Encodes the content a sending device signs and a target verifies.
///
/// `command` is taken as `&str` — its wire name — rather than as a [`RemoteCommand`], so the
/// vector runner can feed the exact string the shared file names and a rename of the Rust
/// enum cannot silently change what gets signed.
///
/// [`RemoteCommand`]: crate::domain::RemoteCommand
pub fn encode_signing_payload(
    sender: &DeviceId,
    command: &str,
    created_at: DateTime<Utc>,
    nonce: &str,
) -> Vec<u8> {
    let millis = created_at.timestamp_millis().to_string();

    let mut out = Vec::with_capacity(
        4 * LENGTH_PREFIX_BYTES
            + sender.as_str().len()
            + command.len()
            + millis.len()
            + nonce.len(),
    );

    push_field(&mut out, sender.as_str().as_bytes());
    push_field(&mut out, command.as_bytes());
    push_field(&mut out, millis.as_bytes());
    push_field(&mut out, nonce.as_bytes());

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(value: &str) -> DeviceId {
        DeviceId::new(value).expect("non-empty")
    }

    fn at(millis: i64) -> DateTime<Utc> {
        DateTime::from_timestamp_millis(millis).expect("in range")
    }

    #[test]
    fn a_field_is_prefixed_by_its_byte_length() {
        let encoded = encode_signing_payload(&device("ab"), "powerOff", at(0), "c");

        // sender: length 2, then "ab".
        assert_eq!(&encoded[0..4], &[0, 0, 0, 2]);
        assert_eq!(&encoded[4..6], b"ab");
    }

    #[test]
    fn the_length_prefix_counts_bytes_not_characters() {
        // Two characters, four bytes. An implementation counting characters would write 2
        // here and disagree with the other language on exactly this input.
        let encoded = encode_signing_payload(&device("Ωλ"), "powerOff", at(0), "n");

        assert_eq!(&encoded[0..4], &[0, 0, 0, 4]);
    }

    #[test]
    fn distinct_field_divisions_encode_differently() {
        // The forgery argument from the module comment, as an assertion. Under a delimiter
        // encoding these two would be the same bytes and one signature would cover both.
        let ab_c = encode_signing_payload(&device("ab"), "powerOff", at(0), "c");
        let a_bc = encode_signing_payload(&device("a"), "powerOff", at(0), "bc");

        assert_ne!(ab_c, a_bc);
    }

    #[test]
    fn a_nonce_full_of_delimiters_is_ordinary_data() {
        // Nothing about `|` or `:` is special here, which is the point of not using them.
        let encoded = encode_signing_payload(&device("phone-a"), "powerOff", at(0), "a|b:c");

        assert_eq!(
            encoded,
            encode_signing_payload(&device("phone-a"), "powerOff", at(0), "a|b:c")
        );
        assert_ne!(
            encoded,
            encode_signing_payload(&device("phone-a"), "powerOff", at(0), "a|b:d")
        );
    }

    #[test]
    fn every_signed_field_changes_the_encoding() {
        // A field outside the encoding is a field an intermediary can change undetected, so
        // each of the four is varied in turn against a fixed baseline.
        let baseline = encode_signing_payload(&device("phone-a"), "powerOff", at(1_000), "n-1");

        assert_ne!(
            baseline,
            encode_signing_payload(&device("phone-b"), "powerOff", at(1_000), "n-1"),
            "sender is not covered"
        );
        assert_ne!(
            baseline,
            encode_signing_payload(&device("phone-a"), "keepAwake", at(1_000), "n-1"),
            "command is not covered"
        );
        assert_ne!(
            baseline,
            encode_signing_payload(&device("phone-a"), "powerOff", at(2_000), "n-1"),
            "created_at is not covered"
        );
        assert_ne!(
            baseline,
            encode_signing_payload(&device("phone-a"), "powerOff", at(1_000), "n-2"),
            "nonce is not covered"
        );
    }

    #[test]
    fn the_epoch_is_a_single_unpadded_digit() {
        let encoded = encode_signing_payload(&device("a"), "x", at(0), "n");

        // sender(4+1) + command(4+1) = 10 bytes, then the millis field.
        assert_eq!(&encoded[10..14], &[0, 0, 0, 1]);
        assert_eq!(&encoded[14..15], b"0");
    }

    #[test]
    fn sub_second_precision_is_preserved() {
        // Truncating to whole seconds would let two commands a few hundred milliseconds
        // apart share an encoding, and therefore a signature.
        assert_ne!(
            encode_signing_payload(&device("a"), "x", at(1_500), "n"),
            encode_signing_payload(&device("a"), "x", at(1_000), "n")
        );
    }
}

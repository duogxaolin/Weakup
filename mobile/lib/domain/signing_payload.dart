/// The canonical byte encoding of a command's signed content.
///
/// A signature is only interoperable if both sides agree on what bytes it covers. Two
/// implementations that encode the same command differently produce different signatures over
/// what is logically the same message, and each then rejects the other's genuine commands while
/// both test suites stay green. That failure appears the first time a phone and a desktop are
/// paired, which is the normal case rather than an edge one. So this encoding is pinned by
/// `shared/testvectors/signing_payload.json` and mirrors
/// `desktop/src-tauri/src/domain/signing_payload.rs` byte for byte.
///
/// # The format
///
/// Four fields, in this fixed order:
///
/// ```text
/// sender || command || createdAtMillis || nonce
/// ```
///
/// Each is emitted as a 4-byte big-endian unsigned length prefix followed by exactly that many
/// bytes: UTF-8 for the three strings, ASCII decimal digits for the instant. The length counts
/// **bytes, not characters** — an implementation prefixing a character count agrees on every
/// ASCII case and diverges only where it matters. Dart makes this trap easy to fall into,
/// because `String.length` is UTF-16 code units rather than bytes.
///
/// # Why length prefixes and not a delimiter
///
/// Because a delimiter is forgeable, and this is a real attack rather than a tidiness concern.
/// Suppose the fields were joined with `|`:
///
/// ```text
/// sender "x|y" + command "z"   encodes to   x|y|z
/// sender "x"   + command "y|z" encodes to   x|y|z
/// ```
///
/// Two different commands, one byte string, and therefore one signature valid for both. An
/// attacker who can choose a device identifier or a nonce could move bytes across a field
/// boundary and keep the signature intact. Length prefixing makes every boundary explicit, so no
/// two distinct field sets can encode identically — the vectors carry a `boundary-ambiguity-*`
/// pair that fails if this property is ever lost.
///
/// # Why the instant is millis and not a formatted string
///
/// A string encoding would put timezone rendering and fractional-second precision inside the
/// signature, and two implementations reliably disagree about both. Milliseconds since the Unix
/// epoch have exactly one spelling per instant.
library;

import 'dart:convert';
import 'dart:typed_data';

import 'device_id.dart';

/// The width of each field's length prefix, in bytes.
///
/// Four bytes big-endian. Fixed rather than variable-length so the prefix itself cannot be the
/// thing two implementations disagree about.
const int lengthPrefixBytes = 4;

/// Encodes the content a sending device signs and a target verifies.
///
/// [command] is the command's wire name rather than the enum, so the vector runner can feed the
/// exact string the shared file names and a rename cannot silently change what gets signed.
Uint8List encodeSigningPayload({
  required DeviceId sender,
  required String command,
  required DateTime createdAt,
  required String nonce,
}) {
  final fields = <List<int>>[
    utf8.encode(sender.value),
    utf8.encode(command),
    // `toUtc()` first: an instant is the same instant in any zone, but a local `DateTime`
    // would still yield the same millis, and being explicit costs nothing and documents that
    // no timezone enters the signature.
    ascii.encode(createdAt.toUtc().millisecondsSinceEpoch.toString()),
    utf8.encode(nonce),
  ];

  final builder = BytesBuilder(copy: false);
  for (final field in fields) {
    builder.add(_lengthPrefix(field.length));
    builder.add(field);
  }

  return builder.toBytes();
}

/// One field's 4-byte big-endian length.
Uint8List _lengthPrefix(int length) {
  final prefix = ByteData(lengthPrefixBytes);
  // Big-endian explicitly. The host's native order is little-endian on every platform this
  // app ships to, so relying on the default would produce bytes the Rust side rejects.
  prefix.setUint32(0, length, Endian.big);
  return prefix.buffer.asUint8List();
}

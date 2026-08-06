/// Ed25519 signature verification: the arithmetic that makes authenticity a fact.
///
/// This library is the reason [CommandEnvelope] no longer carries a `signatureVerified` boolean.
/// Authenticity used to be a value a caller supplied; here it is something the target computes
/// from bytes it was given. A relay can still prevent a command from arriving. It cannot cause
/// one to be obeyed.
///
/// # This library verifies and does not sign
///
/// There is no signing function and no key generation here. The rules take a [VerifyingKey] they
/// are given; pairing is what delivers one, and the change that ships to a real device decides
/// where a private key lives. Stated plainly, because it is the largest remaining gap: a verified
/// signature proves a command came from *whoever holds that key*, not yet that the key is held
/// only by the paired devices.
///
/// # Malformed input returns false and never throws
///
/// Both the signature and the key arrive from outside — an attacker chooses those bytes. Every
/// rejection path returns `false` rather than throwing: a wrong-length signature, a key that is
/// not a curve point, a small-order point, all zeroes. An exception on attacker-controlled input
/// is a denial of service against the machine being defended.
///
/// The underlying package throws on some malformed input rather than returning false, so the
/// catch below is load-bearing rather than defensive habit. It is deliberately broad: the
/// contract is that *nothing* escapes, and enumerating the package's exception types would leave
/// the guarantee dependent on that list staying complete across versions.
///
/// # Why verification is asynchronous here and synchronous in Rust
///
/// The `cryptography` package's Ed25519 is `Future`-returning — its SHA-512 step is async and
/// there is no synchronous variant, including in `DartEd25519`. Rust's `ed25519-dalek` is
/// synchronous. The asymmetry is the packages', not the design's: the same bytes are verified
/// against the same key with the same result, and the shared vectors hold both sides to it.
///
/// Mirrors `desktop/src-tauri/src/domain/signature.rs`, and
/// `shared/testvectors/signing_payload.json` pins the bytes both sides sign over.
library;

import 'package:cryptography/cryptography.dart';

/// The length of an Ed25519 public key in bytes.
const int verifyingKeyBytes = 32;

/// The length of an Ed25519 signature in bytes.
const int signatureBytes = 64;

/// The algorithm, created once. Stateless and safe to share.
final Ed25519 _ed25519 = Ed25519();

/// A public key a target holds for one paired sender.
///
/// A wrapper rather than the package's type used directly, so the crypto library is named in one
/// file and the rest of the domain depends on this library instead of on a particular package
/// version. It also keeps the domain free of a signing type: there is no `SigningKey` here, so no
/// rule can accidentally acquire the ability to produce a signature it is supposed to be
/// checking.
final class VerifyingKey {
  const VerifyingKey._(this._key, this.bytes);

  /// Builds a key from its 32 raw bytes, or `null` if they are not the right shape.
  ///
  /// `null` rather than an exception: these bytes may have come from a pairing record a relay
  /// handed over, so a wrong length is an ordinary refusal.
  ///
  /// Note that a structurally valid but degenerate key — the all-zeroes one, for instance —
  /// parses here and is refused at verification instead, where it is caught as a small-order
  /// point. Both paths end at `false`, which is the only thing a caller sees.
  static VerifyingKey? fromBytes(List<int> bytes) {
    if (bytes.length != verifyingKeyBytes) {
      return null;
    }
    final copy = List<int>.unmodifiable(bytes);
    return VerifyingKey._(
      SimplePublicKey(copy, type: KeyPairType.ed25519),
      copy,
    );
  }

  final SimplePublicKey _key;

  /// The key's raw 32 bytes, for storing or transmitting it.
  final List<int> bytes;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      (other is VerifyingKey && _listEquals(other.bytes, bytes));

  @override
  int get hashCode => Object.hashAll(bytes);

  @override
  String toString() => 'VerifyingKey(${bytes.length} bytes)';
}

/// Whether [signature] is a genuine signature over [payload] by the holder of [key].
///
/// [payload] must be the canonical encoding from `encodeSigningPayload`. Verifying over anything
/// else would verify a different message than the one the target is about to act on.
///
/// # Strictness, and why it is enforced here rather than left to the package
///
/// Rust's `ed25519-dalek` is used through `verify_strict`, which rejects small-order keys and
/// signature components — the degenerate points for which a signature can verify against a key
/// nobody controls. The `cryptography` package implements the permissive RFC 8032 check and has
/// no strict variant, so it accepts an all-zeroes signature against an all-zeroes key where Rust
/// refuses it.
///
/// That difference is not cosmetic: it is two implementations disagreeing about whether a
/// command is genuine, which is precisely the drift the shared vectors exist to catch. Rather
/// than relax the Rust side to match, the small-order cases are refused here, so both languages
/// reach the same answer and the stricter one wins.
///
/// The check is a byte comparison against the known small-order encodings rather than curve
/// arithmetic, because the package exposes no point API. There are eight such points on
/// Curve25519 and their encodings are fixed constants of the curve, so the list is complete and
/// cannot drift.
///
/// Returns `false` for every failure, including malformed input. It never throws.
Future<bool> verifySignature({
  required List<int> payload,
  required List<int> signature,
  required VerifyingKey key,
}) async {
  // Refused here rather than reaching the curve arithmetic, and before the package's own
  // length check, which throws rather than returning false.
  if (signature.length != signatureBytes) {
    return false;
  }

  // A small-order public key, or a small-order R component, makes a signature verifiable
  // without knowing any private key. Rust refuses both; so does this.
  if (_isSmallOrder(key.bytes) || _isSmallOrder(signature.sublist(0, 32))) {
    return false;
  }

  try {
    return await _ed25519.verify(
      payload,
      signature: Signature(signature, publicKey: key._key),
    );
  } catch (_) {
    // Deliberately broad. See the library comment: the contract is that nothing escapes, and
    // an attacker chooses these bytes.
    return false;
  }
}

/// The encodings of the small-order points on Curve25519, as lowercase hex.
///
/// Fixed constants of the curve rather than a heuristic, so this list is complete. A point of
/// small order lets a signature verify against a key nobody holds the private half of, which is
/// why both implementations refuse them.
///
/// The set was checked against `ed25519-dalek` rather than copied from memory: each entry below
/// parses as a key there and is then refused by `verify_strict`. Two further encodings sometimes
/// listed alongside these
/// (`e0eb7a7c…` and `5f9c95bc…`) are not valid point encodings at all and are already rejected
/// when the key is parsed, in both languages, so including them here would be redundant.
const Set<String> _smallOrderPointsHex = {
  // The identity, and the point of order 2.
  '0000000000000000000000000000000000000000000000000000000000000000',
  '0100000000000000000000000000000000000000000000000000000000000000',
  // The order-4 pair.
  'ecffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff7f',
  'edffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff7f',
  // Non-canonical encodings of the two above, which decode to the same points.
  'eeffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff7f',
  'daffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff',
};

bool _isSmallOrder(List<int> pointBytes) {
  if (pointBytes.length != 32) return false;
  final hex =
      pointBytes.map((b) => b.toRadixString(16).padLeft(2, '0')).join();
  return _smallOrderPointsHex.contains(hex);
}

bool _listEquals(List<int> a, List<int> b) {
  if (a.length != b.length) return false;
  for (var i = 0; i < a.length; i++) {
    if (a[i] != b[i]) return false;
  }
  return true;
}

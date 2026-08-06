/// This device's own signing identity: one keypair, generated once, private half sealed.
///
/// Every other library in `domain/` answers a question about a command that arrived. This one
/// answers "who am I" — and holds the only thing in the app that can produce a signature
/// rather than check one.
///
/// # The private key cannot be read back, by construction
///
/// [DeviceIdentity] exposes exactly three things: `deviceId`, `verifyingKey`, and `sign`.
/// There is no `privateKey`, no `export`, no `toJson`, and [DeviceIdentity.toString] is
/// **written by hand** to print `<sealed>`.
///
/// This is not defensive habit. A key that can be read will eventually be read — into a log
/// line, an error message, a crash report, or a debugging aid added at 2am by someone trying
/// to work out why pairing failed. Making it unreachable removes the class of accident rather
/// than relying on every future contributor to avoid it.
///
/// Dart makes this trap easier to fall into than Rust does: the default `toString` is
/// harmless, but any field a future contributor adds to an interpolation is not, and
/// `jsonEncode` on an object with a `toJson` will happily serialise whatever it finds.
/// `test/domain/device_identity_source_test.dart` greps this file's source text and fails the
/// build if such a member appears.
///
/// # Never regenerate on failure
///
/// [loadOrGenerateIdentity] generates a keypair only when the database holds no identity
/// record. If the record says an identity exists but the secret store cannot produce the key,
/// that is a **failure**, not a reason to mint a new one.
///
/// A regenerated identity has a different [DeviceId], so every peer that paired with this
/// device would keep trusting a key it no longer holds: its commands would be refused as
/// coming from an unknown sender, and every pairing would have to be redone by hand at each
/// peer. A locked keystore is recoverable and usually temporary. A silently replaced identity
/// is neither.
///
/// Mirrors `desktop/src-tauri/src/domain/device_identity.rs`, and
/// `shared/testvectors/device_id_derivation.json` pins the identifier both sides derive.
library;

import 'package:cryptography/cryptography.dart';
import 'package:cryptography/dart.dart';

import '../core/app_error.dart';
import '../core/result.dart';
import '../platform/secret_store.dart';
import 'device_id.dart';
import 'signature.dart';

/// How many bytes of the digest become the identifier.
///
/// Truncated from SHA-256's 32 because the full digest is a 64-character string that will end
/// up in a UI. 128 bits is far beyond what a collision attack on a personal device set
/// requires — and a collision would have to be found against one *specific* paired key, not
/// merely between any two keys.
const int deviceIdBytes = 16;

/// The name the private key is filed under in the platform's secret store.
const String identitySecretName = 'device-identity-signing-key';

/// The length of an Ed25519 private key seed, in bytes.
const int _signingKeySeedBytes = 32;

final Ed25519 _ed25519 = Ed25519();

/// A device's identifier, derived from its verifying key.
///
/// The first [deviceIdBytes] bytes of SHA-256 over the 32 raw verifying-key bytes, lowercase
/// hex. The digest is taken over the key bytes themselves — not over their hex form, and not
/// over any prefix.
///
/// # Why derived rather than assigned
///
/// An independently assigned identifier can be claimed by anyone who learns it: an attacker
/// presents a key of their own under a paired device's name, and the target — which looks keys
/// up *by* name — would have to already know better to refuse it. Deriving the identifier from
/// the key makes the claim self-certifying. To use an identifier, a device must hold the key it
/// was derived from; nothing has to be trusted to enforce that, because a mismatched pair
/// simply does not hash to the claimed value.
///
/// Both implementations must agree byte for byte, or two paired devices would compute
/// different identifiers for the same key and be strangers to each other.
///
/// Synchronous, matching the Rust signature. `DartSha256` is the pure-Dart implementation
/// from the already-declared `cryptography` package and exposes `hashSync`; the package's
/// platform-dispatched `Sha256()` is `Future`-returning, which would make every caller async
/// for a hash over 32 bytes and would put an `await` inside the vector runner for no benefit.
DeviceId deriveDeviceId(List<int> verifyingKeyBytes) {
  const sha256 = DartSha256();
  final digest = sha256.hashSync(verifyingKeyBytes).bytes;

  final hex = digest
      .take(deviceIdBytes)
      .map((b) => b.toRadixString(16).padLeft(2, '0'))
      .join();

  return DeviceId(hex);
}

/// This device's signing identity.
///
/// # No `toJson`, no accessor for the private half, and a hand-written `toString`
///
/// Every one of those would be a route by which the key leaves this object, and the
/// source-text test fails the build if one appears. See the library comment.
final class DeviceIdentity {
  DeviceIdentity._(this._keyPair, this.deviceId, this.verifyingKey);

  /// The private half. Never returned, never printed, never serialised — the only operation
  /// performed with it is [sign].
  final SimpleKeyPair _keyPair;

  /// This device's identifier, derived from its verifying key.
  final DeviceId deviceId;

  /// The public half, which is not secret and is what peers record when they pair.
  final VerifyingKey verifyingKey;

  /// Signs [payload], which must be the canonical encoding from `encodeSigningPayload`.
  ///
  /// The signing happens *here*, inside the object that holds the key, rather than by handing
  /// the key to a caller that signs. That is the whole point: the capability offered is
  /// "produce a signature", never "have the key".
  Future<List<int>> sign(List<int> payload) async {
    final signature = await _ed25519.sign(payload, keyPair: _keyPair);
    return signature.bytes;
  }

  /// Prints the device id and the word `<sealed>` — written by hand, never generated.
  ///
  /// A `toString` that rendered the key would put it into any log line or interpolation a
  /// future contributor writes. That is precisely the accident this guards against.
  @override
  String toString() => 'DeviceIdentity($deviceId, signingKey: <sealed>)';
}

/// Rebuilds an identity from its private key seed.
///
/// Library-private in spirit: the only callers are [loadOrGenerateIdentity] and the tests.
/// Nothing else has a reason to hold key bytes, and the parameter here is the last point at
/// which they are visible as bytes at all.
Future<Result<DeviceIdentity>> identityFromSeed(List<int> seed) async {
  if (seed.length != _signingKeySeedBytes) {
    return Result.failure(
      const StorageError(message: 'the stored signing key is not the expected length'),
    );
  }

  final keyPair = await _ed25519.newKeyPairFromSeed(seed);
  final publicKey = await keyPair.extractPublicKey();
  final verifyingKey = VerifyingKey.fromBytes(publicKey.bytes);

  if (verifyingKey == null) {
    return Result.failure(
      const StorageError(
        message: 'the stored signing key does not yield a valid verifying key',
      ),
    );
  }

  return Result.success(
    DeviceIdentity._(keyPair, deriveDeviceId(publicKey.bytes), verifyingKey),
  );
}

/// The public half of an identity, as the database records it.
///
/// Holds no secret. Its purpose is to answer "who am I" without touching the secret store,
/// and — more importantly — to record that an identity *exists*. Without that record a failed
/// secret-store read is indistinguishable from a first run, and the system would regenerate.
final class DeviceIdentityRecord {
  const DeviceIdentityRecord({required this.deviceId, required this.verifyingKey});

  final DeviceId deviceId;
  final VerifyingKey verifyingKey;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      (other is DeviceIdentityRecord &&
          other.deviceId == deviceId &&
          other.verifyingKey == verifyingKey);

  @override
  int get hashCode => Object.hash(deviceId, verifyingKey);
}

/// Where the public half of the identity is recorded.
///
/// An interface rather than the concrete database so the identity logic is testable without
/// one, in the same way [SecretStore] makes it testable without a keystore.
abstract interface class DeviceIdentityRecordStore {
  /// The recorded identity, or null on a first run.
  Future<Result<DeviceIdentityRecord?>> loadIdentityRecord();

  /// Records a newly generated identity. Writes public material only.
  Future<Result<void>> saveIdentityRecord(DeviceIdentityRecord record);
}

/// Loads this device's identity, generating one only on a genuine first run.
///
/// # The three cases, and why the third is a failure
///
/// | Record | Secret store | Result |
/// | --- | --- | --- |
/// | absent | — | generate, store both halves |
/// | present | key found | reuse |
/// | present | key absent **or** unreachable | **failure** |
///
/// The third row is the one that matters. Generating a replacement there would give this
/// device a new [DeviceId], making it a stranger to every peer that had paired with it — their
/// pairings would keep naming a key it no longer holds, and every one would have to be redone
/// by hand. The storage problem is often temporary; the identity loss would not be.
///
/// Note that a *missing* entry is treated as a failure too when the record says one should
/// exist, not only an unreachable store. A record without a key means something removed the
/// key — that is a fault to report, and regenerating would paper over it while destroying the
/// pairings.
Future<Result<DeviceIdentity>> loadOrGenerateIdentity({
  required DeviceIdentityRecordStore records,
  required SecretStore secrets,
}) async {
  final recordResult = await records.loadIdentityRecord();
  if (recordResult case Failure(:final error)) {
    return Result.failure(error);
  }
  final record = recordResult.valueOrNull;

  if (record == null) {
    // A genuine first run: no record, so nothing can be lost by generating.
    final keyPair = await _ed25519.newKeyPair();
    final seed = await keyPair.extractPrivateKeyBytes();

    // The private half goes to the platform store and nowhere else. Written before the
    // record, so a crash between the two leaves a key with no record — which reads as a first
    // run and regenerates harmlessly. The opposite order would leave a record with no key,
    // which is the unrecoverable state below.
    final stored = await secrets.set(identitySecretName, seed);
    if (stored case Failure(:final error)) {
      return Result.failure(error);
    }

    final built = await identityFromSeed(seed);
    if (built case Failure(:final error)) {
      return Result.failure(error);
    }
    final identity = built.valueOrNull!;

    final saved = await records.saveIdentityRecord(
      DeviceIdentityRecord(
        deviceId: identity.deviceId,
        verifyingKey: identity.verifyingKey,
      ),
    );
    if (saved case Failure(:final error)) {
      return Result.failure(error);
    }

    return Result.success(identity);
  }

  // A later run. The key must be there; anything else is a fault.
  //
  // A store outage propagates as a failure from here rather than being flattened into "no
  // key" — that is exactly the distinction SecretStore preserves.
  final lookup = await secrets.get(identitySecretName);
  if (lookup case Failure(:final error)) {
    return Result.failure(error);
  }

  final found = lookup.valueOrNull;
  if (found is! SecretFound) {
    return Result.failure(
      StorageError(
        message: 'this device records an identity (${record.deviceId}) but its signing key '
            'is not in the secure store. Refusing to generate a replacement: a new identity '
            'would make this device unrecognisable to every device it is paired with, and '
            'those pairings would all have to be set up again.',
      ),
    );
  }

  final built = await identityFromSeed(found.secret);
  if (built case Failure(:final error)) {
    return Result.failure(error);
  }
  final identity = built.valueOrNull!;

  // The recorded public half and the stored private half must describe the same device. A
  // mismatch means one of the two was replaced independently, and proceeding would sign
  // commands under an identifier peers do not associate with this key.
  if (identity.deviceId != record.deviceId) {
    return Result.failure(
      StorageError(
        message: 'the signing key in the secure store belongs to device '
            '${identity.deviceId} but this device is recorded as ${record.deviceId}. '
            'Refusing to proceed rather than signing under an identity that does not match '
            'the key.',
      ),
    );
  }

  return Result.success(identity);
}

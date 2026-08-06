/// Signing, for tests only.
///
/// # Why this exists and why it lives under `test/`
///
/// The production library verifies signatures and cannot produce one: `lib/domain/signature.dart`
/// exposes no signing key type and no key generation. That is the guarantee, and it stays intact
/// — nothing under `lib/` imports this file.
///
/// Tests need it because the alternative is worse. Before this change, tests asserted
/// authenticity by setting a boolean to `true`; the whole point of the change is that no caller
/// can do that any more. A test helper that faked verification would reintroduce exactly the hole
/// being closed, one layer down, and every test would pass against an implementation that
/// verified nothing. So tests sign for real, with a real key, over the real canonical encoding.
///
/// # Keys come from fixed seeds
///
/// Ed25519 signing is deterministic, so a fixed seed yields a fixed key and a fixed signature —
/// no randomness and results that reproduce on every machine. Seed *n* here is the byte *n*
/// repeated 32 times, which is the same construction
/// `desktop/src-tauri/tests/signing_support/mod.rs` uses, so seed 1 is the same keypair in both
/// languages. That is what makes the cross-language fixture test meaningful.
library;

import 'package:cryptography/cryptography.dart';
import 'package:weakup/domain/domain.dart';

final Ed25519 _ed25519 = Ed25519();

/// A keypair a test can sign with, built from a fixed seed.
final class TestKeyPair {
  TestKeyPair._(this._keyPair, this.verifyingKey);

  /// The keypair for a given seed byte.
  ///
  /// Distinct seeds give distinct keys, which is what the wrong-key tests need: key A signs,
  /// the target holds key B, and the signature must fail.
  static Future<TestKeyPair> fromSeed(int seed) async {
    final keyPair = await _ed25519.newKeyPairFromSeed(
      List<int>.filled(32, seed),
    );
    final publicKey = await keyPair.extractPublicKey();
    final verifying = VerifyingKey.fromBytes(publicKey.bytes);
    if (verifying == null) {
      throw StateError('a key derived from a signing key must be valid');
    }
    return TestKeyPair._(keyPair, verifying);
  }

  final SimpleKeyPair _keyPair;

  /// The public half, as the domain's verifying key type.
  final VerifyingKey verifyingKey;

  /// Signs the canonical encoding of a command's signed content.
  ///
  /// Deliberately goes through [encodeSigningPayload] rather than signing bytes the test
  /// supplies: a test that signed a different encoding than the rule verifies would pass or
  /// fail for reasons unrelated to what it claims to check.
  Future<List<int>> signCommand({
    required DeviceId sender,
    required String command,
    required DateTime createdAt,
    required String nonce,
  }) =>
      signBytes(encodeSigningPayload(
        sender: sender,
        command: command,
        createdAt: createdAt,
        nonce: nonce,
      ));

  /// Signs arbitrary bytes, for the cross-language fixture test.
  Future<List<int>> signBytes(List<int> payload) async {
    final signature = await _ed25519.sign(payload, keyPair: _keyPair);
    return signature.bytes;
  }
}

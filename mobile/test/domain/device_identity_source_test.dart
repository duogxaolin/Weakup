/// The structural guarantee for the device identity: the private key cannot be read back.
///
/// # Why this file exists separately
///
/// The test below greps `device_identity.dart` for member names that must never appear in it.
/// Put inside the same file, or in a test that also declared those names in its own strings,
/// it would match itself. That trap has bitten this codebase before, which is why
/// `transport_source_test.dart` is also separate.
///
/// # Why source text rather than review
///
/// Because the failure is additive. Someone adds an innocuous-looking `privateKey` getter
/// while debugging why pairing fails, or a `toJson` so the identity can be logged. Every
/// existing test still passes, and the key is now one interpolation away from a log file.
library;

import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

void main() {
  /// Read from disk rather than embedded, so the test cannot drift from the file it guards.
  final source = File('lib/domain/device_identity.dart').readAsStringSync();

  test('the identity exposes no member that returns private key material', () {
    // A key that can be read will eventually be read — into a log line, an error message, a
    // crash report, or a debugging aid added under time pressure. Because such a member would
    // be *added* rather than break anything existing, its absence is asserted rather than
    // assumed.
    //
    // Matched against declaration-shaped text rather than the bare words, so prose in a doc
    // comment mentioning "the private key" does not trip the test: what is forbidden is a
    // member, not the words.
    for (final forbidden in [
      'get privateKey',
      'get secretKey',
      'get signingKey',
      'get keyPair',
      'privateKey()',
      'export()',
      'exportPrivateKey',
      'extractPrivateKeyBytes()',
      'Map<String, dynamic> toJson',
      'toJson()',
    ]) {
      final offending = source
          .split('\n')
          .where((line) {
            final trimmed = line.trimLeft();
            // Comments may discuss these; only code may not contain them.
            return !trimmed.startsWith('//') && !trimmed.startsWith('///');
          })
          .where((line) => line.contains(forbidden))
          // The one legitimate use: reading the seed out of a freshly generated keypair, to
          // hand straight to the secret store. It never leaves `loadOrGenerateIdentity`, and
          // it happens before any DeviceIdentity exists to leak from.
          .where((line) => !line.contains('final seed = await keyPair'))
          .toList();

      expect(
        offending,
        isEmpty,
        reason: 'device_identity.dart contains "$forbidden", which suggests a member '
            'returning private key material. The identity signs; it never hands the key '
            'out: $offending',
      );
    }
  });

  test('the private key field is private and the class exposes only three members', () {
    // The positive form of the guard above: rather than only forbidding names, this pins what
    // the class *does* expose, so a member added under an unforeseen name is still caught by
    // the review this test's failure prompts.
    expect(
      source,
      contains('final SimpleKeyPair _keyPair'),
      reason: 'the key must be held in a private field',
    );
    expect(
      source,
      isNot(contains('final SimpleKeyPair keyPair')),
      reason: 'a public key-pair field would hand the private half to every caller',
    );
  });

  test('toString is written by hand and prints sealed rather than the key', () {
    // A `toString` that rendered the key would put it into any log line or string
    // interpolation a future contributor writes.
    expect(source, contains('<sealed>'));
    expect(
      source,
      contains('String toString() =>'),
      reason: 'toString must be written by hand rather than inherited or generated',
    );
  });

  test('the identity is never serialised', () {
    // `jsonEncode` on an object with a `toJson` will happily serialise whatever it finds, so
    // the absence of one is what keeps the key out of any JSON this app writes.
    //
    // Comment lines are skipped: the implementation's own doc comments explain *why* there is
    // no `toJson`, and that prose is the documentation of this guarantee rather than a breach
    // of it. Only code may not contain the member.
    final codeLines = source.split('\n').where((line) {
      final trimmed = line.trimLeft();
      return !trimmed.startsWith('//') && !trimmed.startsWith('///');
    });

    expect(
      codeLines.where((line) => line.contains('toJson')),
      isEmpty,
      reason: 'a toJson on the identity would be a route by which the key reaches a file',
    );
  });
}

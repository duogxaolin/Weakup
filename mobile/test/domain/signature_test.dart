// Unit tests for the canonical signing payload and Ed25519 verification.
//
// The shared vectors prove the two implementations produce identical bytes; these prove the
// local properties, including the never-throw guarantee on attacker-controlled input.

import 'package:flutter_test/flutter_test.dart';
import 'package:weakup/domain/domain.dart';

import 'test_signing.dart';

DateTime atMillis(int millis) =>
    DateTime.fromMillisecondsSinceEpoch(millis, isUtc: true);

List<int> encode({
  String sender = 'phone-a',
  String command = 'powerOff',
  int millis = 0,
  String nonce = 'n',
}) =>
    encodeSigningPayload(
      sender: DeviceId(sender),
      command: command,
      createdAt: atMillis(millis),
      nonce: nonce,
    );

void main() {
  group('the canonical encoding', () {
    test('a field is prefixed by its byte length', () {
      final encoded = encode(sender: 'ab');

      expect(encoded.sublist(0, 4), [0, 0, 0, 2]);
      expect(encoded.sublist(4, 6), 'ab'.codeUnits);
    });

    test('the length prefix counts bytes, not characters', () {
      // Two characters, four bytes. Dart makes this trap easy: `String.length` is UTF-16 code
      // units, so an implementation using it disagrees with Rust on exactly this input.
      final encoded = encode(sender: 'Ωλ');

      expect(encoded.sublist(0, 4), [0, 0, 0, 4]);
    });

    test('distinct field divisions encode differently', () {
      // The forgery argument, as an assertion. Under a delimiter encoding these two would be
      // the same bytes and one signature would cover both.
      expect(encode(sender: 'ab', nonce: 'c'), isNot(encode(sender: 'a', nonce: 'bc')));
    });

    test('a nonce full of delimiters is ordinary data', () {
      // Nothing about `|` or `:` is special here, which is the point of not using them.
      expect(encode(nonce: 'a|b:c'), encode(nonce: 'a|b:c'));
      expect(encode(nonce: 'a|b:c'), isNot(encode(nonce: 'a|b:d')));
    });

    test('every signed field changes the encoding', () {
      // A field outside the encoding is a field an intermediary can change undetected.
      final baseline = encode(sender: 'phone-a', command: 'powerOff', millis: 1000, nonce: 'n-1');

      expect(baseline, isNot(encode(sender: 'phone-b', millis: 1000, nonce: 'n-1')),
          reason: 'sender is not covered');
      expect(baseline, isNot(encode(command: 'keepAwake', millis: 1000, nonce: 'n-1')),
          reason: 'command is not covered');
      expect(baseline, isNot(encode(millis: 2000, nonce: 'n-1')),
          reason: 'createdAt is not covered');
      expect(baseline, isNot(encode(millis: 1000, nonce: 'n-2')),
          reason: 'nonce is not covered');
    });

    test('the epoch is a single unpadded digit', () {
      final encoded = encode(sender: 'a', command: 'x', millis: 0, nonce: 'n');

      // sender(4+1) + command(4+1) = 10 bytes, then the millis field.
      expect(encoded.sublist(10, 14), [0, 0, 0, 1]);
      expect(encoded.sublist(14, 15), '0'.codeUnits);
    });

    test('sub-second precision is preserved', () {
      // Truncating to whole seconds would let two commands a few hundred milliseconds apart
      // share an encoding, and therefore a signature.
      expect(encode(millis: 1500), isNot(encode(millis: 1000)));
    });

    test('a local DateTime encodes the same as its UTC equivalent', () {
      // The same instant is the same instant. If this ever differs, the signature depends on
      // the sending device's timezone, which the other side cannot know.
      final utc = DateTime.utc(2026, 8, 6, 12);
      final local = utc.toLocal();

      expect(
        encodeSigningPayload(
          sender: DeviceId('phone-a'),
          command: 'powerOff',
          createdAt: local,
          nonce: 'n',
        ),
        encodeSigningPayload(
          sender: DeviceId('phone-a'),
          command: 'powerOff',
          createdAt: utc,
          nonce: 'n',
        ),
      );
    });
  });

  group('verification', () {
    late TestKeyPair key;

    setUpAll(() async {
      key = await TestKeyPair.fromSeed(1);
    });

    test('a genuine signature verifies', () async {
      final payload = encode();
      final signature = await key.signBytes(payload);

      expect(
        await verifySignature(
          payload: payload,
          signature: signature,
          key: key.verifyingKey,
        ),
        isTrue,
      );
    });

    test('a signature over different bytes does not verify', () async {
      final signature = await key.signBytes(encode(nonce: 'n-one'));

      expect(
        await verifySignature(
          payload: encode(nonce: 'n-two'),
          signature: signature,
          key: key.verifyingKey,
        ),
        isFalse,
      );
    });

    test('a signature by another key does not verify', () async {
      final other = await TestKeyPair.fromSeed(9);
      final payload = encode();
      final signature = await other.signBytes(payload);

      expect(
        await verifySignature(
          payload: payload,
          signature: signature,
          key: key.verifyingKey,
        ),
        isFalse,
      );
    });

    test('a malformed signature is false and does not throw', () async {
      expect(
        await verifySignature(
          payload: encode(),
          signature: List<int>.filled(signatureBytes, 0x42),
          key: key.verifyingKey,
        ),
        isFalse,
      );
    });

    test('a wrong-length signature is false and does not throw', () async {
      // The length check must come before anything that would index into the buffer, and
      // before the package's own check, which throws rather than returning false.
      for (final length in [0, 1, 32, 63, 65, 128]) {
        expect(
          await verifySignature(
            payload: encode(),
            signature: List<int>.filled(length, 0x42),
            key: key.verifyingKey,
          ),
          isFalse,
          reason: 'a $length-byte signature must be refused',
        );
      }
    });

    test('an all-zeroes signature is false and does not throw', () async {
      // The degenerate case, and the one a naive implementation is most likely to accept: an
      // all-zero R component is a small-order point.
      expect(
        await verifySignature(
          payload: encode(),
          signature: List<int>.filled(signatureBytes, 0),
          key: key.verifyingKey,
        ),
        isFalse,
      );
    });

    test('random bytes as a signature never throw', () async {
      // The adversarial case stated as its own test rather than assumed: an attacker
      // supplies the signature, so verification must be total over every possible input. A
      // cheap deterministic generator, so a failure is reproducible.
      var state = 0x2545f491;
      int nextByte() {
        state = (state * 1103515245 + 12345) & 0x7fffffff;
        return (state >> 16) & 0xff;
      }

      for (var i = 0; i < 64; i++) {
        final length = nextByte() % 130;
        final signature = [for (var j = 0; j < length; j++) nextByte()];

        // The assertion is that this returns at all. A throw fails the test.
        expect(
          await verifySignature(
            payload: encode(),
            signature: signature,
            key: key.verifyingKey,
          ),
          isFalse,
        );
      }
    });
  });

  group('VerifyingKey', () {
    test('a key of the wrong length is refused rather than throwing', () {
      expect(VerifyingKey.fromBytes(<int>[]), isNull);
      expect(VerifyingKey.fromBytes(List<int>.filled(31, 0)), isNull);
      expect(VerifyingKey.fromBytes(List<int>.filled(33, 0)), isNull);
      expect(VerifyingKey.fromBytes(List<int>.filled(64, 0)), isNull);
    });

    test('an all-zeroes key verifies nothing', () async {
      // It parses — it is the right length — and is refused at verification instead, as a
      // small-order point. Both paths end at false, which is all a caller sees.
      final key = VerifyingKey.fromBytes(List<int>.filled(verifyingKeyBytes, 0));
      if (key == null) return;

      expect(
        await verifySignature(
          payload: encode(),
          signature: List<int>.filled(signatureBytes, 0),
          key: key,
        ),
        isFalse,
      );
    });

    test('a key round-trips through its bytes', () async {
      final pair = await TestKeyPair.fromSeed(1);
      final rebuilt = VerifyingKey.fromBytes(pair.verifyingKey.bytes);

      expect(rebuilt, pair.verifyingKey);
      expect(rebuilt!.bytes, pair.verifyingKey.bytes);
    });
  });
}

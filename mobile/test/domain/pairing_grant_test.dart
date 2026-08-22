// Unit tests for the pairing grant rule.
//
// The shared vectors prove the two implementations agree; these prove the local rule.

import 'package:flutter_test/flutter_test.dart';
import 'package:weakup/domain/domain.dart';

final DateTime issued = DateTime.utc(2026, 8, 6, 12);

PairingGrant grant(
  GrantDelivery delivery, {
  bool redeemed = false,
  bool recognised = true,
}) =>
    PairingGrant(
      delivery: delivery,
      issuedAt: issued,
      redeemed: redeemed,
      recognised: recognised,
    );

DateTime at(int offsetSeconds) => issued.add(Duration(seconds: offsetSeconds));

void main() {
  group('the lifetime boundaries', () {
    test('a grant at exactly its lifetime is still valid', () {
      // Both boundaries are inclusive-accept, and each is expressed against the constant
      // rather than a literal so changing the constant moves the test with it.
      for (final delivery in GrantDelivery.values) {
        expect(
          evaluateGrant(
            grant: grant(delivery),
            now: at(delivery.lifetimeSeconds),
          ),
          const ValidGrant(),
          reason: '$delivery at exactly its lifetime must be valid',
        );
      }
    });

    test('a grant one second past its lifetime has expired', () {
      for (final delivery in GrantDelivery.values) {
        expect(
          evaluateGrant(
            grant: grant(delivery),
            now: at(delivery.lifetimeSeconds + 1),
          ),
          const InvalidGrant(GrantRejection.expired),
          reason: '$delivery one second past its lifetime must be expired',
        );
      }
    });

    test('an age between the two lifetimes splits by delivery path', () {
      // The case that proves the two lifetimes are actually distinct. An implementation using
      // one lifetime for both passes every other test in this file.
      final between = at(
        (atMachineGrantLifetimeSeconds + outOfBandGrantLifetimeSeconds) ~/ 2,
      );

      expect(
        evaluateGrant(grant: grant(GrantDelivery.atMachine), now: between),
        const InvalidGrant(GrantRejection.expired),
      );
      expect(
        evaluateGrant(grant: grant(GrantDelivery.outOfBand), now: between),
        const ValidGrant(),
      );
    });

    test('the two lifetimes are not the same value', () {
      // Guards the split test above: if a later edit made them equal, that test could pass
      // vacuously depending on the midpoint.
      expect(atMachineGrantLifetimeSeconds,
          isNot(outOfBandGrantLifetimeSeconds));
    });

    test('the lifetimes match the Rust ones', () {
      // A cross-language contract, fixed by the shared vectors.
      expect(atMachineGrantLifetimeSeconds, 300);
      expect(outOfBandGrantLifetimeSeconds, 600);
    });
  });

  group('the precedence order', () {
    test('an already-used grant reports that rather than expiry', () {
      final used = grant(GrantDelivery.atMachine, redeemed: true);

      // Before expiry.
      expect(
        evaluateGrant(grant: used, now: at(60)),
        const InvalidGrant(GrantRejection.alreadyUsed),
      );

      // And after it — the precedence pair. Both reasons hold; single use is the one worth
      // reporting, because it is the one that will not change by waiting.
      expect(
        evaluateGrant(grant: used, now: at(outOfBandGrantLifetimeSeconds * 2)),
        const InvalidGrant(GrantRejection.alreadyUsed),
      );
    });

    test('an unrecognised grant is distinguished from an expired one', () {
      final unknown = grant(GrantDelivery.atMachine, recognised: false);

      expect(
        evaluateGrant(grant: unknown, now: at(1)),
        const InvalidGrant(GrantRejection.noSuchGrant),
      );

      // Recognition is settled before any age is computed from fields the target does not
      // stand behind.
      expect(
        evaluateGrant(
          grant: unknown,
          now: at(outOfBandGrantLifetimeSeconds * 2),
        ),
        const InvalidGrant(GrantRejection.noSuchGrant),
      );
    });
  });

  group('the decision type and its messages', () {
    test('a decision carries a reason exactly when it is a refusal', () {
      expect(const ValidGrant().rejection, isNull);
      expect(const ValidGrant().isValid, isTrue);

      const invalid = InvalidGrant(GrantRejection.expired);
      expect(invalid.rejection, GrantRejection.expired);
      expect(invalid.isValid, isFalse);
    });

    test('every rejection has a distinct message a person can act on', () {
      final messages = GrantRejection.values.map((r) => r.userMessage).toList();

      for (final message in messages) {
        expect(message.trim(), isNotEmpty);
        expect(
          message.contains(RegExp('[a-z]')),
          isTrue,
          reason: 'messages are prose, not identifiers: $message',
        );
      }

      expect(messages.toSet().length, messages.length,
          reason: 'two reasons share a message');
    });

    test('wire names round-trip against the vector strings', () {
      expect(GrantDelivery.atMachine.wireName, 'atMachine');
      expect(GrantDelivery.outOfBand.wireName, 'outOfBand');
      expect(GrantRejection.expired.wireName, 'expired');
      expect(GrantRejection.alreadyUsed.wireName, 'alreadyUsed');
      expect(GrantRejection.noSuchGrant.wireName, 'noSuchGrant');
    });
  });
}

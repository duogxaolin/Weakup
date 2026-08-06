// Unit tests for the command acceptance rule.
//
// The shared vectors prove the two implementations agree; these prove the local rule,
// including the no-probing property the spec requires.

import 'package:flutter_test/flutter_test.dart';
import 'package:weakup/domain/domain.dart';

final DateTime created = DateTime.utc(2026, 8, 6, 12);

CommandEnvelope envelope({
  RemoteCommand command = RemoteCommand.powerOff,
  String nonce = 'n-fresh',
  bool signatureVerified = true,
}) =>
    CommandEnvelope(
      sender: DeviceId('phone-a'),
      command: command,
      createdAt: created,
      nonce: nonce,
      signatureVerified: signatureVerified,
    );

CommandTargetState permissiveTarget({
  Set<String>? seenNonces,
  bool targetCanPowerOff = true,
  bool targetIsRemoteTarget = true,
  bool isPaired = true,
  bool remoteControlEnabled = true,
}) =>
    CommandTargetState(
      seenNonces: seenNonces ?? <String>{},
      targetCanPowerOff: targetCanPowerOff,
      targetIsRemoteTarget: targetIsRemoteTarget,
      isPaired: isPaired,
      remoteControlEnabled: remoteControlEnabled,
    );

DateTime at(int offsetSeconds) => created.add(Duration(seconds: offsetSeconds));

void main() {
  group('the no-probing requirement', () {
    test('an unverified signature reports no permission reason', () {
      // The spec's no-probing requirement, and the reason authenticity is checked first: a
      // caller must not be able to learn the target's permission state by sending
      // unauthenticated commands. Here every permission check would also fail, and none of
      // that may surface.
      final decision = evaluateCommand(
        envelope: envelope(signatureVerified: false),
        targetState: permissiveTarget(
          targetCanPowerOff: false,
          targetIsRemoteTarget: false,
          isPaired: false,
          remoteControlEnabled: false,
        ),
        now: created,
      );

      expect(
        decision,
        const RejectedCommand(RejectionReason.authenticityUnverified),
      );
      // Stated as its own expectation rather than implied by the equality above: the
      // property is that no permission reason is reported.
      expect(
        decision.rejectionReason,
        isNot(RejectionReason.notPermitted),
        reason: 'an unauthenticated caller must not learn the permission state',
      );
    });

    test('an unverified signature hides replay and freshness state too', () {
      // Every reason after the first leaks something about the target. A caller must not be
      // able to discover which nonces this target has seen, either.
      expect(
        evaluateCommand(
          envelope: envelope(signatureVerified: false, nonce: 'n-seen'),
          targetState: permissiveTarget(seenNonces: {'n-seen'}),
          now: created,
        ),
        const RejectedCommand(RejectionReason.authenticityUnverified),
      );
    });
  });

  group('the precedence order', () {
    test('replay is reported before either time reason', () {
      // A replayed command is evidence of an attack; a stale one is more often a bad
      // network. Reporting the more serious finding when both hold means the record shows an
      // attack as an attack.
      final target = permissiveTarget(seenNonces: {'n-fresh'});

      expect(
        evaluateCommand(
          envelope: envelope(),
          targetState: target,
          now: at(freshnessWindowSeconds + 60),
        ),
        const RejectedCommand(RejectionReason.replayedNonce),
      );

      expect(
        evaluateCommand(
          envelope: envelope(),
          targetState: target,
          now: at(-(futureToleranceSeconds + 60)),
        ),
        const RejectedCommand(RejectionReason.replayedNonce),
      );
    });

    test('future dating and staleness are each reported before permission', () {
      // notPermitted is last because it leaks the most — whether remote control is enabled
      // and what the platform can do — and is reached only by a command already authentic,
      // fresh, and new.
      final forbidden = permissiveTarget(remoteControlEnabled: false);

      expect(
        evaluateCommand(
          envelope: envelope(),
          targetState: forbidden,
          now: at(-(futureToleranceSeconds + 1)),
        ),
        const RejectedCommand(RejectionReason.futureDated),
      );

      expect(
        evaluateCommand(
          envelope: envelope(),
          targetState: forbidden,
          now: at(freshnessWindowSeconds + 1),
        ),
        const RejectedCommand(RejectionReason.stale),
      );
    });

    test('every reason holding at once still reports authenticity', () {
      expect(
        evaluateCommand(
          envelope: envelope(
            command: RemoteCommand.enableRemoteControl,
            nonce: 'n-seen',
            signatureVerified: false,
          ),
          targetState: permissiveTarget(
            seenNonces: {'n-seen'},
            targetCanPowerOff: false,
            targetIsRemoteTarget: false,
            isPaired: false,
            remoteControlEnabled: false,
          ),
          now: at(-(futureToleranceSeconds + 60)),
        ),
        const RejectedCommand(RejectionReason.authenticityUnverified),
      );
    });
  });

  group('the boundaries', () {
    test('a command at exactly the freshness window is accepted', () {
      // Inclusive-accept, and expressed against the constant rather than a literal so
      // changing the constant moves the test with it.
      expect(
        evaluateCommand(
          envelope: envelope(),
          targetState: permissiveTarget(),
          now: at(freshnessWindowSeconds),
        ),
        const AcceptedCommand(),
      );
    });

    test('a command one second past the freshness window is stale', () {
      expect(
        evaluateCommand(
          envelope: envelope(),
          targetState: permissiveTarget(),
          now: at(freshnessWindowSeconds + 1),
        ),
        const RejectedCommand(RejectionReason.stale),
      );
    });

    test('a command at exactly the future tolerance is accepted', () {
      // Ordinary clock skew between two honest devices must not refuse a valid command.
      expect(
        evaluateCommand(
          envelope: envelope(),
          targetState: permissiveTarget(),
          now: at(-futureToleranceSeconds),
        ),
        const AcceptedCommand(),
      );
    });

    test('one second past the future tolerance is futureDated, not stale', () {
      // The two indicate different faults, and a user shown "expired" for a clock-skew
      // problem will spend the evening looking in the wrong place.
      expect(
        evaluateCommand(
          envelope: envelope(),
          targetState: permissiveTarget(),
          now: at(-(futureToleranceSeconds + 1)),
        ),
        const RejectedCommand(RejectionReason.futureDated),
      );
    });
  });

  group('replay', () {
    test('a seen nonce is refused and a distinct one is not', () {
      // Without the second half, an implementation could refuse every command from a device
      // that had ever sent one and still pass the replay case.
      final target = permissiveTarget(seenNonces: {'n-one', 'n-two'});

      expect(
        evaluateCommand(
          envelope: envelope(nonce: 'n-two'),
          targetState: target,
          now: created,
        ),
        const RejectedCommand(RejectionReason.replayedNonce),
      );

      expect(
        evaluateCommand(
          envelope: envelope(nonce: 'n-three'),
          targetState: target,
          now: created,
        ),
        const AcceptedCommand(),
      );
    });
  });

  group('permission is delegated, not reimplemented', () {
    test('every remotely permitted command is accepted when nothing is wrong', () {
      for (final command in [
        RemoteCommand.powerOff,
        RemoteCommand.keepAwake,
        RemoteCommand.cancelJob,
      ]) {
        expect(
          evaluateCommand(
            envelope: envelope(command: command),
            targetState: permissiveTarget(),
            now: created,
          ),
          const AcceptedCommand(),
          reason: '$command should be accepted',
        );
      }
    });

    test('the specific permission reason is not disclosed', () {
      // Acceptance reports only that the command was not permitted. The authorization rule
      // keeps its own reason set for the question it answers.
      for (final target in [
        permissiveTarget(remoteControlEnabled: false),
        permissiveTarget(isPaired: false),
        permissiveTarget(targetCanPowerOff: false),
      ]) {
        expect(
          evaluateCommand(
            envelope: envelope(),
            targetState: target,
            now: created,
          ),
          const RejectedCommand(RejectionReason.notPermitted),
        );
      }

      expect(
        evaluateCommand(
          envelope: envelope(command: RemoteCommand.enableRemoteControl),
          targetState: permissiveTarget(),
          now: created,
        ),
        const RejectedCommand(RejectionReason.notPermitted),
      );
    });
  });

  group('the decision type, the constants, and the messages', () {
    test('a decision carries a reason exactly when it is a refusal', () {
      expect(const AcceptedCommand().rejectionReason, isNull);
      expect(const AcceptedCommand().isAccepted, isTrue);

      const rejected = RejectedCommand(RejectionReason.stale);
      expect(rejected.rejectionReason, RejectionReason.stale);
      expect(rejected.isAccepted, isFalse);
    });

    test('nonce retention outlasts the acceptance window', () {
      // The Dart equivalent of the Rust compile-time assertion, and weaker than it: there,
      // shortening retention fails the build. Here it fails this test, plus an `assert` that
      // is stripped in release builds.
      expect(nonceRetentionOutlastsAcceptanceWindow, isTrue);
      expect(
        nonceRetentionSeconds,
        greaterThan(freshnessWindowSeconds + futureToleranceSeconds),
      );
    });

    test('freshness is not coupled to the presence threshold', () {
      // The two answer different questions and reusing one constant for both would make
      // either unchangeable without the other.
      expect(freshnessWindowSeconds, isNot(onlineThresholdSeconds));
    });

    test('the constants match the Rust ones', () {
      // These values are a cross-language contract, fixed by the shared vectors.
      expect(freshnessWindowSeconds, 120);
      expect(futureToleranceSeconds, 30);
      expect(nonceRetentionSeconds, 600);
    });

    test('every reason has a distinct message a person can act on', () {
      final messages = RejectionReason.values.map((r) => r.userMessage).toList();

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
      expect(
        RejectionReason.authenticityUnverified.wireName,
        'authenticityUnverified',
      );
      expect(RejectionReason.replayedNonce.wireName, 'replayedNonce');
      expect(RejectionReason.futureDated.wireName, 'futureDated');
      expect(RejectionReason.stale.wireName, 'stale');
      expect(RejectionReason.notPermitted.wireName, 'notPermitted');
    });
  });

  group('DeviceId', () {
    test('an empty id is refused', () {
      // Two empty ids would compare equal, making two unrelated devices one device.
      expect(() => DeviceId(''), throwsArgumentError);
      expect(() => DeviceId('   '), throwsArgumentError);
      expect(DeviceId.tryParse(''), isNull);
    });

    test('no structure is imposed on the value', () {
      // If a later change makes one of these fail, it has embedded an assumption about the
      // account model into an identifier that must not carry one.
      for (final value in [
        '3f2504e0-4f89-11d3-9a0c-0305e82c3301',
        'opaque-token',
        'device@example',
      ]) {
        expect(DeviceId(value).value, value);
      }
    });

    test('two ids with the same value are the same device', () {
      expect(DeviceId('phone-a'), DeviceId('phone-a'));
      expect(DeviceId('phone-a'), isNot(DeviceId('phone-b')));
      expect(DeviceId('phone-a').hashCode, DeviceId('phone-a').hashCode);
    });
  });
}

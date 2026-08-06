// Unit tests for the command acceptance rule.
//
// The shared vectors prove the two implementations agree; these prove the local rule,
// including the no-probing property the spec requires.

import 'package:flutter_test/flutter_test.dart';
import 'package:weakup/domain/domain.dart';

import 'test_signing.dart';

final DateTime created = DateTime.utc(2026, 8, 6, 12);

/// The key the sending device signs with, and whose public half the target holds.
late TestKeyPair senderKey;

/// A key nobody legitimate holds, for the wrong-key cases.
late TestKeyPair attackerKey;

/// An envelope signed by [key] over exactly the fields it carries.
///
/// The signature is **real**: produced by signing the canonical encoding with a real key, and
/// checked by the rule with real arithmetic. There is no helper here that makes verification
/// succeed without one. Before this change these tests set `signatureVerified: true`, which is
/// precisely the shortcut the change exists to remove — reintroducing it as a test helper would
/// leave every test below passing against an implementation that verified nothing.
///
/// Every variation goes through here rather than editing a field of an already-signed envelope,
/// so a case meaning "a valid command with a different nonce" carries a signature valid for
/// *that* nonce.
Future<CommandEnvelope> envelope({
  RemoteCommand command = RemoteCommand.powerOff,
  String nonce = 'n-fresh',
  DateTime? createdAt,
  TestKeyPair? key,
}) async {
  final sender = DeviceId('phone-a');
  final at = createdAt ?? created;
  final signer = key ?? senderKey;

  return CommandEnvelope(
    sender: sender,
    command: command,
    createdAt: at,
    nonce: nonce,
    signature: await signer.signCommand(
      sender: sender,
      command: command.wireName,
      createdAt: at,
      nonce: nonce,
    ),
  );
}

/// An envelope whose signature is well-formed garbage rather than a real one.
Future<CommandEnvelope> unsignedEnvelope({
  RemoteCommand command = RemoteCommand.powerOff,
  String nonce = 'n-fresh',
}) async {
  final valid = await envelope(command: command, nonce: nonce);
  return CommandEnvelope(
    sender: valid.sender,
    command: valid.command,
    createdAt: valid.createdAt,
    nonce: valid.nonce,
    signature: List<int>.filled(signatureBytes, 0x42),
  );
}

CommandTargetState permissiveTarget({
  Set<String>? seenNonces,
  Map<DeviceId, VerifyingKey>? verifyingKeys,
  bool targetCanPowerOff = true,
  bool targetIsRemoteTarget = true,
  bool isPaired = true,
  bool remoteControlEnabled = true,
}) =>
    CommandTargetState(
      seenNonces: seenNonces ?? <String>{},
      verifyingKeys:
          verifyingKeys ?? {DeviceId('phone-a'): senderKey.verifyingKey},
      targetCanPowerOff: targetCanPowerOff,
      targetIsRemoteTarget: targetIsRemoteTarget,
      isPaired: isPaired,
      remoteControlEnabled: remoteControlEnabled,
    );

DateTime at(int offsetSeconds) => created.add(Duration(seconds: offsetSeconds));

void main() {
  setUpAll(() async {
    senderKey = await TestKeyPair.fromSeed(1);
    attackerKey = await TestKeyPair.fromSeed(9);
  });

  group('authenticity is verified, not asserted', () {
    test('a command signed by the wrong key is refused though all else is valid', () async {
      // The property the previous change specified and could not enforce. This command is
      // fresh, unreplayed, and permitted; only the key differs.
      expect(
        await evaluateCommand(
          envelope: await envelope(key: attackerKey),
          targetState: permissiveTarget(),
          now: created,
        ),
        const RejectedCommand(RejectionReason.authenticityUnverified),
      );
    });

    test('a sender the target holds no key for is refused on authenticity grounds', () async {
      // Not a distinct reason, deliberately. Distinguishing "I hold no key for that device"
      // from "the signature did not verify" would tell an attacker which device identifiers
      // this target knows.
      expect(
        await evaluateCommand(
          envelope: await envelope(),
          targetState: permissiveTarget(verifyingKeys: {}),
          now: created,
        ),
        const RejectedCommand(RejectionReason.authenticityUnverified),
      );
    });

    test('an unknown sender and a bad signature are indistinguishable', () async {
      // The two failure modes must be reported identically, or the difference is an oracle.
      final unknownSender = await evaluateCommand(
        envelope: await envelope(),
        targetState: permissiveTarget(verifyingKeys: {}),
        now: created,
      );
      final badSignature = await evaluateCommand(
        envelope: await unsignedEnvelope(),
        targetState: permissiveTarget(),
        now: created,
      );

      expect(unknownSender, badSignature);
    });

    test('a key held for a different device does not authenticate this sender', () async {
      // A target paired with several devices must check the claimed sender's key
      // specifically, or one paired device could issue commands in another's name.
      expect(
        await evaluateCommand(
          envelope: await envelope(),
          targetState: permissiveTarget(
            verifyingKeys: {DeviceId('phone-b'): senderKey.verifyingKey},
          ),
          now: created,
        ),
        const RejectedCommand(RejectionReason.authenticityUnverified),
      );
    });

    test('altering any signed field after signing invalidates the command', () async {
      // Every field the signature covers, varied one at a time against a valid baseline. A
      // field that survives alteration is one a relay could rewrite in transit.
      final valid = await envelope();
      expect(
        await evaluateCommand(
          envelope: valid,
          targetState: permissiveTarget(),
          now: created,
        ),
        const AcceptedCommand(),
        reason: 'the baseline must be accepted or the mutations below prove nothing',
      );

      // Both devices map to the same key, so an altered *sender* fails on the signature
      // rather than on a missing entry.
      final bothKeys = permissiveTarget(verifyingKeys: {
        DeviceId('phone-a'): senderKey.verifyingKey,
        DeviceId('phone-b'): senderKey.verifyingKey,
      });

      final mutations = <String, (CommandEnvelope, CommandTargetState, DateTime)>{
        'sender': (
          CommandEnvelope(
            sender: DeviceId('phone-b'),
            command: valid.command,
            createdAt: valid.createdAt,
            nonce: valid.nonce,
            signature: valid.signature,
          ),
          bothKeys,
          created,
        ),
        'command': (
          CommandEnvelope(
            sender: valid.sender,
            command: RemoteCommand.keepAwake,
            createdAt: valid.createdAt,
            nonce: valid.nonce,
            signature: valid.signature,
          ),
          permissiveTarget(),
          created,
        ),
        'createdAt': (
          CommandEnvelope(
            sender: valid.sender,
            command: valid.command,
            createdAt: at(1),
            nonce: valid.nonce,
            signature: valid.signature,
          ),
          permissiveTarget(),
          // Judged at the altered instant, so only the signature can refuse it.
          at(1),
        ),
        'nonce': (
          CommandEnvelope(
            sender: valid.sender,
            command: valid.command,
            createdAt: valid.createdAt,
            nonce: 'n-altered',
            signature: valid.signature,
          ),
          permissiveTarget(),
          created,
        ),
      };

      for (final entry in mutations.entries) {
        final (tampered, target, now) = entry.value;
        expect(
          await evaluateCommand(
            envelope: tampered,
            targetState: target,
            now: now,
          ),
          const RejectedCommand(RejectionReason.authenticityUnverified),
          reason: 'altering ${entry.key} must invalidate the signature',
        );
      }
    });

    test('a malformed signature is refused rather than throwing', () async {
      // These bytes are attacker-controlled: a relay puts whatever it likes here. Every shape
      // must produce a refusal, and none may throw.
      final valid = await envelope();

      for (final signature in <List<int>>[
        <int>[],
        <int>[0],
        List<int>.filled(signatureBytes - 1, 0),
        List<int>.filled(signatureBytes, 0),
        List<int>.filled(signatureBytes + 1, 0),
        List<int>.filled(200, 0xff),
      ]) {
        expect(
          await evaluateCommand(
            envelope: CommandEnvelope(
              sender: valid.sender,
              command: valid.command,
              createdAt: valid.createdAt,
              nonce: valid.nonce,
              signature: signature,
            ),
            targetState: permissiveTarget(),
            now: created,
          ),
          const RejectedCommand(RejectionReason.authenticityUnverified),
          reason: 'a ${signature.length}-byte signature must be refused',
        );
      }
    });
  });

  group('the no-probing requirement', () {
    test('an unverified signature reports no permission reason', () async {
      // The spec's no-probing requirement, and the reason authenticity is checked first: a
      // caller must not be able to learn the target's permission state by sending
      // unauthenticated commands. Here every permission check would also fail, and none of
      // that may surface.
      final decision = await evaluateCommand(
        envelope: await unsignedEnvelope(),
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

    test('an unverified signature hides replay and freshness state too', () async {
      // Every reason after the first leaks something about the target. A caller must not be
      // able to discover which nonces this target has seen, either.
      expect(
        await evaluateCommand(
          envelope: await unsignedEnvelope(nonce: 'n-seen'),
          targetState: permissiveTarget(seenNonces: {'n-seen'}),
          now: created,
        ),
        const RejectedCommand(RejectionReason.authenticityUnverified),
      );
    });
  });

  group('the precedence order', () {
    test('replay is reported before either time reason', () async {
      // A replayed command is evidence of an attack; a stale one is more often a bad
      // network. Reporting the more serious finding when both hold means the record shows an
      // attack as an attack.
      final target = permissiveTarget(seenNonces: {'n-fresh'});

      expect(
        await evaluateCommand(
          envelope: await envelope(),
          targetState: target,
          now: at(freshnessWindowSeconds + 60),
        ),
        const RejectedCommand(RejectionReason.replayedNonce),
      );

      expect(
        await evaluateCommand(
          envelope: await envelope(),
          targetState: target,
          now: at(-(futureToleranceSeconds + 60)),
        ),
        const RejectedCommand(RejectionReason.replayedNonce),
      );
    });

    test('future dating and staleness are each reported before permission', () async {
      // notPermitted is last because it leaks the most — whether remote control is enabled
      // and what the platform can do — and is reached only by a command already authentic,
      // fresh, and new.
      final forbidden = permissiveTarget(remoteControlEnabled: false);

      expect(
        await evaluateCommand(
          envelope: await envelope(),
          targetState: forbidden,
          now: at(-(futureToleranceSeconds + 1)),
        ),
        const RejectedCommand(RejectionReason.futureDated),
      );

      expect(
        await evaluateCommand(
          envelope: await envelope(),
          targetState: forbidden,
          now: at(freshnessWindowSeconds + 1),
        ),
        const RejectedCommand(RejectionReason.stale),
      );
    });

    test('every reason holding at once still reports authenticity', () async {
      expect(
        await evaluateCommand(
          envelope: await unsignedEnvelope(
            command: RemoteCommand.enableRemoteControl,
            nonce: 'n-seen',
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
    test('a command at exactly the freshness window is accepted', () async {
      // Inclusive-accept, and expressed against the constant rather than a literal so
      // changing the constant moves the test with it.
      expect(
        await evaluateCommand(
          envelope: await envelope(),
          targetState: permissiveTarget(),
          now: at(freshnessWindowSeconds),
        ),
        const AcceptedCommand(),
      );
    });

    test('a command one second past the freshness window is stale', () async {
      expect(
        await evaluateCommand(
          envelope: await envelope(),
          targetState: permissiveTarget(),
          now: at(freshnessWindowSeconds + 1),
        ),
        const RejectedCommand(RejectionReason.stale),
      );
    });

    test('a command at exactly the future tolerance is accepted', () async {
      // Ordinary clock skew between two honest devices must not refuse a valid command.
      expect(
        await evaluateCommand(
          envelope: await envelope(),
          targetState: permissiveTarget(),
          now: at(-futureToleranceSeconds),
        ),
        const AcceptedCommand(),
      );
    });

    test('one second past the future tolerance is futureDated, not stale', () async {
      // The two indicate different faults, and a user shown "expired" for a clock-skew
      // problem will spend the evening looking in the wrong place.
      expect(
        await evaluateCommand(
          envelope: await envelope(),
          targetState: permissiveTarget(),
          now: at(-(futureToleranceSeconds + 1)),
        ),
        const RejectedCommand(RejectionReason.futureDated),
      );
    });
  });

  group('replay', () {
    test('a seen nonce is refused and a distinct one is not', () async {
      // Without the second half, an implementation could refuse every command from a device
      // that had ever sent one and still pass the replay case. Each envelope is signed over
      // its own nonce, or both would fail on authenticity instead.
      final target = permissiveTarget(seenNonces: {'n-one', 'n-two'});

      expect(
        await evaluateCommand(
          envelope: await envelope(nonce: 'n-two'),
          targetState: target,
          now: created,
        ),
        const RejectedCommand(RejectionReason.replayedNonce),
      );

      expect(
        await evaluateCommand(
          envelope: await envelope(nonce: 'n-three'),
          targetState: target,
          now: created,
        ),
        const AcceptedCommand(),
      );
    });
  });

  group('permission is delegated, not reimplemented', () {
    test('every remotely permitted command is accepted when nothing is wrong', () async {
      for (final command in [
        RemoteCommand.powerOff,
        RemoteCommand.keepAwake,
        RemoteCommand.cancelJob,
      ]) {
        expect(
          await evaluateCommand(
            envelope: await envelope(command: command),
            targetState: permissiveTarget(),
            now: created,
          ),
          const AcceptedCommand(),
          reason: '$command should be accepted',
        );
      }
    });

    test('the specific permission reason is not disclosed', () async {
      // Acceptance reports only that the command was not permitted. The authorization rule
      // keeps its own reason set for the question it answers.
      for (final target in [
        permissiveTarget(remoteControlEnabled: false),
        permissiveTarget(isPaired: false),
        permissiveTarget(targetCanPowerOff: false),
      ]) {
        expect(
          await evaluateCommand(
            envelope: await envelope(),
            targetState: target,
            now: created,
          ),
          const RejectedCommand(RejectionReason.notPermitted),
        );
      }

      expect(
        await evaluateCommand(
          envelope: await envelope(command: RemoteCommand.enableRemoteControl),
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

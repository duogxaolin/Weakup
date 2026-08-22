// The Dart mirror of `desktop/src-tauri/tests/end_to_end_authenticity.rs`.
//
// The archived design (D2) recorded that the acceptance rule took `signatureVerified` as a
// caller-supplied boolean, and stated the condition for closing that gap: the change that
// introduces a transport must verify signatures against a key held only by the paired devices,
// and must carry an end-to-end test that an invalid signature is rejected — in each language.
// This file is the Dart half.
//
// # Why these run through the transport
//
// Every case sends through `FakeTransport` and judges what comes out the other side, rather than
// calling the verifier directly. Calling the verifier proves the verifier works; it does not
// prove the path from an arriving envelope to a decision actually consults it. The hole being
// closed was a *composition* failure — a rule that could be handed the answer — so a test that
// skips the composition would not have caught it.

import 'package:flutter_test/flutter_test.dart';
import 'package:weakup/application/transport.dart';
import 'package:weakup/domain/domain.dart';

import '../domain/test_signing.dart';

final DateTime created = DateTime.utc(2026, 8, 6, 12);

late TestKeyPair genuineKey;
late TestKeyPair attackerKey;

DeviceId phone() => DeviceId('phone-a');
DeviceId desktop() => DeviceId('desktop-a');

DateTime at(int offsetSeconds) => created.add(Duration(seconds: offsetSeconds));

/// An envelope signed by [key], over exactly the fields it carries.
Future<CommandEnvelope> signed({
  required TestKeyPair key,
  RemoteCommand command = RemoteCommand.powerOff,
  String nonce = 'n-1',
  DateTime? createdAt,
}) async {
  final sender = phone();
  final when = createdAt ?? created;

  return CommandEnvelope(
    sender: sender,
    command: command,
    createdAt: when,
    nonce: nonce,
    signature: await key.signCommand(
      sender: sender,
      command: command.wireName,
      createdAt: when,
      nonce: nonce,
    ),
  );
}

/// A target that permits everything and holds [key] for the phone.
CommandTargetState targetHolding(
  VerifyingKey key, {
  Set<String>? seenNonces,
  Map<DeviceId, VerifyingKey>? verifyingKeys,
}) =>
    CommandTargetState(
      seenNonces: seenNonces ?? <String>{},
      verifyingKeys: verifyingKeys ?? {phone(): key},
      targetCanPowerOff: true,
      targetIsRemoteTarget: true,
      isPaired: true,
      remoteControlEnabled: true,
    );

/// Sends an envelope and judges whatever the transport delivers.
///
/// The full path: sign, hand to the transport, take what comes out, evaluate.
Future<List<CommandAcceptance>> roundTrip(
  FakeTransport transport,
  CommandEnvelope envelope,
  CommandTargetState targetState,
  DateTime now,
) async {
  await transport.sendEnvelope(desktop(), envelope);
  final delivered = await transport.receiveEnvelopes(desktop());

  final decisions = <CommandAcceptance>[];
  for (final one in delivered) {
    decisions.add(await evaluateCommand(
      envelope: one,
      targetState: targetState,
      now: now,
    ));
  }
  return decisions;
}

void main() {
  setUpAll(() async {
    genuineKey = await TestKeyPair.fromSeed(1);
    attackerKey = await TestKeyPair.fromSeed(9);
  });

  group('the blocker: a wrong key travelling the full path', () {
    test('a command signed with one key is refused by a target holding another', () async {
      // Key A signs; the target holds key B for that sender. Everything else about this
      // command is in order — fresh, unseen, permitted — so authenticity is the only thing
      // that can refuse it. Before this change the sender could have set a boolean instead.
      final transport = FakeTransport();

      expect(
        await roundTrip(
          transport,
          await signed(key: attackerKey),
          targetHolding(genuineKey.verifyingKey),
          created,
        ),
        [const RejectedCommand(RejectionReason.authenticityUnverified)],
      );
    });

    test('a command signed with the key the target holds is accepted', () async {
      // The positive counterpart. Without it, an implementation that refused everything
      // would pass the test above — and a system that never obeys a command is not the goal.
      final transport = FakeTransport();

      expect(
        await roundTrip(
          transport,
          await signed(key: genuineKey),
          targetHolding(genuineKey.verifyingKey),
          created,
        ),
        [const AcceptedCommand()],
      );
    });

    test('a relay that swaps the signature cannot make a command obeyed', () async {
      // The property the whole architecture rests on: the relay's cooperation is not
      // sufficient. Here the relay substitutes a signature of its own making, which is the
      // most it can do, and the target refuses.
      final transport = FakeTransport();
      final hostileRelay = await TestKeyPair.fromSeed(7);
      final original = await signed(key: genuineKey);

      final tampered = CommandEnvelope(
        sender: original.sender,
        command: original.command,
        createdAt: original.createdAt,
        nonce: original.nonce,
        signature: await hostileRelay.signCommand(
          sender: original.sender,
          command: original.command.wireName,
          createdAt: original.createdAt,
          nonce: original.nonce,
        ),
      );

      expect(
        await roundTrip(
          transport,
          tampered,
          targetHolding(genuineKey.verifyingKey),
          created,
        ),
        [const RejectedCommand(RejectionReason.authenticityUnverified)],
      );
    });
  });

  group('the four faults a transport is assumed capable of', () {
    test('a duplicated command is obeyed once', () async {
      // A retry after an ambiguous timeout. The transport delivers the same signed bytes
      // twice; the signature verifies both times, because it is genuinely the same command.
      // What stops the second is the nonce — exercised here through the full path rather
      // than by calling the replay check directly.
      final transport =
          FakeTransport(faults: const TransportFaults(duplicateDelivery: true));
      final envelope = await signed(key: genuineKey, nonce: 'n-dup');

      await transport.sendEnvelope(desktop(), envelope);
      final delivered = await transport.receiveEnvelopes(desktop());
      expect(delivered, hasLength(2), reason: 'the fake must actually duplicate');

      // The target remembers each nonce it acts on, which is what makes the second refusal
      // possible. Recording it here rather than inside the rule keeps the rule pure.
      final seen = <String>{};
      final decisions = <CommandAcceptance>[];
      for (final one in delivered) {
        final decision = await evaluateCommand(
          envelope: one,
          targetState: targetHolding(genuineKey.verifyingKey, seenNonces: seen),
          now: created,
        );
        if (decision.isAccepted) {
          seen.add(one.nonce);
        }
        decisions.add(decision);
      }

      expect(decisions, [
        const AcceptedCommand(),
        const RejectedCommand(RejectionReason.replayedNonce),
      ]);
    });

    test('a command delayed past the freshness window is refused as stale', () async {
      // A backgrounded app, or a relay holding a message. The command's creation instant
      // does not advance while it waits, which is exactly why the freshness rule catches it.
      final transport =
          FakeTransport(faults: const TransportFaults(delayDelivery: true));
      final envelope = await signed(key: genuineKey, nonce: 'n-slow');

      await transport.sendEnvelope(desktop(), envelope);
      expect(
        await transport.receiveEnvelopes(desktop()),
        isEmpty,
        reason: 'a delayed envelope must not arrive yet',
      );

      transport.releaseDelayed();
      final late = at(freshnessWindowSeconds + 1);
      final delivered = await transport.receiveEnvelopes(desktop());

      expect(delivered, hasLength(1));
      expect(
        await evaluateCommand(
          envelope: delivered.first,
          targetState: targetHolding(genuineKey.verifyingKey),
          now: late,
        ),
        const RejectedCommand(RejectionReason.stale),
      );
    });

    test('two reordered commands are each judged on their own merits', () async {
      // Two commands racing through different paths. Arriving out of order is not itself a
      // fault: each carries its own signature and its own nonce, so each stands alone.
      final transport =
          FakeTransport(faults: const TransportFaults(reorderDelivery: true));

      await transport.sendEnvelope(
        desktop(),
        await signed(key: genuineKey, command: RemoteCommand.keepAwake, nonce: 'n-first'),
      );
      await transport.sendEnvelope(
        desktop(),
        await signed(key: genuineKey, command: RemoteCommand.cancelJob, nonce: 'n-second'),
      );

      final delivered = await transport.receiveEnvelopes(desktop());
      expect(
        delivered.map((e) => e.nonce).toList(),
        ['n-second', 'n-first'],
        reason: 'the fake must actually reorder',
      );

      for (final one in delivered) {
        expect(
          await evaluateCommand(
            envelope: one,
            targetState: targetHolding(genuineKey.verifyingKey),
            now: created,
          ),
          const AcceptedCommand(),
          reason: '${one.nonce} should be judged on its own merits',
        );
      }
    });

    test('a dropped command leaves the target untouched', () async {
      // An ordinary mobile network. The sender is told nothing is wrong — it cannot tell —
      // and the target simply never hears.
      final transport =
          FakeTransport(faults: const TransportFaults(dropEverything: true));

      await transport.sendEnvelope(
        desktop(),
        await signed(key: genuineKey, nonce: 'n-lost'),
      );

      expect(await transport.receiveEnvelopes(desktop()), isEmpty);
      expect(transport.queuedCount(desktop()), 0);
    });
  });

  group('tampering, field by field', () {
    test('altering any one signed field in transit invalidates the command', () async {
      // A relay's remaining powers, tried one at a time. Each of the four signed fields is
      // something a relay would gain from changing: the command to escalate a keep-awake
      // into a shutdown, the creation instant to revive a stale command, the nonce to replay
      // one, the sender to attribute it elsewhere. All four must fail.
      final transport = FakeTransport();
      final pristine =
          await signed(key: genuineKey, command: RemoteCommand.keepAwake, nonce: 'n-tamper');

      // Both devices map to the same key, so an altered *sender* fails on the signature
      // rather than on a missing entry.
      final bothKeys = targetHolding(
        genuineKey.verifyingKey,
        verifyingKeys: {
          phone(): genuineKey.verifyingKey,
          DeviceId('phone-b'): genuineKey.verifyingKey,
        },
      );

      expect(
        await roundTrip(
          transport,
          pristine,
          targetHolding(genuineKey.verifyingKey),
          created,
        ),
        [const AcceptedCommand()],
        reason: 'the baseline must be accepted or the mutations below prove nothing',
      );

      final mutations = <String, (CommandEnvelope, CommandTargetState, DateTime)>{
        'sender': (
          CommandEnvelope(
            sender: DeviceId('phone-b'),
            command: pristine.command,
            createdAt: pristine.createdAt,
            nonce: pristine.nonce,
            signature: pristine.signature,
          ),
          bothKeys,
          created,
        ),
        'command': (
          CommandEnvelope(
            sender: pristine.sender,
            command: RemoteCommand.powerOff,
            createdAt: pristine.createdAt,
            nonce: pristine.nonce,
            signature: pristine.signature,
          ),
          targetHolding(genuineKey.verifyingKey),
          created,
        ),
        'createdAt': (
          CommandEnvelope(
            sender: pristine.sender,
            command: pristine.command,
            createdAt: at(1),
            nonce: pristine.nonce,
            signature: pristine.signature,
          ),
          targetHolding(genuineKey.verifyingKey),
          // Judged at the altered instant, so only the signature can refuse it.
          at(1),
        ),
        'nonce': (
          CommandEnvelope(
            sender: pristine.sender,
            command: pristine.command,
            createdAt: pristine.createdAt,
            nonce: 'n-rewritten',
            signature: pristine.signature,
          ),
          targetHolding(genuineKey.verifyingKey),
          created,
        ),
      };

      for (final entry in mutations.entries) {
        final (tampered, target, now) = entry.value;
        expect(
          await roundTrip(transport, tampered, target, now),
          [const RejectedCommand(RejectionReason.authenticityUnverified)],
          reason: 'altering ${entry.key} in transit must invalidate the signature',
        );
      }
    });
  });
}

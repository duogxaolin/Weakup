// What a compromised relay can and cannot do, and what happens when push fails entirely.
//
// # Why these tests route through `evaluateCommand`
//
// The spec's claim is about the *outcome at the target*, not about the plumbing. A test
// asserting that the transport returned some particular value proves nothing about whether the
// machine shuts down — the transport is not what decides that, and a transport that returned
// nothing at all would pass such a test while proving no property whatsoever.
//
// So the hostile cases below take what the relay returned, hand it to the same rule the real
// receive path uses, and assert on the refusal.
//
// # Why the relay is stubbed rather than mocked at the network
//
// These tests need no network and must not have one. `RelayDocuments` is the seam: a stub
// implementing it returns exactly the documents a compromised relay could return, and the
// parsing and the decision then run for real. Nothing here reaches Firestore, and nothing here
// would pass if the signature check were removed.

import 'dart:async';
import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:weakup/core/app_error.dart';
import 'package:weakup/domain/domain.dart';
import 'package:weakup/platform/firebase_transport.dart';

import '../domain/test_signing.dart';

// ---------------------------------------------------------------------------
// A relay that returns whatever it is told to.
// ---------------------------------------------------------------------------

/// A document store that answers with canned content and records what it was asked.
///
/// This is the hostile relay. It is not a mock of Firestore — it is a stand-in for an operator
/// who has been compromised and will return anything at all.
final class StubRelay implements RelayDocuments {
  StubRelay({List<RelayDocument>? commands, List<RelayDocument>? devices})
      : _commands = commands ?? [],
        _devices = devices ?? [];

  final List<RelayDocument> _commands;
  final List<RelayDocument> _devices;

  /// Everything written, so a test can assert on what left the device.
  final List<({String collection, String id, Map<String, Object?> fields})> writes = [];

  final StreamController<List<RelayDocument>> _watch =
      StreamController<List<RelayDocument>>.broadcast();

  /// Pushes a document change to any active listener, as an arriving command would.
  void deliver(List<RelayDocument> documents) => _watch.add(documents);

  @override
  Future<void> write(String collection, String id, Map<String, Object?> fields) async {
    writes.add((collection: collection, id: id, fields: fields));
  }

  @override
  Future<void> merge(String collection, String id, Map<String, Object?> fields) async {
    writes.add((collection: collection, id: id, fields: fields));
  }

  @override
  Future<void> add(String collection, Map<String, Object?> fields) async {
    writes.add((collection: collection, id: '', fields: fields));
  }

  @override
  Future<List<RelayDocument>> readAll(String collection) async =>
      collection == devicesCollection ? _devices : _commands;

  @override
  Future<List<RelayDocument>> readWhere(
    String collection,
    String field,
    Object? value,
  ) async =>
      collection == devicesCollection ? _devices : _commands;

  @override
  Stream<List<RelayDocument>> watchWhere(String collection, String field, Object? value) =>
      _watch.stream;
}

/// A wake-up signal a test can fire by hand.
final class ControllableWakeUp implements WakeUpSignal {
  final StreamController<void> _controller = StreamController<void>.broadcast();

  void fire() => _controller.add(null);

  @override
  Stream<void> get signals => _controller.stream;
}

// ---------------------------------------------------------------------------
// Fixtures.
// ---------------------------------------------------------------------------

DeviceId desktop() => DeviceId('desktop-1');
DeviceId phone() => DeviceId('phone-a');

DateTime at(int second) => DateTime.utc(2026, 8, 6, 12, 0, second);

/// A genuine envelope from the phone, signed with the key the desktop holds.
Future<CommandEnvelope> genuineEnvelope(TestKeyPair keys) async {
  final createdAt = at(0);
  const nonce = 'nonce-1';
  final signature = await keys.signCommand(
    sender: phone(),
    command: RemoteCommand.powerOff.wireName,
    createdAt: createdAt,
    nonce: nonce,
  );

  return CommandEnvelope(
    sender: phone(),
    command: RemoteCommand.powerOff,
    createdAt: createdAt,
    nonce: nonce,
    signature: signature,
  );
}

/// A target paired with the phone, holding its key, permitting remote power-off.
///
/// Everything is set to the permissive value deliberately: a refusal below must come from the
/// signature failing, not from some other check happening to be switched off.
CommandTargetState targetHolding(TestKeyPair key) => CommandTargetState(
      seenNonces: const {},
      verifyingKeys: {phone(): key.verifyingKey},
      targetCanPowerOff: true,
      targetIsRemoteTarget: true,
      isPaired: true,
      remoteControlEnabled: true,
    );

RelayDocument commandDocument(
  CommandEnvelope envelope, {
  String id = 'c1',
  Map<String, Object?> Function(Map<String, Object?>)? tamper,
}) {
  var fields = envelopeToDocument(desktop(), envelope);
  if (tamper != null) {
    fields = tamper(Map<String, Object?>.from(fields));
  }
  return RelayDocument(id: id, fields: fields);
}

void main() {
  // ---------------------------------------------------------------------------
  // The three hostile relays.
  // ---------------------------------------------------------------------------

  group('a compromised relay changes no outcome', () {
    test('a fabricated command is refused on authenticity grounds', () async {
      // The relay invents a power-off from the phone outright. It has no signing key — that is
      // the entire point of the design — so it puts plausible bytes in the signature field and
      // hopes.
      //
      // Asserted at the *target*, through the same rule the real receive path uses. The
      // transport delivering it is not the property under test; what matters is that the
      // machine does not shut down.
      final key = await TestKeyPair.fromSeed(1);

      final fabricated = CommandEnvelope(
        sender: phone(),
        command: RemoteCommand.powerOff,
        createdAt: at(0),
        nonce: 'relay-invented-this',
        signature: List<int>.filled(64, 0x11),
      );

      final relay = StubRelay(commands: [commandDocument(fabricated)]);
      final transport = FirebaseTransport(deviceId: desktop(), documents: relay);

      final delivered = await transport.receiveEnvelopes(desktop());
      expect(delivered, hasLength(1), reason: 'the relay did deliver something');

      final decision = await evaluateCommand(
        envelope: delivered.first,
        targetState: targetHolding(key),
        now: at(1),
      );

      expect(
        decision,
        const RejectedCommand(RejectionReason.authenticityUnverified),
        reason: 'a command the named sender did not produce must be refused on authenticity '
            'grounds, and the outcome must not depend on anything the relay did or claimed',
      );
    });

    test('an altered command is refused on authenticity grounds', () async {
      // Harder than fabrication and the more realistic attack: the relay holds a real,
      // correctly signed envelope and changes one field of the signed content. Here it moves
      // the creation instant forward to revive a command that would otherwise be stale.
      //
      // The signature covers sender, command, createdAt, and nonce, so altering any of them
      // invalidates it.
      final key = await TestKeyPair.fromSeed(1);
      final genuine = await genuineEnvelope(key);

      final relay = StubRelay(
        commands: [
          commandDocument(
            genuine,
            tamper: (fields) => {
              ...fields,
              'createdAt': at(30).toIso8601String(),
            },
          ),
        ],
      );
      final transport = FirebaseTransport(deviceId: desktop(), documents: relay);

      final delivered = await transport.receiveEnvelopes(desktop());
      expect(
        delivered.first.createdAt,
        isNot(genuine.createdAt),
        reason: 'the test must actually alter the signed content or it proves nothing',
      );

      final decision = await evaluateCommand(
        envelope: delivered.first,
        targetState: targetHolding(key),
        now: at(31),
      );

      expect(
        decision,
        const RejectedCommand(RejectionReason.authenticityUnverified),
        reason: 'altering signed content must invalidate the signature; a relay that could '
            'edit a command and have it obeyed would be able to forge one',
      );
    });

    test('the same command unaltered is accepted, so the refusals above mean something',
        () async {
      // The control. Without this, both tests above would pass against an implementation that
      // refused everything unconditionally — including every genuine command — and the suite
      // would report a working system that can never be commanded at all.
      final key = await TestKeyPair.fromSeed(1);
      final genuine = await genuineEnvelope(key);

      final relay = StubRelay(commands: [commandDocument(genuine)]);
      final transport = FirebaseTransport(deviceId: desktop(), documents: relay);

      final delivered = await transport.receiveEnvelopes(desktop());

      expect(
        delivered.first,
        genuine,
        reason: 'a round trip through the relay document shape must preserve every signed '
            'field exactly, including the signature bytes',
      );

      final decision = await evaluateCommand(
        envelope: delivered.first,
        targetState: targetHolding(key),
        now: at(1),
      );

      expect(decision, const AcceptedCommand());
    });

    test('malformed content is an error rather than a crash', () async {
      // A crash on relay-controlled input is a denial of service against the device being
      // defended: an attacker who can kill the app has stopped it responding to anything,
      // including the user's own commands.
      final relay = StubRelay(
        commands: [
          const RelayDocument(id: 'bad', fields: {'target': 'desktop-1', 'sender': 42}),
        ],
      );
      final transport = FirebaseTransport(deviceId: desktop(), documents: relay);

      await expectLater(
        transport.receiveEnvelopes(desktop()),
        throwsA(isA<StorageError>()),
        reason: 'malformed content must surface as an error the caller can report, not as a '
            'success carrying nothing',
      );
    });

    test('a document with a missing field is an error, not a default', () async {
      // Design D8's central claim, tested rather than asserted in prose. A defaulted
      // `createdAt` would make a stale command look fresh; a defaulted nonce would collide with
      // every other defaulted nonce and break replay defence.
      final key = await TestKeyPair.fromSeed(1);
      final genuine = await genuineEnvelope(key);

      for (final field in ['sender', 'command', 'createdAt', 'nonce', 'signature']) {
        final relay = StubRelay(
          commands: [
            commandDocument(
              genuine,
              tamper: (fields) => fields..remove(field),
            ),
          ],
        );
        final transport = FirebaseTransport(deviceId: desktop(), documents: relay);

        await expectLater(
          transport.receiveEnvelopes(desktop()),
          throwsA(isA<StorageError>()),
          reason: 'a document missing "$field" must be an error; a silent default is '
              'indistinguishable from a real value and design D8 forbids it',
        );
      }
    });

    test('an unrecognised command name is an error rather than a guess', () async {
      // Mapping an unknown name onto a plausible one would let a relay steer a command towards
      // a different action than the one that was signed.
      final key = await TestKeyPair.fromSeed(1);
      final genuine = await genuineEnvelope(key);

      final relay = StubRelay(
        commands: [
          commandDocument(
            genuine,
            tamper: (fields) => {...fields, 'command': 'selfDestruct'},
          ),
        ],
      );
      final transport = FirebaseTransport(deviceId: desktop(), documents: relay);

      await expectLater(
        transport.receiveEnvelopes(desktop()),
        throwsA(isA<StorageError>()),
      );
    });

    test('a signature that is not valid base64 is an error rather than a crash', () async {
      final key = await TestKeyPair.fromSeed(1);
      final genuine = await genuineEnvelope(key);

      final relay = StubRelay(
        commands: [
          commandDocument(
            genuine,
            tamper: (fields) => {...fields, 'signature': '!!!not base64!!!'},
          ),
        ],
      );
      final transport = FirebaseTransport(deviceId: desktop(), documents: relay);

      await expectLater(
        transport.receiveEnvelopes(desktop()),
        throwsA(isA<StorageError>()),
      );
    });
  });

  // ---------------------------------------------------------------------------
  // The degradation path design D4 claims.
  // ---------------------------------------------------------------------------

  group('push is a wake-up signal only', () {
    test('disabling push entirely still delivers commands through the listener', () async {
      // This is the claim D4 makes, and an untested claim is a guess. A device with no push at
      // all — a revoked token, a Play Services outage, a user who switched notifications off —
      // must still receive its commands. It loses latency, not delivery.
      //
      // `NoWakeUpSignal` is not merely a test double here: it *is* the failed-push device.
      final key = await TestKeyPair.fromSeed(1);
      final genuine = await genuineEnvelope(key);

      final relay = StubRelay();
      final transport = FirebaseTransport(
        deviceId: desktop(),
        documents: relay,
        wakeUp: const NoWakeUpSignal(),
      );

      final received = <CommandEnvelope>[];
      final subscription = transport.listen().listen(received.add);
      addTearDown(subscription.cancel);

      // Nothing will ever wake this device. The listener is the delivery path regardless.
      var pushFired = false;
      final wakeSubscription = transport.wakeUps.listen((_) => pushFired = true);
      addTearDown(wakeSubscription.cancel);

      relay.deliver([commandDocument(genuine)]);
      await pumpEventQueue();

      expect(
        received,
        hasLength(1),
        reason: 'a device that never receives a push must still receive its commands; push '
            'failing degrades latency, never delivery',
      );
      expect(received.first, genuine);
      expect(pushFired, isFalse, reason: 'no push was delivered in this scenario at all');

      // And the command that arrived that way is still a real one at the target.
      final decision = await evaluateCommand(
        envelope: received.first,
        targetState: targetHolding(key),
        now: at(1),
      );
      expect(decision, const AcceptedCommand());
    });

    test('a push carries no command content', () async {
      // Design D4. The signal reports only *that* something arrived — its payload type is
      // `void`, so a command cannot travel in it. A push carrying content would place a command
      // in a channel neither device controls, and a delivered-but-unread push would become a
      // command the target believes it never received.
      final wakeUp = ControllableWakeUp();
      final transport = FirebaseTransport(
        deviceId: desktop(),
        documents: StubRelay(),
        wakeUp: wakeUp,
      );

      final signals = <void>[];
      final subscription = transport.wakeUps.listen(signals.add);
      addTearDown(subscription.cancel);

      wakeUp.fire();
      await pumpEventQueue();

      expect(signals, hasLength(1));
      expect(
        transport.wakeUps,
        isA<Stream<void>>(),
        reason: 'the wake-up stream must carry no payload, so a command cannot travel in it',
      );
    });

    test('a push does not deliver a command by itself', () async {
      // The push is a signal to look, not the thing that was found. Firing one with nothing in
      // the relay must produce no command — otherwise the push would be a second delivery path
      // and the two could disagree.
      final wakeUp = ControllableWakeUp();
      final relay = StubRelay();
      final transport = FirebaseTransport(
        deviceId: desktop(),
        documents: relay,
        wakeUp: wakeUp,
      );

      final received = <CommandEnvelope>[];
      final subscription = transport.listen().listen(received.add);
      addTearDown(subscription.cancel);

      wakeUp.fire();
      await pumpEventQueue();

      expect(received, isEmpty);
    });
  });

  // ---------------------------------------------------------------------------
  // Presence is data, reported only for oneself.
  // ---------------------------------------------------------------------------

  group('liveness is data, not a verdict', () {
    test('a device reports only its own presence', () async {
      // Enforced locally rather than left to the relay's rules, so it holds even against a
      // relay whose rules failed entirely.
      final relay = StubRelay();
      final transport = FirebaseTransport(deviceId: desktop(), documents: relay);

      await expectLater(
        transport.reportPresence(phone(), at(0)),
        throwsA(isA<ArgumentError>()),
        reason: 'a transport that could report presence for a peer could make an absent device '
            'look present, and a user would send a command to a machine that is not there',
      );
      expect(
        relay.writes,
        isEmpty,
        reason: 'the refusal must happen before anything is written, not after',
      );
    });

    test('reporting presence writes an instant and not a state', () async {
      final relay = StubRelay();
      final transport = FirebaseTransport(deviceId: desktop(), documents: relay);

      await transport.reportPresence(desktop(), at(0));

      expect(relay.writes, hasLength(1));
      final fields = relay.writes.single.fields;
      expect(fields.containsKey('lastReportedAt'), isTrue);
      expect(
        fields.keys.any((key) => key.toLowerCase().contains('online')),
        isFalse,
        reason: 'the transport must report an instant rather than a verdict',
      );
    });

    test('listing devices distinguishes never-reported from long-ago', () async {
      // `null` rather than a distant past instant: "never spoke" and "spoke long ago" are
      // different facts, and flattening them would make a brand-new device look stale.
      final relay = StubRelay(devices: [
        RelayDocument(id: 'desktop-1', fields: {
          'deviceId': 'desktop-1',
          'displayName': 'Studio Desktop',
          'lastReportedAt': at(0).toIso8601String(),
        }),
        const RelayDocument(id: 'phone-a', fields: {
          'deviceId': 'phone-a',
          'displayName': 'Phone A',
        }),
      ]);
      final transport = FirebaseTransport(deviceId: desktop(), documents: relay);

      final devices = await transport.listDevices();

      expect(devices, hasLength(2));
      expect(devices[0].lastReportedAt, at(0));
      expect(
        devices[1].lastReportedAt,
        isNull,
        reason: 'a device that has never reported must carry no instant at all',
      );
    });

    test('a device document with a missing field is an error, not a default', () async {
      final relay = StubRelay(devices: [
        const RelayDocument(id: 'x', fields: {'deviceId': 'desktop-1'}),
      ]);
      final transport = FirebaseTransport(deviceId: desktop(), documents: relay);

      await expectLater(transport.listDevices(), throwsA(isA<StorageError>()));
    });
  });

  // ---------------------------------------------------------------------------
  // Sending, and delivery semantics.
  // ---------------------------------------------------------------------------

  group('sending and delivery', () {
    test('sending carries the signature bytes unchanged', () async {
      // The relay stores opaque content. A transport that re-encoded, truncated, or normalised
      // the signature would break the check for a genuine command and would be doing something
      // to bytes it has no business interpreting.
      final key = await TestKeyPair.fromSeed(1);
      final genuine = await genuineEnvelope(key);

      final relay = StubRelay();
      final transport = FirebaseTransport(deviceId: phone(), documents: relay);

      await transport.sendEnvelope(desktop(), genuine);

      final fields = relay.writes.single.fields;
      expect(base64Decode(fields['signature']! as String), genuine.signature);
      expect(documentToEnvelope(fields), genuine);
    });

    test('the same document is not delivered twice in a session', () async {
      // Not replay defence — the nonce store in the acceptance rule is that, and it survives a
      // restart. This only avoids pointless duplicate work within one session.
      final key = await TestKeyPair.fromSeed(1);
      final genuine = await genuineEnvelope(key);

      final relay = StubRelay(commands: [commandDocument(genuine, id: 'dup')]);
      final transport = FirebaseTransport(deviceId: desktop(), documents: relay);

      expect(await transport.receiveEnvelopes(desktop()), hasLength(1));
      expect(await transport.receiveEnvelopes(desktop()), isEmpty);
    });

    test('an empty account lists no devices rather than failing', () async {
      final transport = FirebaseTransport(deviceId: desktop(), documents: StubRelay());

      expect(await transport.listDevices(), isEmpty);
    });
  });
}

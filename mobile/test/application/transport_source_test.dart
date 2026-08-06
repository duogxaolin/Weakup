// The structural guarantee for the transport seam, plus the transport's own behaviour.
//
// # Why this file is separate from the implementation
//
// The first test greps `lib/application/transport.dart` for name fragments that must never
// appear in it. Written inside that file, the test's own string literals would be part of the
// source it searches, and it would fail against itself.
//
// # Why source text rather than review
//
// Because the failure is additive. Someone adds a helpful-looking flag to the interface, meaning
// only to plumb through something a relay already knows; every existing test still passes, and
// the guarantee is gone with nothing turning red.

import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:weakup/application/transport.dart';
import 'package:weakup/domain/domain.dart';

/// Resolved relative to the package root, which is the working directory for `flutter test`.
final File transportSource = File('lib/application/transport.dart');

DeviceId phone() => DeviceId('phone-a');

void main() {
  group('the structural guarantee', () {
    test('the transport interface offers no way to claim a command is genuine', () {
      // A transport that could vouch for a command would make compromise of the relay
      // equivalent to control of every user's machine. The interface therefore has no member
      // an implementation could use to say so — and because such a member would be *added*
      // rather than break anything existing, its absence is asserted rather than assumed.
      //
      // Deliberately checked over the whole file, comments included. A doc comment describing
      // a field that does not exist is the first step towards the field existing.
      expect(
        transportSource.existsSync(),
        isTrue,
        reason: 'the transport source must be readable or this test proves nothing',
      );

      final lines = transportSource.readAsLinesSync();

      for (final forbidden in ['verified', 'trusted', 'attested', 'authentic']) {
        final uses =
            lines.where((line) => line.toLowerCase().contains(forbidden)).toList();

        expect(
          uses,
          isEmpty,
          reason: 'the transport library contains "$forbidden", which suggests a member by '
              'which a relay could assert that a command is genuine. A transport moves '
              'bytes and makes no such claim; the target checks the signature itself: $uses',
        );
      }
    });
  });

  group('liveness is data, not a verdict', () {
    test('the transport reports an instant rather than an online state', () async {
      // Two devices asking the same relay about the same peer must not be able to receive
      // different answers, so the relay reports when a device last spoke and the presence
      // rule decides what that means, at the asking device.
      final transport = FakeTransport();
      final at = DateTime.utc(2026, 8, 6, 12);

      await transport.registerDevice(phone(), 'Phone A');
      await transport.reportPresence(phone(), at);

      final devices = await transport.listDevices();
      expect(devices, hasLength(1));
      expect(devices.first.deviceId, phone());
      expect(devices.first.lastReportedAt, at);
    });

    test('a device that has never reported presence has no instant', () async {
      // `null` rather than a distant past instant: "never spoke" and "spoke long ago" are
      // different facts, and flattening them would make a brand-new device look stale.
      final transport = FakeTransport();

      await transport.registerDevice(phone(), 'Phone A');

      final devices = await transport.listDevices();
      expect(devices.first.lastReportedAt, isNull);
    });

    test('registering the same device twice updates it rather than duplicating it', () async {
      final transport = FakeTransport();

      await transport.registerDevice(phone(), 'Old name');
      await transport.registerDevice(phone(), 'New name');

      final devices = await transport.listDevices();
      expect(devices, hasLength(1));
      expect(devices.first.displayName, 'New name');
    });
  });

  group('the fake behaves by default', () {
    test('nothing is queued before anything is sent', () {
      // A test that does not opt into a fault should read plainly, so the default must be
      // boring.
      expect(FakeTransport().queuedCount(phone()), 0);
    });

    test('faults are opt-in and independent', () {
      const faults = TransportFaults(duplicateDelivery: true);

      expect(faults.duplicateDelivery, isTrue);
      expect(faults.dropEverything, isFalse);
      expect(faults.delayDelivery, isFalse);
      expect(faults.reorderDelivery, isFalse);
    });
  });

  group('account identity is a separate concern', () {
    test('the auth provider reports an account and nothing about permission', () async {
      // The whole surface of the provider. It answers whose devices to show, and says
      // nothing about what may be commanded — pairing decides that, and the authorization
      // rules already state that same-account access is not sufficient.
      const provider = FakeAuthProvider(AccountId('account-1'));

      expect(await provider.currentAccount(), const AccountId('account-1'));
    });

    test('a signed-out device reports no account', () async {
      const provider = FakeAuthProvider.signedOut();

      expect(await provider.currentAccount(), isNull);
    });

    test('the two interfaces are separately implementable', () {
      // Stated as a compiling fact rather than as prose: each is satisfied by a type that
      // does not implement the other, so replacing one cannot require touching the other.
      void takesTransport(RemoteTransport _) {}
      void takesAuth(AuthProvider _) {}

      takesTransport(FakeTransport());
      takesAuth(const FakeAuthProvider.signedOut());
    });
  });
}

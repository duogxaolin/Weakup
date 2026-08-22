// Widget tests for the pairing and paired-devices screens.
//
// These prove the controls exist, are wired to the callbacks they claim, and render the derived
// presence. They do **not** prove a pairing works between two real devices, and nothing here
// touches a relay — see the completion report for what remains unverified.

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:weakup/application/pairing_flow.dart';
import 'package:weakup/domain/device_id.dart';
import 'package:weakup/domain/pairing_grant.dart';
import 'package:weakup/domain/presence.dart';
import 'package:weakup/domain/remote_command.dart';
import 'package:weakup/ui/screens/paired_devices_screen.dart';
import 'package:weakup/ui/screens/pairing_screen.dart';
import 'package:weakup/ui/screens/presence_presentation.dart';

void main() {
  final now = DateTime.utc(2026, 8, 6, 12, 0, 0);

  PairedPeer peer({
    String id = 'desktop-1',
    int? silentForSeconds = 10,
    bool revoked = false,
  }) =>
      PairedPeer(
        deviceId: id,
        displayName: 'Study laptop',
        lastReportedAt: silentForSeconds == null
            ? null
            : now.subtract(Duration(seconds: silentForSeconds)),
        revoked: revoked,
      );

  Widget wrap(Widget child) => MaterialApp(home: child);

  group('PairingScreen', () {
    testWidgets('hands the typed code to the pairing flow verbatim', (tester) async {
      // Verbatim matters: normalisation and the alphabet check live in `PairingCode.parse`, and
      // a screen that trimmed or upper-cased first would be a second definition of a code.
      String? seenCode;

      await tester.pumpWidget(wrap(PairingScreen(
        onSubmit: ({
          required String code,
          required String peerDeviceId,
          required String peerVerifyingKey,
        }) async {
          seenCode = code;
          return PairedOutcome(DeviceId(peerDeviceId));
        },
      )));

      await tester.enterText(find.byKey(const Key('pairing-code-field')), 'ab cd-ef');
      await tester.enterText(find.byKey(const Key('pairing-peer-id-field')), 'desktop-1');
      await tester.enterText(find.byKey(const Key('pairing-peer-key-field')), 'd75a');
      await tester.tap(find.byKey(const Key('pairing-submit-button')));
      await tester.pumpAndSettle();

      expect(seenCode, 'ab cd-ef', reason: 'the screen normalised the code itself');
    });

    testWidgets('shows the domain\'s own message on a refusal', (tester) async {
      // Not prose invented here. A user who sees a refusal on their phone and again on their
      // laptop must not be told two different things.
      await tester.pumpWidget(wrap(PairingScreen(
        onSubmit: ({
          required String code,
          required String peerDeviceId,
          required String peerVerifyingKey,
        }) async =>
            const RefusedOutcome(GrantRejection.expired),
      )));

      await tester.tap(find.byKey(const Key('pairing-submit-button')));
      await tester.pumpAndSettle();

      final message = tester.widget<Text>(find.byKey(const Key('pairing-message')));
      expect(message.data, GrantRejection.expired.userMessage);
    });

    testWidgets('says both devices must be in hand', (tester) async {
      // The requirement the whole design rests on. A pairing that could be established
      // remotely and unattended would return the system to trusting whoever holds the account.
      await tester.pumpWidget(wrap(PairingScreen(
        onSubmit: ({
          required String code,
          required String peerDeviceId,
          required String peerVerifyingKey,
        }) async =>
            PairedOutcome(DeviceId('x')),
      )));

      expect(find.textContaining('Both devices need to be with you'), findsOneWidget);
    });
  });

  group('PairedDevicesScreen', () {
    testWidgets('renders presence derived from the reported instant', (tester) async {
      await tester.pumpWidget(wrap(PairedDevicesScreen(
        peers: [
          peer(id: 'fresh', silentForSeconds: 5),
          peer(id: 'napping', silentForSeconds: onlineThresholdSeconds + 1),
          peer(id: 'gone', silentForSeconds: offlineThresholdSeconds + 1),
        ],
        now: now,
        onSendCommand: (_, _) async => 'sent',
        onRevoke: (_) async {},
      )));

      // Three peers, three different states — from one code path over three instants. A
      // relay-supplied flag could not produce this without the relay being asked three
      // different questions.
      expect(
        tester.widget<Text>(find.byKey(const Key('presence-fresh'))).data,
        presenceLabel(PresenceState.online),
      );
      expect(
        tester.widget<Text>(find.byKey(const Key('presence-napping'))).data,
        presenceLabel(PresenceState.stale),
      );
      expect(
        tester.widget<Text>(find.byKey(const Key('presence-gone'))).data,
        presenceLabel(PresenceState.offline),
      );
    });

    testWidgets('offers power-off and keep-awake requests to a reachable peer', (tester) async {
      final sent = <RemoteCommand>[];

      await tester.pumpWidget(wrap(PairedDevicesScreen(
        peers: [peer()],
        now: now,
        onSendCommand: (_, command) async {
          sent.add(command);
          return 'The request was sent.';
        },
        onRevoke: (_) async {},
      )));

      await tester.tap(find.byKey(const Key('request-power-off-desktop-1')));
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const Key('request-keep-awake-desktop-1')));
      await tester.pumpAndSettle();

      expect(sent, [RemoteCommand.powerOff, RemoteCommand.keepAwake]);
    });

    testWidgets('offers no control that would enable remote control on a peer', (tester) async {
      // `enableRemoteControl` is refused by `authorize` before the pairing check even runs, so a
      // button for it could only ever fail. Requiring physical presence to grant the permission
      // is what keeps account access from being enough to power off every device on it.
      final sent = <RemoteCommand>[];

      await tester.pumpWidget(wrap(PairedDevicesScreen(
        peers: [peer()],
        now: now,
        onSendCommand: (_, command) async {
          sent.add(command);
          return 'sent';
        },
        onRevoke: (_) async {},
      )));

      for (final button in find.byType(FilledButton).evaluate()) {
        await tester.tap(find.byWidget(button.widget));
        await tester.pumpAndSettle();
      }

      expect(sent, isNot(contains(RemoteCommand.enableRemoteControl)));
    });

    testWidgets('tells the user the target waits and can cancel', (tester) async {
      // Said before the button is pressed, not in a dialog afterwards. Someone requesting a
      // power-off should already know the machine warns whoever is using it.
      await tester.pumpWidget(wrap(PairedDevicesScreen(
        peers: [peer()],
        now: now,
        onSendCommand: (_, _) async => 'sent',
        onRevoke: (_) async {},
      )));

      expect(find.textContaining('waits five minutes'), findsOneWidget);
      expect(find.textContaining('cancel it'), findsOneWidget);
    });

    testWidgets('disables sending to a peer that has been silent too long', (tester) async {
      await tester.pumpWidget(wrap(PairedDevicesScreen(
        peers: [peer(silentForSeconds: offlineThresholdSeconds + 1)],
        now: now,
        onSendCommand: (_, _) async => 'sent',
        onRevoke: (_) async {},
      )));

      final button = tester.widget<FilledButton>(
        find.byKey(const Key('request-power-off-desktop-1')),
      );
      expect(button.onPressed, isNull);
      // And says why, rather than leaving a control that silently does nothing.
      expect(find.textContaining('has not reported in'), findsOneWidget);
    });

    testWidgets('lists a revoked pairing but offers no controls on it', (tester) async {
      // A row that vanished on revocation would make "was this device ever paired?"
      // unanswerable — the question that matters most after a device is lost.
      await tester.pumpWidget(wrap(PairedDevicesScreen(
        peers: [peer(revoked: true)],
        now: now,
        onSendCommand: (_, _) async => 'sent',
        onRevoke: (_) async {},
      )));

      expect(find.text('Study laptop'), findsOneWidget);
      expect(tester.widget<Text>(find.byKey(const Key('presence-desktop-1'))).data, 'Unpaired');
      expect(find.byKey(const Key('request-power-off-desktop-1')), findsNothing);
      expect(find.byKey(const Key('revoke-desktop-1')), findsNothing);
    });

    testWidgets('unpairing asks first and says it works offline', (tester) async {
      final revoked = <String>[];

      await tester.pumpWidget(wrap(PairedDevicesScreen(
        peers: [peer()],
        now: now,
        onSendCommand: (_, _) async => 'sent',
        onRevoke: (id) async => revoked.add(id),
      )));

      await tester.tap(find.byKey(const Key('revoke-desktop-1')));
      await tester.pumpAndSettle();

      // Revocation has to work with no network, because that is exactly when someone
      // unpairing a lost phone is most likely to be doing it.
      expect(find.textContaining('even with no network'), findsOneWidget);

      await tester.tap(find.text('Keep paired'));
      await tester.pumpAndSettle();
      expect(revoked, isEmpty, reason: 'declining the dialog still revoked');

      await tester.tap(find.byKey(const Key('revoke-desktop-1')));
      await tester.pumpAndSettle();
      // Scoped to the dialog: the row's own "Unpair" text button is still on screen behind it,
      // and an unscoped finder matches both.
      await tester.tap(
        find.descendant(of: find.byType(AlertDialog), matching: find.text('Unpair')),
      );
      await tester.pumpAndSettle();
      expect(revoked, ['desktop-1']);
    });

    testWidgets('an empty list says how to pair rather than looking broken', (tester) async {
      await tester.pumpWidget(wrap(PairedDevicesScreen(
        peers: const [],
        now: now,
        onSendCommand: (_, _) async => 'sent',
        onRevoke: (_) async {},
      )));

      expect(find.textContaining('No devices are paired yet'), findsOneWidget);
      expect(find.textContaining('Show a pairing code'), findsOneWidget);
    });
  });
}

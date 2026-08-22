// The structural guarantees for the Firestore transport.
//
// # Why this file is separate from the implementation
//
// The tests below grep `lib/platform/firebase_transport.dart` for name fragments that must
// never appear in it. Written inside that file, each test's own string literals would be part
// of the source it searches, and it would fail against itself.
//
// # Why source text rather than review
//
// Because the failure is additive. Someone adds a helpful-looking flag to the transport,
// meaning only to plumb through something the relay already knows; every existing test still
// passes, and the guarantee is gone with nothing turning red.

import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:weakup/domain/presence.dart';
import 'package:weakup/platform/firebase_transport.dart';

/// Resolved relative to the package root, which is the working directory for `flutter test`.
final File transportSource = File('lib/platform/firebase_transport.dart');

void main() {
  group('the structural guarantee', () {
    test('the Firestore transport contains no way to claim a command is genuine', () {
      // This is the same check the transport interface carries, applied to the first real
      // implementation of it. The interface offering no such member is only half the
      // guarantee: an implementation could still grow its own field and have a caller read
      // it. A relay that could vouch for a command would make compromise of the relay
      // equivalent to control of every user's machine.
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
        final uses = lines.where((line) => line.toLowerCase().contains(forbidden)).toList();

        expect(
          uses,
          isEmpty,
          reason: 'firebase_transport.dart contains "$forbidden", which suggests a member by '
              'which a relay could assert that a command is genuine. A transport moves bytes '
              'and makes no such claim; the target checks the signature itself: $uses',
        );
      }
    });

    test('the Firestore transport never names the power-off path', () {
      // A remote request creates a *job*, which the existing scheduler runs by its existing
      // rules — including the mandatory cancellable countdown a remote-origin job earns. The
      // transport reaching an executor directly would be a second power-off path with no
      // countdown in front of it, and the whole safety structure rests on there being exactly
      // one.
      final lines = transportSource.readAsLinesSync();

      for (final forbidden in [
        'PowerOffGate',
        'PowerOffExecutor',
        'GraceOutcome',
        'powerOffNow',
      ]) {
        final uses = lines.where((line) => line.contains(forbidden)).toList();

        expect(
          uses,
          isEmpty,
          reason: 'firebase_transport.dart names "$forbidden". A transport delivers bytes; a '
              'remote request becomes a job and the scheduler runs it, which is what keeps '
              'the countdown mandatory: $uses',
        );
      }
    });

    test('a push carries no command content', () {
      // Design D4. The wake-up signal's payload type is `void`, so a command cannot travel in
      // it even by accident — but the *stream declaration* is what enforces that, and a
      // contributor widening it to carry an envelope would be undoing the decision. Asserted
      // in source text because the change would compile perfectly well.
      final source = transportSource.readAsStringSync();

      expect(
        source.contains('Stream<void> get signals'),
        isTrue,
        reason: 'the wake-up signal must carry no payload: a push carrying command content '
            'would be a second delivery path, and a delivered-but-unread push would become a '
            'command the target believes it never received',
      );
    });
  });

  group('the presence interval', () {
    test('presence is reported every sixty seconds', () {
      // The presence rule's 90-second online threshold was chosen to tolerate one missed
      // report on a 60-second heartbeat plus jitter. Reporting on this interval is what makes
      // the threshold mean what its own documentation says.
      expect(presenceReportIntervalSeconds, 60);
      expect(FirebaseTransport.presenceInterval, const Duration(seconds: 60));
    });

    test('the interval is shorter than the online threshold', () {
      // A heartbeat at or beyond the threshold would report a healthy device as absent between
      // two consecutive successful reports, which is a false alarm rather than a detection.
      expect(presenceIntervalFitsOnlineThreshold, isTrue);
      expect(presenceReportIntervalSeconds, lessThan(onlineThresholdSeconds));
    });

    test('the interval is documented against the online threshold', () {
      // The 60-second interval and the 90-second threshold are one decision expressed in two
      // files. A contributor changing the interval without reading why would silently change
      // what "online" means for every device, so the constant's comment must name the
      // threshold it is derived from.
      expect(
        transportSource.readAsStringSync().contains('onlineThresholdSeconds'),
        isTrue,
        reason: 'the presence interval must be documented against onlineThresholdSeconds',
      );
    });

    test('the interval matches the Rust constant', () {
      // Both implementations report on the same heartbeat, or one of them makes the shared
      // presence threshold mean something different from what the other believes.
      final rust = File('../desktop/src-tauri/src/platform/firebase_transport.rs');

      expect(
        rust.existsSync(),
        isTrue,
        reason: 'the Rust mirror must be readable or this test proves nothing',
      );
      expect(
        rust.readAsStringSync().contains(
              'PRESENCE_REPORT_INTERVAL_SECONDS: u64 = $presenceReportIntervalSeconds',
            ),
        isTrue,
        reason: 'the two implementations must report presence on the same interval',
      );
    });
  });
}

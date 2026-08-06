// Unit tests for the presence rule.
//
// The shared vectors in `shared/testvectors/presence.json` prove this agrees with the
// Rust implementation. These prove the local rule, in the style of the existing domain
// tests: the boundaries from both sides, the clamp, and the never-reported case.

import 'package:flutter_test/flutter_test.dart';
import 'package:weakup/domain/presence.dart';

void main() {
  final now = DateTime.utc(2026, 7, 30, 10, 30);

  /// Evaluates a device that last reported [seconds] before a fixed now. Negative values
  /// put the last report in the future.
  PresenceEvaluation after(int seconds) => evaluatePresence(
        lastSeen: now.subtract(Duration(seconds: seconds)),
        now: now,
      );

  group('evaluatePresence', () {
    test('a device that just reported is online', () {
      expect(after(5).state, PresenceState.online);
    });

    test('exactly the online threshold is still online', () {
      // The boundary is inclusive-online by decision, not by whichever comparison
      // operator happened to be typed. Asserted from both sides so a `<` slipping in for
      // a `<=` fails here rather than only in the cross-language vectors.
      expect(after(onlineThresholdSeconds).state, PresenceState.online);
      expect(after(onlineThresholdSeconds + 1).state, PresenceState.stale);
    });

    test('exactly the offline threshold is still stale', () {
      expect(after(offlineThresholdSeconds).state, PresenceState.stale);
      expect(after(offlineThresholdSeconds + 1).state, PresenceState.offline);
    });

    test('the online window is shorter than the offline one', () {
      // Equal or inverted thresholds would erase the stale state entirely, which is the
      // one thing the three-state model exists to provide. The Rust side asserts this at
      // compile time; Dart has no equivalent, so it is a test here.
      expect(onlineThresholdSeconds, lessThan(offlineThresholdSeconds));
    });

    test('the thresholds match the desktop implementation', () {
      // Named here as well as in the vectors: a change to one language's constant should
      // fail its own suite before it fails the cross-language one.
      expect(onlineThresholdSeconds, 90);
      expect(offlineThresholdSeconds, 900);
    });

    test('a device that never reported is offline with no age', () {
      // The absent value must not be treated as the current instant, which would show a
      // device that has never been seen as online.
      final evaluation = evaluatePresence(lastSeen: null, now: now);

      expect(evaluation.state, PresenceState.offline);
      expect(evaluation.elapsed, isNull);
    });

    test('a lastSeen in the future clamps to zero rather than failing', () {
      // A peer with a skewed clock. Reporting an error here would make someone else's
      // clock look like a local failure, and a negative elapsed would print as a
      // countdown in the UI.
      final evaluation = evaluatePresence(
        lastSeen: now.add(const Duration(seconds: 5)),
        now: now,
      );

      expect(evaluation.state, PresenceState.online);
      expect(evaluation.elapsed, Duration.zero);
    });

    test('a lastSeen far in the future clamps the same way', () {
      // A month of skew, not five seconds: a clamp with a small tolerance would pass the
      // previous test and fail this one.
      final evaluation = evaluatePresence(
        lastSeen: now.add(const Duration(days: 31)),
        now: now,
      );

      expect(evaluation.state, PresenceState.online);
      expect(evaluation.elapsed, Duration.zero);
    });

    test('the elapsed time is reported for every state that has one', () {
      // Including online, so a caller does not have to branch on the state to learn how
      // long it has been.
      for (final seconds in [0, 5, onlineThresholdSeconds + 1, 86400]) {
        expect(
          after(seconds).elapsed?.inSeconds,
          seconds,
          reason: 'elapsed must survive for a device silent ${seconds}s',
        );
      }
    });

    test('elapsed time is never negative', () {
      for (final offset in [-1, -60, -86400]) {
        expect(
          after(offset).elapsed!.isNegative,
          isFalse,
          reason: 'a skewed peer clock must not produce a negative age',
        );
      }
    });

    test('the state names match the shared wire format', () {
      // These exact strings appear in the shared vectors, so they are a cross-language
      // contract rather than an internal detail.
      expect(PresenceState.online.name, 'online');
      expect(PresenceState.stale.name, 'stale');
      expect(PresenceState.offline.name, 'offline');
    });
  });

  group('PresenceEvaluation', () {
    test('two evaluations of the same state and age are equal', () {
      expect(after(30), after(30));
      expect(after(30).hashCode, after(30).hashCode);
    });

    test('the age is part of the identity, not only the state', () {
      // Both are offline. A model comparing only the state would report no change when a
      // device went from silent-for-an-hour to silent-for-a-day.
      expect(after(1000), isNot(after(86400)));
    });
  });
}

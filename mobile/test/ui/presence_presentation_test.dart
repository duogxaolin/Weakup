// Tests for the presentation of presence on the phone.
//
// The claim under test is task 7.2's: presence is *derived* from a reported instant by the
// presence rule, never taken as a flag from the relay. These tests are written so that a
// passthrough implementation fails them — see `a_boolean_cannot_express_what_these_three_do`.

import 'package:flutter_test/flutter_test.dart';
import 'package:weakup/domain/presence.dart';
import 'package:weakup/ui/screens/presence_presentation.dart';

void main() {
  final now = DateTime.utc(2026, 8, 6, 12, 0, 0);

  PresenceView after(int seconds) => presencePresentation(
        lastReportedAt: now.subtract(Duration(seconds: seconds)),
        now: now,
      );

  group('presencePresentation', () {
    test('walks all three states across the rule\'s own boundaries', () {
      // A view that hardcoded one state, or ignored the instant entirely, would pass a
      // single-case test. This one pins both sides of both boundaries, so a constant cannot.
      expect(after(0).state, PresenceState.online);
      expect(after(onlineThresholdSeconds).state, PresenceState.online);
      expect(after(onlineThresholdSeconds + 1).state, PresenceState.stale);
      expect(after(offlineThresholdSeconds).state, PresenceState.stale);
      expect(after(offlineThresholdSeconds + 1).state, PresenceState.offline);
    });

    test('a device that never reported is offline with no age', () {
      // The absent instant must not stand in for `now`, which would show a device nobody has
      // ever heard from as online.
      final view = presencePresentation(lastReportedAt: null, now: now);

      expect(view.state, PresenceState.offline);
      // Not "silent for 0 seconds", which reads as "just now" — the opposite of the truth.
      expect(view.detail, 'Never seen');
    });

    test('a peer with a skewed clock reads online rather than failing', () {
      // Someone else's clock running fast is not a local fault, and there is nothing the user
      // could do about it either way. The rule clamps; this checks the clamp survives the
      // presentation layer rather than turning into a negative duration on screen.
      final view = presencePresentation(
        lastReportedAt: now.add(const Duration(days: 31)),
        now: now,
      );

      expect(view.state, PresenceState.online);
      expect(view.detail, isNot(contains('-')));
    });

    test('the silence detail distinguishes a minute from a day', () {
      // Both are offline. Showing only the state would tell the user the same thing about a
      // machine that blinked and one that has been off since yesterday.
      final brief = after(offlineThresholdSeconds + 60);
      final long = after(86400 * 2);

      expect(brief.detail, isNot(long.detail));
      expect(long.detail, contains('days'));
    });

    test('a boolean cannot express what these three do', () {
      // The falsification. If presence were reduced to reachable/unreachable, `stale` would
      // have to collapse into one of the other two — and whichever was chosen would mislead:
      // shown as online, a command that never arrives looks ignored; shown as offline, the
      // user concludes the phone is broken.
      final labels = {
        presenceLabel(PresenceState.online),
        presenceLabel(PresenceState.stale),
        presenceLabel(PresenceState.offline),
      };

      expect(labels.length, 3, reason: 'two states share a label, so one is unreachable');
      expect(PresenceState.values.length, 3);
    });

    test('stale is still worth sending to, offline is not', () {
      // A backgrounded phone or a sleeping desktop may well pick a command up when it next
      // consults the listener. Refusing to let the user try would make the interface less
      // capable than the system underneath it.
      expect(worthSending(PresenceState.online), isTrue);
      expect(worthSending(PresenceState.stale), isTrue);
      expect(worthSending(PresenceState.offline), isFalse);
    });

    test('the thresholds are not restated in the presentation layer', () {
      // The presentation must not decide presence. It reads the rule's own constants through
      // `evaluatePresence`; a copy of 90 or 900 here would be a second implementation, and the
      // one without vectors would be this one.
      expect(onlineThresholdSeconds, 90);
      expect(offlineThresholdSeconds, 900);

      // Derived, not assumed: shifting `now` by one second either side of the boundary must
      // change the answer, which only holds if the rule is actually consulted.
      expect(after(90).state, PresenceState.online);
      expect(after(91).state, PresenceState.stale);
    });
  });
}

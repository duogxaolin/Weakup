// The presentation of presence and pairing, kept out of the widgets.
//
// Pure functions taking values and returning values, so `test/ui/presence_presentation_test.dart`
// can call them without pumping a widget. The same split `desktop/src/logic.js` makes, and for
// the same reason: a decision the user reads should be testable without a running app.
//
// # Presence is derived here, never reported
//
// [presencePresentation] takes a *last reported instant* and runs [evaluatePresence] over it.
// It does not take a boolean, and there is no parameter through which a relay could supply
// one. That absence is the design:
//
//   - the relay is untrusted, so an online/offline flag from it is a claim rather than a fact;
//   - two devices asking the same relay about one peer would otherwise be able to disagree
//     about it, each repeating whatever it was told;
//   - presence has three states, and any boolean collapses `stale` into one of the other two.
//     A backgrounded phone shown as offline reads as broken; shown as online it makes a
//     command that never arrives look ignored.
//
// The Rust side does the same derivation in `commands/dto.rs`, and
// `shared/testvectors/presence.json` is what keeps the two from drifting.

import '../../domain/presence.dart';

/// A peer's presence and how long it has been silent, ready to render.
final class PresenceView {
  const PresenceView({
    required this.state,
    required this.label,
    required this.detail,
  });

  /// The derived state. Three values, never a boolean.
  final PresenceState state;

  /// Short prose for the state itself.
  final String label;

  /// How long the device has been silent, or that it has never been seen.
  ///
  /// Needed separately from [label] because "silent for a minute" and "silent for a day"
  /// are both [PresenceState.offline] and mean quite different things.
  final String detail;
}

/// Derives a peer's presence from when it last reported.
///
/// `now` is a parameter rather than read from a clock, so every boundary the rule defines is
/// reachable from a test on any machine.
PresenceView presencePresentation({
  required DateTime? lastReportedAt,
  required DateTime now,
}) {
  final evaluation = evaluatePresence(lastSeen: lastReportedAt, now: now);

  return PresenceView(
    state: evaluation.state,
    label: presenceLabel(evaluation.state),
    detail: evaluation.elapsed == null
        // Not "silent for 0 seconds", which would read as "just now" — the opposite of
        // the truth for a device that has never reported at all.
        ? 'Never seen'
        : 'Silent for ${formatSilence(evaluation.elapsed!)}',
  );
}

/// Prose for one presence state.
///
/// "May be asleep" rather than "stale": the user is being told what to expect of the
/// device, and "stale" describes the record rather than the phone.
String presenceLabel(PresenceState state) {
  switch (state) {
    case PresenceState.online:
      return 'Online';
    case PresenceState.stale:
      return 'May be asleep';
    case PresenceState.offline:
      return 'Offline';
  }
}

/// A span of silence in plain language.
///
/// Coarser as it grows, for the reason the desktop's `formatRemaining` gives: a user reading
/// "2 hours 14 minutes 3 seconds" does not care about the 3.
String formatSilence(Duration elapsed) {
  final seconds = elapsed.inSeconds;
  if (seconds < 60) return '$seconds sec';

  final minutes = elapsed.inMinutes;
  if (minutes < 60) return '$minutes min';

  final hours = elapsed.inHours;
  if (hours < 24) return '$hours hr ${minutes % 60} min';

  return '${elapsed.inDays} days';
}

/// Whether a peer is reachable enough that sending it a command should be offered.
///
/// `stale` counts as reachable. A backgrounded phone or a sleeping desktop may well pick the
/// command up when it next consults the listener, and refusing to let the user try would
/// make the interface less capable than the system underneath it. `offline` is the only
/// state where the button is worth disabling, and even then the reason is shown rather than
/// the control silently doing nothing.
bool worthSending(PresenceState state) => state != PresenceState.offline;

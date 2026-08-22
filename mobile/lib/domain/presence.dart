/// How long a device has been silent, and what that means.
///
/// A device that has not reported recently is not simply "offline". There is a middle
/// state, because a phone the operating system has suspended is neither reachable nor
/// gone, and reporting it as either one misleads the user: shown as online, a command
/// sent to it appears to be ignored; shown as offline, the user concludes the device
/// has stopped working.
///
/// Pure, in the same sense [TriggerResolver] is pure: `now` is passed in rather than
/// read from a clock, so every boundary is reachable from a test on any machine and the
/// shared vectors in `shared/testvectors/presence.json` are deterministic.
///
/// Mirrors `desktop/src-tauri/src/domain/presence.rs`. Those vectors are what keep the
/// two implementations from drifting.
library;

/// Silence up to and including this is still [PresenceState.online].
///
/// 90 seconds tolerates one missed report on a 60-second heartbeat plus jitter, without
/// requiring a second missed one. Not user-configurable: a tunable presence window would
/// let a device be configured to look online indefinitely.
const int onlineThresholdSeconds = 90;

/// Silence beyond this is [PresenceState.offline]; up to and including it is
/// [PresenceState.stale].
///
/// 15 minutes matches the magnitude of the power-off overdue tolerance, reusing a
/// timescale the codebase already reasons about rather than introducing a third. It is a
/// separate constant regardless, because it answers a different question and coupling the
/// two would make one unchangeable without the other.
const int offlineThresholdSeconds = 900;

/// How reachable a device is, as three states rather than a boolean.
///
/// The names are the ones both implementations serialise, so they are a cross-language
/// contract rather than an internal detail.
enum PresenceState {
  /// Reported within the online window; a command sent now should arrive.
  online,

  /// Silent past the online window but within the offline one. Typically a phone the OS
  /// has backgrounded: it may come back on its own.
  stale,

  /// Silent past the offline window, or never seen at all.
  offline,
}

/// A presence decision and the silence it was decided from.
///
/// The elapsed time travels with the state rather than being left to the caller to
/// subtract. The UI needs it — "silent for one minute" and "silent for a day" are both
/// [PresenceState.offline] and mean quite different things — and a second subtraction
/// outside this function is where the two implementations would drift by an off-by-one.
final class PresenceEvaluation {
  const PresenceEvaluation({required this.state, required this.elapsed});

  final PresenceState state;

  /// Null only when the device has never reported: there is no age to give.
  /// Never negative — see the clamp in [evaluatePresence].
  final Duration? elapsed;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      (other is PresenceEvaluation &&
          other.state == state &&
          other.elapsed == elapsed);

  @override
  int get hashCode => Object.hash(state, elapsed);

  @override
  String toString() => 'PresenceEvaluation($state, elapsed=$elapsed)';
}

/// Derives a device's presence from its last report and the current instant.
///
/// A [lastSeen] in the *future* — a peer whose clock is skewed — clamps to zero elapsed
/// and reports [PresenceState.online]. Treating it as an error would make the peer's
/// clock skew look like a local failure, and there is nothing the local user could do
/// about it either way. Two vector cases pin this, at five seconds and at a month of skew.
///
/// Both boundaries are inclusive-online: silence of exactly [onlineThresholdSeconds] is
/// online, and of exactly [offlineThresholdSeconds] is stale. That is a decision rather
/// than an accident of which comparison operator was typed, which is why each boundary is
/// pinned from both sides in the shared vectors.
PresenceEvaluation evaluatePresence({
  required DateTime? lastSeen,
  required DateTime now,
}) {
  if (lastSeen == null) {
    // Never reported. The absent value must not stand in for `now`, which would report a
    // device that has never been seen as online.
    return const PresenceEvaluation(
      state: PresenceState.offline,
      elapsed: null,
    );
  }

  final rawElapsed = now.difference(lastSeen);
  final elapsed = rawElapsed.isNegative ? Duration.zero : rawElapsed;
  final seconds = elapsed.inSeconds;

  final state = switch (seconds) {
    _ when seconds <= onlineThresholdSeconds => PresenceState.online,
    _ when seconds <= offlineThresholdSeconds => PresenceState.stale,
    _ => PresenceState.offline,
  };

  return PresenceEvaluation(state: state, elapsed: elapsed);
}

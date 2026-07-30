import 'package:timezone/timezone.dart' as tz;

import '../core/app_error.dart';
import '../core/result.dart';
import 'job_enums.dart';
import 'trigger_spec.dart';

/// The overdue threshold for power-off jobs.
///
/// Past this, a stale intent must not trigger an irreversible action: the user
/// closed the lid at 23:00 for a 23:30 shutdown and reopened it the next
/// morning; powering off then is not what they asked for.
const Duration kPowerOffOvertolerance = Duration(minutes: 15);

/// What reconciliation decided about a single job, with no side effects.
///
/// Mirrors `ReconcileOutcome` in `desktop/`. The wire names in
/// `shared/testvectors/reconciliation.json` are the camelCase enum names.
enum ReconcileOutcome {
  /// Target has not been reached; the job keeps waiting.
  stillPending,

  /// A keep-awake job whose target passed: complete it and release the assertion.
  completed,

  /// A power-off job due or overdue within tolerance: run the grace-period countdown.
  proceedToGracePeriod,

  /// A power-off job overdue beyond tolerance: do NOT power off; notify the user.
  overdue,
}

/// Pure trigger resolution logic.
/// No Flutter or plugin imports — fully unit-testable.
abstract final class TriggerResolver {
  TriggerResolver._();

  /// Decides what to do with a job whose state is [jobType] and
  /// [targetInstantUtc], as of [now]. Pure: the caller performs the effects.
  ///
  /// A target exactly equal to [now] is *due*, not pending — returning
  /// [ReconcileOutcome.stillPending] there would drop the job, because the
  /// scheduler only arms timers for targets strictly after [now].
  ///
  /// Mirrors `TriggerResolver::reconcile` in `desktop/`. Both are exercised by
  /// `shared/testvectors/reconciliation.json`.
  static ReconcileOutcome reconcile({
    required JobType jobType,
    required DateTime? targetInstantUtc,
    required DateTime now,
  }) {
    // Indefinite keep-awake: no target, so nothing to reconcile.
    if (targetInstantUtc == null) return ReconcileOutcome.stillPending;

    if (targetInstantUtc.isAfter(now)) return ReconcileOutcome.stillPending;

    return switch (jobType) {
      JobType.keepAwake => ReconcileOutcome.completed,
      JobType.powerOff =>
        now.difference(targetInstantUtc) <= kPowerOffOvertolerance
            ? ReconcileOutcome.proceedToGracePeriod
            : ReconcileOutcome.overdue,
    };
  }

  /// Validates [trigger] against [type] and returns a [Result].
  ///
  /// Rules:
  /// - [IndefiniteTrigger] is valid only for [JobType.keepAwake].
  /// - [DurationTrigger.minutes] must be in 1..1440.
  /// - [AbsoluteTimeTrigger.hour] must be in 0..23 and
  ///   [AbsoluteTimeTrigger.minute] in 0..59.
  ///
  /// The out-of-range check on absolute time is not cosmetic: `TZDateTime`
  /// normalises hour 24 into 00:00 of the following day, so an unvalidated
  /// typo would silently schedule a power-off a day later than the user asked.
  ///
  /// Mirrors `TriggerResolver::validate` in `desktop/`. Both are exercised by
  /// `shared/testvectors/validation.json`.
  static Result<void> validate(TriggerSpec trigger, JobType type) {
    return switch (trigger) {
      IndefiniteTrigger() => type == JobType.powerOff
          ? Result.failure(const ValidationError(
              message: 'Power-off requires a duration or a specific time, not indefinite.',
            ))
          : Result.success(null),
      DurationTrigger(:final minutes) => switch (minutes) {
          <= 0 => Result.failure(const ValidationError(
              message: 'Duration must be positive (at least 1 minute).',
            )),
          > 1440 => Result.failure(const ValidationError(
              message: 'Duration cannot exceed 24 hours (1440 minutes).',
            )),
          _ => Result.success(null),
        },
      AbsoluteTimeTrigger(:final hour, :final minute) => switch ((hour, minute)) {
          _ when hour < 0 || hour > 23 => Result.failure(const ValidationError(
              message: 'Hour must be between 0 and 23.',
            )),
          _ when minute < 0 || minute > 59 => Result.failure(const ValidationError(
              message: 'Minute must be between 0 and 59.',
            )),
          _ => Result.success(null),
        },
    };
  }

  /// Resolves [trigger] to an absolute UTC target instant.
  ///
  /// - [IndefiniteTrigger] → returns null (no target instant).
  /// - [DurationTrigger] → now + duration, in real elapsed time.
  /// - [AbsoluteTimeTrigger] → the next future occurrence of that wall time in
  ///   [location].
  ///
  /// DST rules (absolute-time only):
  /// - Spring-forward gap: the requested time does not exist; resolve to the
  ///   instant the clock jumps to (i.e. the transition instant).
  /// - Fall-back overlap: the requested time occurs twice; resolve to the
  ///   **first** (earlier/pre-transition) occurrence.
  ///
  /// [now] defaults to the current moment in [location] and is injectable for testing.
  ///
  /// Mirrors `TriggerResolver::resolve` in `desktop/`. Both are exercised by
  /// `shared/testvectors/resolution.json`.
  static DateTime? resolve(
    TriggerSpec trigger,
    tz.Location location, {
    DateTime? now,
  }) {
    final tzNow = now != null
        ? tz.TZDateTime.from(now, location)
        : tz.TZDateTime.now(location);

    return switch (trigger) {
      IndefiniteTrigger() => null,
      DurationTrigger(:final minutes) =>
        _toPlainUtc(tzNow.add(Duration(minutes: minutes))),
      AbsoluteTimeTrigger(:final hour, :final minute) =>
        _toPlainUtc(_resolveAbsolute(hour, minute, location, tzNow)),
    };
  }

  /// Converts to a plain UTC [DateTime].
  ///
  /// Not cosmetic. `TZDateTime.toUtc()` returns another `TZDateTime`, and
  /// `TZDateTime.operator==` demands both a `TZDateTime` *and* an equal
  /// location — so a resolved target would compare unequal to the very same
  /// instant loaded back from the database, silently and with no type error.
  /// Returning a plain instant removes that trap and matches the
  /// `DateTime<Utc>` the desktop implementation returns.
  static DateTime _toPlainUtc(tz.TZDateTime value) =>
      DateTime.fromMicrosecondsSinceEpoch(value.microsecondsSinceEpoch, isUtc: true);

  /// Resolves an absolute time-of-day to a [tz.TZDateTime], handling DST correctly.
  static tz.TZDateTime _resolveAbsolute(
    int hour,
    int minute,
    tz.Location location,
    tz.TZDateTime tzNow,
  ) {
    // Try today first.
    var candidate = _wallTimeOn(location, tzNow.year, tzNow.month, tzNow.day, hour, minute);

    // If that's not strictly in the future, roll to tomorrow. Month/year
    // rollover is handled by DateTime's own normalisation of day + 1.
    if (!candidate.isAfter(tzNow)) {
      final tomorrow = DateTime.utc(tzNow.year, tzNow.month, tzNow.day + 1);
      candidate = _wallTimeOn(
          location, tomorrow.year, tomorrow.month, tomorrow.day, hour, minute);
    }

    return candidate;
  }

  /// Resolves the wall time `hour:minute` on a given calendar date, handling the
  /// two DST anomalies.
  ///
  /// Fall-back overlap: `TZDateTime` picks the earlier occurrence, which is what
  /// the spec asks for, so it is used as-is.
  ///
  /// Spring-forward gap: `TZDateTime` does **not** give the transition instant.
  /// Asking for 02:30 on a day when 02:00 jumps to 03:00 yields 03:30 — it keeps
  /// the requested offset-from-midnight and lands half an hour *past* the jump.
  /// For a shutdown that is a real error: the machine powers off 30 minutes later
  /// than the user was shown. So the gap is detected by checking whether the
  /// result actually reports the wall time that was requested, and resolved by
  /// scanning forward to the first wall time that exists — the jump instant.
  static tz.TZDateTime _wallTimeOn(
    tz.Location location,
    int year,
    int month,
    int day,
    int hour,
    int minute,
  ) {
    final exact = _exactWallTime(location, year, month, day, hour, minute);
    if (exact != null) return exact;

    // Inside a gap. Real-world gaps run from 30 minutes up to a full day
    // (Pacific/Apia skipped 2011-12-30 entirely), so a one-day scan is
    // guaranteed to clear any of them.
    const maxGapScanMinutes = 24 * 60;
    for (var offset = 1; offset <= maxGapScanMinutes; offset++) {
      final probeWall = DateTime.utc(year, month, day, hour, minute)
          .add(Duration(minutes: offset));
      final probe = _exactWallTime(location, probeWall.year, probeWall.month,
          probeWall.day, probeWall.hour, probeWall.minute);
      if (probe != null) return probe;
    }

    // Unreachable: no timezone gap in the IANA database spans more than a day,
    // so the scan above always finds an existing wall time. Loud rather than
    // silently wrong if that ever stops holding.
    throw StateError(
      'Could not resolve $hour:$minute on $year-$month-$day in ${location.name}',
    );
  }

  /// Returns the instant for a wall time, or null if that wall time does not
  /// exist in [location] (a spring-forward gap).
  ///
  /// Existence is verified by round-trip: construct the instant, then read its
  /// wall-clock fields back. A gap is exactly the case where they differ.
  static tz.TZDateTime? _exactWallTime(
    tz.Location location,
    int year,
    int month,
    int day,
    int hour,
    int minute,
  ) {
    final candidate = tz.TZDateTime(location, year, month, day, hour, minute);
    final matches = candidate.year == year &&
        candidate.month == month &&
        candidate.day == day &&
        candidate.hour == hour &&
        candidate.minute == minute;
    return matches ? candidate : null;
  }
}

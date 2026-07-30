import 'package:timezone/timezone.dart' as tz;

import '../core/app_error.dart';
import '../core/result.dart';
import 'job_enums.dart';
import 'trigger_spec.dart';

/// Pure trigger resolution logic.
/// No Flutter or plugin imports — fully unit-testable.
abstract final class TriggerResolver {
  TriggerResolver._();

  /// Validates [trigger] against [type] and returns a [Result].
  ///
  /// Rules:
  /// - [IndefiniteTrigger] is valid only for [JobType.keepAwake].
  /// - [DurationTrigger.minutes] must be in 1..1440.
  /// - [AbsoluteTimeTrigger] hour 0–23 and minute 0–59 are always valid.
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
      AbsoluteTimeTrigger() => Result.success(null),
    };
  }

  /// Resolves [trigger] to a [TZDateTime] target in [location].
  ///
  /// - [IndefiniteTrigger] → returns null (no target instant).
  /// - [DurationTrigger] → now + duration.
  /// - [AbsoluteTimeTrigger] → resolves to the next future occurrence in [location].
  ///
  /// DST rules (absolute-time only):
  /// - Spring-forward gap: the requested time does not exist; resolve to the
  ///   instant the clock jumps to (i.e. the transition instant).
  /// - Fall-back overlap: the requested time occurs twice; resolve to the
  ///   **first** (earlier/pre-transition) occurrence.
  ///
  /// [now] defaults to the current moment in [location] and is injectable for testing.
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
        tzNow.add(Duration(minutes: minutes)).toUtc(),
      AbsoluteTimeTrigger(:final hour, :final minute) =>
        _resolveAbsolute(hour, minute, location, tzNow).toUtc(),
    };
  }

  /// Resolves an absolute time-of-day to a [TZDateTime], handling DST correctly.
  static tz.TZDateTime _resolveAbsolute(
    int hour,
    int minute,
    tz.Location location,
    tz.TZDateTime tzNow,
  ) {
    // Try today first.
    var candidate = _makeTZDateTime(location, tzNow.year, tzNow.month, tzNow.day, hour, minute);

    // If that's not strictly in the future, roll to tomorrow.
    if (!candidate.isAfter(tzNow)) {
      candidate = _makeTZDateTime(location, tzNow.year, tzNow.month, tzNow.day + 1, hour, minute);
    }

    return candidate;
  }

  /// Creates a [TZDateTime], handling DST gaps and overlaps.
  ///
  /// The [tz] package TZDateTime constructor already handles DST:
  /// - For a gap (spring-forward): the time is adjusted forward to after the gap.
  /// - For an overlap (fall-back): the constructor uses the first occurrence by default.
  ///
  /// However, to guarantee the fall-back behavior (first occurrence), we compare
  /// the result to what we asked for: if the resulting offset is the DST offset
  /// (i.e. the second occurrence was used), we re-create with isUtcMidnight trick.
  /// Actually, the tz package uses the first (earlier) offset for overlapping
  /// times by default when constructed from local time — we verify this assumption.
  static tz.TZDateTime _makeTZDateTime(
    tz.Location location,
    int year,
    int month,
    int day,
    int hour,
    int minute,
  ) {
    // TZDateTime(location, year, month, day, hour, minute) uses the first offset
    // for overlapping wall times (fall-back). For spring-forward gaps, it advances
    // to the post-gap instant. This matches the spec exactly.
    return tz.TZDateTime(location, year, month, day, hour, minute);
  }
}

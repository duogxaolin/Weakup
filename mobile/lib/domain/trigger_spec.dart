/// Sealed trigger specification — what conditions fire a job.
library;

import 'calendar_date.dart';

sealed class TriggerSpec {
  const TriggerSpec();
}

/// The job runs until cancelled manually (only valid for [JobType.keepAwake]).
final class IndefiniteTrigger extends TriggerSpec {
  const IndefiniteTrigger();

  @override
  bool operator ==(Object other) =>
      identical(this, other) || other is IndefiniteTrigger;

  @override
  int get hashCode => runtimeType.hashCode;

  @override
  String toString() => 'IndefiniteTrigger';
}

/// The job fires after [minutes] minutes from activation.
/// Must be in range 1..1440 (1 minute to 24 hours).
final class DurationTrigger extends TriggerSpec {
  const DurationTrigger({required this.minutes});
  final int minutes;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      (other is DurationTrigger && other.minutes == minutes);

  @override
  int get hashCode => minutes.hashCode;

  @override
  String toString() => 'DurationTrigger(${minutes}min)';
}

/// The job fires at a wall-clock time in the device's timezone.
///
/// [date] decides which of two semantics applies, and they differ in one important way:
///
/// - `null` — a *time of day*, resolved to its next occurrence. If that time has already
///   passed today it rolls to tomorrow.
/// - non-null — a *one-off instant* on exactly that local date. It does **not** roll
///   forward; a dated instant that has passed is refused, because moving an irreversible
///   power-off to a day the user never chose is worse than refusing.
///
/// Mirrors `TriggerSpec::AbsoluteTime` in `desktop/`. On the wire the date is an optional
/// `YYYY-MM-DD` string, and its absence is the pre-existing shape.
final class AbsoluteTimeTrigger extends TriggerSpec {
  const AbsoluteTimeTrigger({
    required this.hour,
    required this.minute,
    this.date,
  });

  /// 0–23
  final int hour;

  /// 0–59
  final int minute;

  /// The exact local date to fire on, or null for the next occurrence of [hour]:[minute].
  final CalendarDate? date;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      (other is AbsoluteTimeTrigger &&
          other.hour == hour &&
          other.minute == minute &&
          other.date == date);

  @override
  int get hashCode => Object.hash(hour, minute, date);

  @override
  String toString() {
    final time = '${hour.toString().padLeft(2, '0')}:'
        '${minute.toString().padLeft(2, '0')}';
    return date == null
        ? 'AbsoluteTimeTrigger($time)'
        : 'AbsoluteTimeTrigger($time on ${date!.format()})';
  }
}

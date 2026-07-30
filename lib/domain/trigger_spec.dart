/// Sealed trigger specification — what conditions fire a job.
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

/// The job fires at a specific wall-clock time (e.g. 23:30).
/// Resolves to the next future occurrence in the device's timezone.
final class AbsoluteTimeTrigger extends TriggerSpec {
  const AbsoluteTimeTrigger({required this.hour, required this.minute});

  /// 0–23
  final int hour;

  /// 0–59
  final int minute;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      (other is AbsoluteTimeTrigger &&
          other.hour == hour &&
          other.minute == minute);

  @override
  int get hashCode => Object.hash(hour, minute);

  @override
  String toString() => 'AbsoluteTimeTrigger(${hour.toString().padLeft(2, '0')}:${minute.toString().padLeft(2, '0')})';
}

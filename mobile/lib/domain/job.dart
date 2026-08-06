import 'job_enums.dart';
import 'trigger_spec.dart';

/// A scheduled job persisted to the database.
/// The [targetInstantUtc] is always the source of truth; remaining time is always
/// computed as `targetInstantUtc − now`, never stored as a decrementing counter.
final class Job {
  const Job({
    required this.id,
    required this.type,
    required this.trigger,
    required this.status,
    this.targetInstantUtc,
    required this.createdAt,
    required this.updatedAt,
    this.failureMessage,
    this.origin = JobOrigin.local,
  });

  final int id;
  final JobType type;
  final TriggerSpec trigger;
  final JobStatus status;

  /// UTC target instant. Null for [IndefiniteTrigger] (no target).
  final DateTime? targetInstantUtc;

  final DateTime createdAt;
  final DateTime updatedAt;

  /// Set when status is [JobStatus.failed]; human-readable description.
  final String? failureMessage;

  /// Where the request that created this job came from. Decides the countdown length for
  /// a power-off, and nothing else.
  ///
  /// Defaulted rather than required: a job constructed before this field existed meant
  /// [JobOrigin.local], so no existing call site changes behaviour. In-memory only in
  /// this change — see [JobOrigin] for the gap that leaves and what it blocks.
  final JobOrigin origin;

  /// Returns remaining duration until the target instant, or null.
  Duration? remainingFrom(DateTime now) {
    final target = targetInstantUtc;
    if (target == null) return null;
    final remaining = target.difference(now);
    return remaining.isNegative ? Duration.zero : remaining;
  }

  Job copyWith({
    int? id,
    JobType? type,
    TriggerSpec? trigger,
    JobStatus? status,
    DateTime? targetInstantUtc,
    DateTime? createdAt,
    DateTime? updatedAt,
    String? failureMessage,
    JobOrigin? origin,
  }) {
    return Job(
      id: id ?? this.id,
      type: type ?? this.type,
      trigger: trigger ?? this.trigger,
      status: status ?? this.status,
      targetInstantUtc: targetInstantUtc ?? this.targetInstantUtc,
      createdAt: createdAt ?? this.createdAt,
      updatedAt: updatedAt ?? this.updatedAt,
      failureMessage: failureMessage ?? this.failureMessage,
      origin: origin ?? this.origin,
    );
  }

  @override
  bool operator ==(Object other) =>
      identical(this, other) || (other is Job && other.id == id);

  @override
  int get hashCode => id.hashCode;

  @override
  String toString() =>
      'Job(id=$id, type=$type, status=$status, trigger=$trigger, target=$targetInstantUtc)';
}

import 'package:drift/drift.dart';

import '../domain/job.dart';
import '../domain/job_enums.dart';
import '../domain/trigger_spec.dart';
import 'app_database.dart';

/// Maps between [Job] domain entities and Drift [JobsCompanion]/[JobRow] rows.
abstract final class JobMapper {
  JobMapper._();

  static Job toDomain(JobRow row) {
    return Job(
      id: row.id,
      type: _parseType(row.type),
      trigger: _parseTrigger(row),
      status: _parseStatus(row.status),
      targetInstantUtc: row.targetInstantUtc,
      createdAt: row.createdAt,
      updatedAt: row.updatedAt,
      failureMessage: row.failureMessage,
    );
  }

  static JobsCompanion toCompanion(Job job) {
    return JobsCompanion.insert(
      type: job.type.name,
      triggerKind: _triggerKindString(job.trigger),
      triggerMinutes: Value(_triggerMinutes(job.trigger)),
      triggerHour: Value(_triggerHour(job.trigger)),
      triggerMinute: Value(_triggerMinuteVal(job.trigger)),
      status: job.status.name,
      targetInstantUtc: Value(job.targetInstantUtc),
      createdAt: job.createdAt,
      updatedAt: job.updatedAt,
      failureMessage: Value(job.failureMessage),
    );
  }

  // ---- helpers ----

  static JobType _parseType(String s) => switch (s) {
        'keepAwake' => JobType.keepAwake,
        'powerOff' => JobType.powerOff,
        _ => throw StateError('Unknown job type: $s'),
      };

  static JobStatus _parseStatus(String s) => switch (s) {
        'active' => JobStatus.active,
        'paused' => JobStatus.paused,
        'completed' => JobStatus.completed,
        'cancelled' => JobStatus.cancelled,
        'failed' => JobStatus.failed,
        'overdue' => JobStatus.overdue,
        'degraded' => JobStatus.degraded,
        _ => throw StateError('Unknown job status: $s'),
      };

  static TriggerSpec _parseTrigger(JobRow row) => switch (row.triggerKind) {
        'indefinite' => const IndefiniteTrigger(),
        'duration' => DurationTrigger(minutes: row.triggerMinutes!),
        'absoluteTime' => AbsoluteTimeTrigger(
            hour: row.triggerHour!,
            minute: row.triggerMinute!,
          ),
        _ => throw StateError('Unknown trigger kind: ${row.triggerKind}'),
      };

  static String _triggerKindString(TriggerSpec t) => switch (t) {
        IndefiniteTrigger() => 'indefinite',
        DurationTrigger() => 'duration',
        AbsoluteTimeTrigger() => 'absoluteTime',
      };

  static int? _triggerMinutes(TriggerSpec t) => switch (t) {
        DurationTrigger(:final minutes) => minutes,
        _ => null,
      };

  static int? _triggerHour(TriggerSpec t) => switch (t) {
        AbsoluteTimeTrigger(:final hour) => hour,
        _ => null,
      };

  static int? _triggerMinuteVal(TriggerSpec t) => switch (t) {
        AbsoluteTimeTrigger(:final minute) => minute,
        _ => null,
      };
}

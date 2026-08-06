import 'package:drift/drift.dart';

import '../domain/calendar_date.dart';
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
      origin: _parseOrigin(row.origin),
    );
  }

  static JobsCompanion toCompanion(Job job) {
    return JobsCompanion.insert(
      type: job.type.name,
      triggerKind: _triggerKindString(job.trigger),
      triggerMinutes: Value(_triggerMinutes(job.trigger)),
      triggerHour: Value(_triggerHour(job.trigger)),
      triggerMinute: Value(_triggerMinuteVal(job.trigger)),
      triggerDate: Value(_triggerDate(job.trigger)),
      status: job.status.name,
      targetInstantUtc: Value(job.targetInstantUtc),
      createdAt: job.createdAt,
      updatedAt: job.updatedAt,
      failureMessage: Value(job.failureMessage),
      // Written explicitly rather than left null so a remote job reads back as remote. A
      // null would be read as local, which for a remote job means the shorter countdown.
      origin: Value(job.origin.name),
    );
  }

  // ---- helpers ----

  /// Null reads as [JobOrigin.local]: every row written before the column existed was
  /// scheduled at this device, so this is a faithful reading of old data rather than a
  /// default standing in for information that was lost.
  ///
  /// An unrecognised non-null value throws, matching how an unparseable `trigger_date` is
  /// handled and what the desktop repository does for the same case. Silently reading it as
  /// local would shorten a power-off countdown, which is the failure the column prevents.
  static JobOrigin _parseOrigin(String? raw) => switch (raw) {
        null => JobOrigin.local,
        'local' => JobOrigin.local,
        'remote' => JobOrigin.remote,
        _ => throw StateError('Row holds an unknown job origin: $raw'),
      };

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
            // Null for every row written before the column existed, which is
            // exactly the undated semantic those rows meant.
            date: _parseDate(row.triggerDate),
          ),
        _ => throw StateError('Unknown trigger kind: ${row.triggerKind}'),
      };

  /// A stored date that will not parse is a corrupt row, not an undated trigger.
  /// Silently treating it as undated would turn a fixed-date power-off into a daily
  /// alarm, so it fails loudly instead — matching the desktop repository, which
  /// returns a storage error for the same case.
  static CalendarDate? _parseDate(String? raw) {
    if (raw == null) return null;
    final parsed = CalendarDate.tryParse(raw);
    if (parsed == null) {
      throw StateError('Row holds an unparseable trigger_date: $raw');
    }
    return parsed;
  }

  static String? _triggerDate(TriggerSpec t) => switch (t) {
        AbsoluteTimeTrigger(:final date) => date?.format(),
        _ => null,
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

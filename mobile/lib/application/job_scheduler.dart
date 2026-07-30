import 'dart:async';

import 'package:timezone/timezone.dart' as tz;

import '../core/app_error.dart';
import '../core/result.dart';
import '../domain/job.dart';
import '../domain/job_enums.dart';
import '../domain/job_repository.dart';
import '../domain/trigger_resolver.dart';
import '../domain/trigger_spec.dart';
import '../platform/foreground_service_controller.dart';
import '../platform/notification_service.dart';
import '../platform/power_off_executor.dart';
import '../platform/wakelock_controller.dart';

/// The overdue threshold for power-off jobs lives in `domain/` alongside the
/// reconciliation rule it belongs to, and is re-exported here because callers
/// have always read it from the scheduler.
export '../domain/trigger_resolver.dart' show kPowerOffOvertolerance;

/// Duration of the mandatory grace-period countdown before power-off.
const Duration kGracePeriod = Duration(seconds: 60);

/// Callback invoked when the grace-period countdown should be shown.
/// Receives the job that fired. The callback must call [cancelCallback] or
/// [proceedCallback] at the end of the countdown.
typedef GracePeriodStarted = void Function(
  Job job,
  VoidCallback cancelCallback,
  VoidCallback proceedCallback,
);
typedef VoidCallback = void Function();

/// The central job scheduler.
///
/// Holds a single in-process timer armed for the nearest pending job.
/// Re-arms after each fire. OS-level notifications serve as a durability
/// backstop, not as a replacement.
class JobScheduler {
  JobScheduler({
    required this._repository,
    required PowerOffExecutor powerOffExecutor,
    required WakelockController wakelockController,
    required NotificationService notificationService,
    this._foregroundService = const NoopForegroundServiceController(),
    this._onGracePeriodStarted,
    this._onJobFailed,
    this._onJobOverdue,
  })  : _powerOff = powerOffExecutor,
        _wakelock = wakelockController,
        _notifications = notificationService;

  final JobRepository _repository;
  final PowerOffExecutor _powerOff;
  final WakelockController _wakelock;
  final NotificationService _notifications;
  final ForegroundServiceController _foregroundService;
  final GracePeriodStarted? _onGracePeriodStarted;
  final void Function(Job job, AppError error)? _onJobFailed;
  final void Function(Job job)? _onJobOverdue;

  Timer? _nextTimer;
  bool _gracePeriodActive = false;

  /// Must be called on app start and every resume.
  /// Reconciles overdue jobs, re-acquires wakelock, re-arms timer.
  Future<void> reconcile() async {
    final activeResult = await _repository.getActiveJobs();
    if (activeResult.isFailure) return;

    final activeJobs = activeResult.valueOrNull!;
    final now = DateTime.now().toUtc();

    for (final job in activeJobs) {
      await _reconcileJob(job, now);
    }

    await _syncForegroundService();
    await _armNextTimer();
  }

  /// Applies the effects for one job. The *decision* is
  /// [TriggerResolver.reconcile], which is pure and shared with `desktop/` via
  /// `shared/testvectors/reconciliation.json`; this method only carries it out.
  Future<void> _reconcileJob(Job job, DateTime now) async {
    final outcome = TriggerResolver.reconcile(
      jobType: job.type,
      targetInstantUtc: job.targetInstantUtc,
      now: now,
    );

    switch (outcome) {
      case ReconcileOutcome.stillPending:
        // Keep-awake must keep asserting across a restart; the timer handles
        // the eventual fire. Power-off needs nothing until then.
        if (job.type == JobType.keepAwake && !_wakelock.isHeld) {
          await _wakelock.acquire();
        }

      case ReconcileOutcome.completed:
        // Keep-awake window elapsed — complete and release.
        await _completeKeepAwakeJob(job);

      case ReconcileOutcome.proceedToGracePeriod:
        _startGracePeriod(job);

      case ReconcileOutcome.overdue:
        // Too stale to act on — never powers off.
        await _markOverdue(job);
    }
  }

  /// Arms a timer for the nearest pending job with a target instant.
  Future<void> _armNextTimer() async {
    _nextTimer?.cancel();

    final activeResult = await _repository.getActiveJobs();
    if (activeResult.isFailure) return;
    final jobs = activeResult.valueOrNull!;

    final now = DateTime.now().toUtc();
    DateTime? nearest;
    Job? nearestJob;

    for (final job in jobs) {
      final target = job.targetInstantUtc;
      if (target == null) continue;
      if (target.isAfter(now) && (nearest == null || target.isBefore(nearest))) {
        nearest = target;
        nearestJob = job;
      }
    }

    if (nearestJob == null || nearest == null) return;

    final delay = nearest.difference(now);
    _nextTimer = Timer(delay, () => _onTimerFired(nearestJob!));
  }

  void _onTimerFired(Job job) async {
    // Reload the job to check it hasn't been cancelled meanwhile.
    final activeResult = await _repository.getActiveJobs();
    if (activeResult.isFailure) return;
    final current = activeResult.valueOrNull!.where((j) => j.id == job.id).firstOrNull;
    if (current == null || current.status != JobStatus.active) {
      await _armNextTimer();
      return;
    }

    if (current.type == JobType.keepAwake) {
      await _completeKeepAwakeJob(current);
    } else if (current.type == JobType.powerOff) {
      _startGracePeriod(current);
    }

    // Re-arm for any remaining jobs.
    await _armNextTimer();
  }

  void _startGracePeriod(Job job) {
    if (_gracePeriodActive) return; // avoid double-triggering
    _gracePeriodActive = true;

    // Schedule a notification so the user knows shutdown is imminent.
    _notifications.showImmediate(
      id: job.id,
      title: 'Power-off in 60 seconds',
      body: 'Cancel now to abort the scheduled power-off.',
    );

    _onGracePeriodStarted?.call(
      job,
      () => _cancelGracePeriod(job), // cancel callback
      () => _executePowerOff(job), // proceed callback
    );
  }

  void _cancelGracePeriod(Job job) {
    _gracePeriodActive = false;
    _repository.updateStatus(job.id, JobStatus.cancelled);
    _wakelock.release();
    _notifications.cancel(job.id);
    // Fire and forget — cancel is best-effort, no await needed here.
    _syncForegroundService();
  }

  Future<void> _executePowerOff(Job job) async {
    _gracePeriodActive = false;

    // One last safety check: confirm the job is still active.
    final activeResult = await _repository.getActiveJobs();
    final current = activeResult.valueOrNull
        ?.where((j) => j.id == job.id && j.status == JobStatus.active)
        .firstOrNull;
    if (current == null) return;

    final result = await _powerOff.powerOff();
    result.fold(
      onSuccess: (_) {
        _repository.updateStatus(job.id, JobStatus.completed);
      },
      onFailure: (error) {
        _repository.updateStatus(
          job.id,
          JobStatus.failed,
          failureMessage: _errorMessage(error),
        );
        _onJobFailed?.call(job, error);
      },
    );
    await _syncForegroundService();
  }

  Future<void> _completeKeepAwakeJob(Job job) async {
    await _wakelock.release();
    await _repository.updateStatus(job.id, JobStatus.completed);
    await _notifications.showImmediate(
      id: job.id,
      title: 'Keep-awake completed',
      body: 'Your screen will now sleep normally.',
    );
    await _syncForegroundService();
  }

  Future<void> _markOverdue(Job job) async {
    await _repository.updateStatus(job.id, JobStatus.overdue);
    await _notifications.showImmediate(
      id: job.id,
      title: 'Scheduled power-off skipped',
      body: 'The scheduled power-off was skipped because the app was not running.',
    );
    _onJobOverdue?.call(job);
    await _syncForegroundService();
  }

  // ---- Background (foreground service) management ----

  /// Starts or stops the background service based on whether any active jobs
  /// remain. No-op on platforms without a foreground service.
  ///
  /// If the platform refuses to start the service — on Android 15+ most often
  /// the 6-hour/day `dataSync` cap — every active job is marked
  /// [JobStatus.degraded] and the user is notified, so a job never silently
  /// vanishes when the OS takes background execution away.
  Future<void> _syncForegroundService() async {
    if (!_foregroundService.isSupported) return;

    final result = await _repository.getActiveJobs();
    final activeJobs = result.valueOrNull ?? const <Job>[];

    if (activeJobs.isEmpty) {
      await _foregroundService.stop();
      return;
    }

    final outcome =
        await _foregroundService.start(activeJobCount: activeJobs.length);
    if (outcome != ForegroundServiceStartOutcome.refused) return;

    // Background execution is gone. Stop the (dead) service so we do not leave
    // a stale notification behind, then degrade every affected job.
    await _foregroundService.stop();

    for (final job in activeJobs) {
      await _repository.updateStatus(job.id, JobStatus.degraded);
      await _notifications.showImmediate(
        id: job.id,
        title: 'Background limit reached',
        body: 'Android stopped Weakup from running in the background, so this '
            'job may not fire on time. Reopen Weakup to restore full '
            'scheduling.',
      );
    }
  }

  /// Activates a new job: persists it, acquires resources, arms timer.
  Future<Result<Job>> activateJob(Job job) async {
    final result = await _repository.insert(job);
    if (result.isFailure) return result;

    final inserted = result.valueOrNull!;

    if (inserted.type == JobType.keepAwake) {
      await _wakelock.acquire();
    }

    if (inserted.type == JobType.powerOff && inserted.targetInstantUtc != null) {
      await _scheduleNotificationBackstop(inserted);
    }

    await _syncForegroundService();
    await _armNextTimer();
    return Result.success(inserted);
  }

  /// Replaces an existing active job of the same type.
  Future<Result<Job>> replaceJob(Job newJob) async {
    // Release wakelock if replacing a keepAwake job.
    if (newJob.type == JobType.keepAwake && _wakelock.isHeld) {
      await _wakelock.release();
    }

    final result = await _repository.replaceActiveJobOfType(newJob.type, newJob);
    if (result.isFailure) return result;

    final inserted = result.valueOrNull!;

    if (inserted.type == JobType.keepAwake) {
      await _wakelock.acquire();
    }

    if (inserted.type == JobType.powerOff && inserted.targetInstantUtc != null) {
      await _scheduleNotificationBackstop(inserted);
    }

    await _syncForegroundService();
    await _armNextTimer();
    return Result.success(inserted);
  }

  /// Pauses a job: releases resources but keeps it in the database.
  Future<Result<void>> pauseJob(int jobId) async {
    final activeResult = await _repository.getActiveJobs();
    final job = activeResult.valueOrNull?.where((j) => j.id == jobId).firstOrNull;
    if (job == null) return Result.success(null);

    if (job.type == JobType.keepAwake) {
      await _wakelock.release();
    }
    await _notifications.cancel(jobId);
    final result = await _repository.updateStatus(jobId, JobStatus.paused);
    await _syncForegroundService();
    return result;
  }

  /// Resumes a paused job, recomputing the target for duration triggers.
  Future<Result<void>> resumeJob(int jobId, tz.Location location) async {
    // We need the job from all jobs (not just active).
    final allStream = _repository.watchAll();
    final allJobs = await allStream.first;
    final job = allJobs.where((j) => j.id == jobId).firstOrNull;
    if (job == null) return Result.failure(const StorageError(message: 'Job not found'));

    DateTime? newTarget;
    if (job.trigger case DurationTrigger(:final minutes)) {
      newTarget = DateTime.now().toUtc().add(Duration(minutes: minutes));
    } else {
      newTarget = job.targetInstantUtc;
    }

    if (newTarget != null) {
      await _repository.updateTarget(jobId, newTarget);
    }
    final statusResult = await _repository.updateStatus(jobId, JobStatus.active);
    if (statusResult.isFailure) return statusResult;

    if (job.type == JobType.keepAwake) {
      await _wakelock.acquire();
    }

    await _syncForegroundService();
    await _armNextTimer();
    return Result.success(null);
  }

  /// Cancels a job, releasing all resources.
  Future<Result<void>> cancelJob(int jobId) async {
    final activeResult = await _repository.getActiveJobs();
    final job = activeResult.valueOrNull?.where((j) => j.id == jobId).firstOrNull;
    if (job != null && job.type == JobType.keepAwake) {
      await _wakelock.release();
    }
    await _notifications.cancel(jobId);
    final result = await _repository.updateStatus(jobId, JobStatus.cancelled);
    await _syncForegroundService();
    return result;
  }

  /// Detects timezone changes and re-resolves absolute-time jobs.
  Future<void> handleTimezoneChange(tz.Location newLocation) async {
    final activeResult = await _repository.getActiveJobs();
    if (activeResult.isFailure) return;

    for (final job in activeResult.valueOrNull!) {
      if (job.trigger is! AbsoluteTimeTrigger) continue;

      final newTarget = TriggerResolver.resolve(job.trigger, newLocation);
      if (newTarget == null) continue;

      final oldTarget = job.targetInstantUtc;
      if (oldTarget != null && newTarget != oldTarget) {
        await _repository.updateTarget(job.id, newTarget);
        await _notifications.showImmediate(
          id: job.id,
          title: 'Job rescheduled',
          body: 'Your scheduled power-off time was updated due to a timezone change.',
        );
      }
    }

    await _armNextTimer();
  }

  Future<void> _scheduleNotificationBackstop(Job job) async {
    final target = job.targetInstantUtc;
    if (target == null) return;
    // Schedule a notification 60 seconds before the target (the grace-period start).
    final notifyAt = target.subtract(const Duration(seconds: 10));
    if (notifyAt.isAfter(DateTime.now().toUtc())) {
      await _notifications.scheduleZoned(
        id: job.id,
        title: 'Power-off scheduled',
        body: 'Your device will power off soon.',
        scheduledDateUtc: notifyAt,
      );
    }
  }

  String _errorMessage(AppError error) => switch (error) {
        PowerOffPrivilegeDenied() =>
          "Your organization's policy prevents this app from shutting down the PC.",
        PowerOffConsentDenied() =>
          'macOS Automation permission was denied. '
              'Go to System Settings > Privacy & Security > Automation to re-enable it.',
        PowerOffPolicyDenied() =>
          "The system's power policy blocked the shutdown request.",
        PowerOffUnsupported() =>
          'Power-off is not supported on this platform.',
        PowerOffFailed(:final message) => 'Power-off failed: $message',
        _ => error.toString(),
      };

  void dispose() {
    _nextTimer?.cancel();
  }
}

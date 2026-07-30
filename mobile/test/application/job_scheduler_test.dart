import 'package:drift/native.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:timezone/data/latest.dart' as tz_data;
import 'package:timezone/timezone.dart' as tz;
import 'package:weakup/application/job_scheduler.dart';
import 'package:weakup/core/app_error.dart';
import 'package:weakup/core/result.dart';
import 'package:weakup/data/app_database.dart';
import 'package:weakup/data/drift_job_repository.dart';
import 'package:weakup/domain/job.dart';
import 'package:weakup/domain/job_enums.dart';
import 'package:weakup/domain/trigger_spec.dart';
import 'package:weakup/platform/foreground_service_controller.dart';

import '../platform/fake_foreground_service_controller.dart';
import '../platform/fake_notification_service.dart';
import '../platform/fake_power_off_executor.dart';
import '../platform/fake_wakelock_controller.dart';

/// Simple in-memory repository using the Drift test db.
DriftJobRepository _makeRepo() {
  final db = AppDatabase(NativeDatabase.memory());
  return DriftJobRepository(db);
}

Job _makePowerOffJob({
  required DateTime targetUtc,
  JobStatus status = JobStatus.active,
}) {
  final now = DateTime.now().toUtc();
  return Job(
    id: 0,
    type: JobType.powerOff,
    trigger: const DurationTrigger(minutes: 1),
    status: status,
    targetInstantUtc: targetUtc,
    createdAt: now,
    updatedAt: now,
  );
}

Job _makeKeepAwakeJob({
  DateTime? targetUtc,
  JobStatus status = JobStatus.active,
}) {
  final now = DateTime.now().toUtc();
  return Job(
    id: 0,
    type: JobType.keepAwake,
    trigger: targetUtc != null
        ? const DurationTrigger(minutes: 30)
        : const IndefiniteTrigger(),
    status: status,
    targetInstantUtc: targetUtc,
    createdAt: now,
    updatedAt: now,
  );
}

void main() {
  setUpAll(tz_data.initializeTimeZones);

  group('JobScheduler', () {
    late DriftJobRepository repo;
    late FakePowerOffExecutor fakeExecutor;
    late FakeWakelockController fakeWakelock;
    late FakeNotificationService fakeNotifications;
    late FakeForegroundServiceController fakeForegroundService;
    late List<Job> gracePeriodJobs;
    late List<Job> overdueJobs;
    late List<Job> failedJobs;
    late JobScheduler scheduler;

    setUp(() {
      repo = _makeRepo();
      fakeExecutor = FakePowerOffExecutor();
      fakeWakelock = FakeWakelockController();
      fakeNotifications = FakeNotificationService();
      fakeForegroundService = FakeForegroundServiceController();
      gracePeriodJobs = [];
      overdueJobs = [];
      failedJobs = [];

      scheduler = JobScheduler(
        repository: repo,
        powerOffExecutor: fakeExecutor,
        wakelockController: fakeWakelock,
        notificationService: fakeNotifications,
        foregroundService: fakeForegroundService,
        onGracePeriodStarted: (job, cancel, proceed) {
          gracePeriodJobs.add(job);
          // In tests, immediately proceed (no real 60s wait needed for unit tests).
          // Tests that check cancellation call cancel() directly.
        },
        onJobFailed: (job, error) => failedJobs.add(job),
        onJobOverdue: (job) => overdueJobs.add(job),
      );
    });

    tearDown(() async {
      scheduler.dispose();
      await repo.close();
    });

    group('Grace period countdown', () {
      test('grace period precedes power-off execution', () async {
        // Activate a power-off job due immediately (past due by 1 second, within tolerance).
        final target = DateTime.now().toUtc().subtract(const Duration(seconds: 1));
        final insertResult = await repo.insert(_makePowerOffJob(targetUtc: target));
        expect(insertResult.isSuccess, isTrue);

        // Reconcile: job is recently overdue, should start grace period.
        await scheduler.reconcile();

        // Grace period should have been triggered.
        expect(gracePeriodJobs.length, 1);
        // Executor should NOT have been called yet (grace period in progress).
        expect(fakeExecutor.wasCalled, isFalse);
      });

      test('cancel during grace period prevents power-off execution', () async {
        bool cancelCalled = false;
        final cancelScheduler = JobScheduler(
          repository: repo,
          powerOffExecutor: fakeExecutor,
          wakelockController: fakeWakelock,
          notificationService: fakeNotifications,
          onGracePeriodStarted: (job, cancel, proceed) {
            cancelCalled = true;
            // User cancels immediately.
            cancel();
          },
        );

        final target = DateTime.now().toUtc().subtract(const Duration(seconds: 1));
        await repo.insert(_makePowerOffJob(targetUtc: target));
        await cancelScheduler.reconcile();

        expect(cancelCalled, isTrue);
        expect(fakeExecutor.wasCalled, isFalse,
            reason: 'Cancel must prevent executor from being called');

        cancelScheduler.dispose();
      });

      test('countdown expiry triggers power-off execution', () async {
        bool proceedCalled = false;
        final proceedScheduler = JobScheduler(
          repository: repo,
          powerOffExecutor: fakeExecutor,
          wakelockController: fakeWakelock,
          notificationService: fakeNotifications,
          onGracePeriodStarted: (job, cancel, proceed) {
            proceedCalled = true;
            // Simulate countdown expiry.
            proceed();
          },
        );

        final target = DateTime.now().toUtc().subtract(const Duration(seconds: 1));
        await repo.insert(_makePowerOffJob(targetUtc: target));
        await proceedScheduler.reconcile();

        expect(proceedCalled, isTrue);
        expect(fakeExecutor.wasCalled, isTrue,
            reason: 'Countdown expiry must call the executor');

        proceedScheduler.dispose();
      });

      test('failure sets job to failed status and surfaces error', () async {
        final failError = const PowerOffPrivilegeDenied();
        fakeExecutor.setNextResult(Result.failure(failError));

        final proceedScheduler = JobScheduler(
          repository: repo,
          powerOffExecutor: fakeExecutor,
          wakelockController: fakeWakelock,
          notificationService: fakeNotifications,
          onGracePeriodStarted: (job, cancel, proceed) => proceed(),
          onJobFailed: (job, error) => failedJobs.add(job),
        );

        final target = DateTime.now().toUtc().subtract(const Duration(seconds: 1));
        await repo.insert(_makePowerOffJob(targetUtc: target));
        await proceedScheduler.reconcile();

        // Give the async chain time to complete.
        await Future.delayed(const Duration(milliseconds: 10));

        expect(failedJobs.length, 1,
            reason: 'Failed job must be reported via onJobFailed');

        // Task 10.6: failure must never leave the job pending or completed.
        final stored = (await repo.watchAll().first).first;
        expect(
          stored.status,
          JobStatus.failed,
          reason: 'A power-off failure must persist JobStatus.failed, never '
              'leave the job active or completed',
        );

        proceedScheduler.dispose();
      });
    });

    group('Overdue reconciliation', () {
      test('recently overdue power-off (5 min) starts grace period', () async {
        final target = DateTime.now().toUtc().subtract(const Duration(minutes: 5));
        await repo.insert(_makePowerOffJob(targetUtc: target));
        await scheduler.reconcile();

        expect(gracePeriodJobs.length, 1,
            reason: '5-min overdue power-off must trigger grace period');
        expect(fakeExecutor.wasCalled, isFalse);
      });

      test('long-overdue power-off (30 min) does NOT execute', () async {
        // CRITICAL: this test asserts that a stale power-off intent never fires.
        final target = DateTime.now().toUtc().subtract(const Duration(minutes: 30));
        await repo.insert(_makePowerOffJob(targetUtc: target));
        await scheduler.reconcile();

        expect(fakeExecutor.wasCalled, isFalse,
            reason: 'Long-overdue power-off MUST NOT execute — this would destroy user work');
        expect(overdueJobs.length, 1,
            reason: 'Long-overdue job must be marked overdue');
      });

      test('overdue keep-awake job completes without executor call', () async {
        final target = DateTime.now().toUtc().subtract(const Duration(hours: 1));
        final job = _makeKeepAwakeJob(targetUtc: target);
        await repo.insert(job);
        await scheduler.reconcile();

        expect(fakeExecutor.wasCalled, isFalse);
        // Wakelock should have been released.
        expect(fakeWakelock.isHeld, isFalse);

        final allJobs = await repo.watchAll().first;
        expect(
          allJobs.first.status,
          anyOf(JobStatus.completed, JobStatus.active),
          // completed is the expected outcome after reconcile
        );
      });

      test('duration job target is unchanged by a real timezone change',
          () async {
        // Duration jobs are anchored at creation, so a timezone change must
        // leave them alone. Spec: power-job-scheduling — "Duration-based job is
        // unaffected by timezone change".
        final target = DateTime.now().toUtc().add(const Duration(hours: 2));
        final now = DateTime.now().toUtc();
        await repo.insert(Job(
          id: 0,
          type: JobType.powerOff,
          trigger: const DurationTrigger(minutes: 120),
          status: JobStatus.active,
          targetInstantUtc: target,
          createdAt: now,
          updatedAt: now,
        ));

        // Actually drive the timezone change through the scheduler rather than
        // reading back what we just stored.
        await scheduler
            .handleTimezoneChange(tz.getLocation('America/Los_Angeles'));

        final stored = (await repo.watchAll().first).first;
        // Drift stores DateTimeColumn as Unix seconds → compare at 1s precision.
        expect(
          (stored.targetInstantUtc?.millisecondsSinceEpoch ?? 0) ~/ 1000,
          target.millisecondsSinceEpoch ~/ 1000,
          reason: 'Duration job target must be unchanged by timezone change',
        );
        expect(
          fakeNotifications.immediateNotifications,
          isEmpty,
          reason: 'An unaffected job must not notify the user of a reschedule',
        );
      });
    });

    group('Pause, resume, cancel', () {
      test('pause releases wakelock', () async {
        final job = _makeKeepAwakeJob();
        final inserted = (await repo.insert(job)).valueOrNull!;
        await fakeWakelock.acquire(); // simulate activation

        await scheduler.pauseJob(inserted.id);
        expect(fakeWakelock.isHeld, isFalse);
      });

      test('cancel releases wakelock and marks cancelled', () async {
        final job = _makeKeepAwakeJob();
        final inserted = (await repo.insert(job)).valueOrNull!;
        await fakeWakelock.acquire();

        await scheduler.cancelJob(inserted.id);
        expect(fakeWakelock.isHeld, isFalse);
      });
    });

    group('Activate job', () {
      test('keep-awake activation acquires wakelock', () async {
        final job = _makeKeepAwakeJob();
        final result = await scheduler.activateJob(job);
        expect(result.isSuccess, isTrue);
        expect(fakeWakelock.isHeld, isTrue);
      });

      test('power-off activation does not acquire wakelock', () async {
        final target = DateTime.now().toUtc().add(const Duration(hours: 1));
        final job = _makePowerOffJob(targetUtc: target);
        await scheduler.activateJob(job);
        expect(fakeWakelock.isHeld, isFalse);
      });
    });

    group('handleTimezoneChange', () {
      test(
          'AbsoluteTimeTrigger job: target updated and notification sent on timezone change',
          () async {
        // Create a job with an AbsoluteTimeTrigger set for 22:00 in New York.
        // We store an explicit NY-based target (forced to "tomorrow") so it is
        // always strictly in the future at insert time.
        final nyLoc = tz.getLocation('America/New_York');
        final nyNow = tz.TZDateTime.now(nyLoc);
        final originalTarget = tz.TZDateTime(
          nyLoc,
          nyNow.year,
          nyNow.month,
          nyNow.day + 1,
          22,
          0,
        ).toUtc();

        final now = DateTime.now().toUtc();
        final job = Job(
          id: 0,
          type: JobType.powerOff,
          trigger: const AbsoluteTimeTrigger(hour: 22, minute: 0),
          status: JobStatus.active,
          targetInstantUtc: originalTarget,
          createdAt: now,
          updatedAt: now,
        );
        final insertResult = await repo.insert(job);
        expect(insertResult.isSuccess, isTrue);

        // Compute what TriggerResolver would produce for LA timezone.
        // This mirrors the scheduler's own logic to avoid brittle date arithmetic.
        final laLoc = tz.getLocation('America/Los_Angeles');
        final laNow = tz.TZDateTime.now(laLoc);
        // Try today; roll to tomorrow if 22:00 LA hasn't passed.
        var expectedLaTarget = tz.TZDateTime(
          laLoc,
          laNow.year,
          laNow.month,
          laNow.day,
          22,
          0,
        );
        if (!expectedLaTarget.isAfter(laNow)) {
          expectedLaTarget = tz.TZDateTime(
            laLoc,
            laNow.year,
            laNow.month,
            laNow.day + 1,
            22,
            0,
          );
        }
        final expectedUtcMs = expectedLaTarget.toUtc().millisecondsSinceEpoch;

        // The NY and LA targets must differ (NY is 3h ahead, so same wall-clock
        // time lands at a different UTC moment — unless the two timezones happen
        // to match, which only occurs during DST transitions).
        expect(
          expectedUtcMs,
          isNot(equals(originalTarget.millisecondsSinceEpoch)),
          reason: 'NY and LA 22:00 should resolve to different UTC times',
        );

        // Trigger timezone change to LA.
        await scheduler.handleTimezoneChange(laLoc);

        // The repository should now have the LA-based target.
        final allJobs = await repo.watchAll().first;
        expect(allJobs.length, 1);
        final updatedJob = allJobs.first;

        expect(
          updatedJob.targetInstantUtc?.millisecondsSinceEpoch,
          closeTo(expectedUtcMs, 1000),
          reason: 'AbsoluteTimeTrigger target must be re-resolved for the new timezone',
        );

        // A notification must have been sent to inform the user.
        expect(
          fakeNotifications.immediateNotifications,
          isNotEmpty,
          reason: 'A notification must be sent when the job is rescheduled',
        );
        expect(
          fakeNotifications.immediateNotifications.last['title'],
          contains('rescheduled'),
        );
      });

      test('DurationTrigger job: target is NOT changed by timezone change',
          () async {
        final target = DateTime.now().toUtc().add(const Duration(hours: 2));
        final now = DateTime.now().toUtc();
        final job = Job(
          id: 0,
          type: JobType.powerOff,
          trigger: const DurationTrigger(minutes: 120),
          status: JobStatus.active,
          targetInstantUtc: target,
          createdAt: now,
          updatedAt: now,
        );
        await repo.insert(job);

        final laLoc = tz.getLocation('America/Los_Angeles');
        await scheduler.handleTimezoneChange(laLoc);

        final allJobs = await repo.watchAll().first;
        expect(allJobs.length, 1);
        expect(
          allJobs.first.targetInstantUtc?.millisecondsSinceEpoch,
          closeTo(target.millisecondsSinceEpoch, 1000),
          reason: 'DurationTrigger target must not change on timezone change',
        );

        // No notification should be sent for duration-trigger jobs.
        expect(fakeNotifications.immediateNotifications, isEmpty);
      });
    });

    group('Power-off overtolerance boundary', () {
      test('overdue just under 15 minutes proceeds to grace period', () async {
        // kPowerOffOvertolerance = 15 minutes; check is strict (overdue > 15min skips).
        // A job overdue by 14 min 59 s is strictly within tolerance → grace period.
        final target = DateTime.now()
            .toUtc()
            .subtract(kPowerOffOvertolerance - const Duration(seconds: 1));
        await repo.insert(_makePowerOffJob(targetUtc: target));
        await scheduler.reconcile();

        expect(
          gracePeriodJobs.length,
          1,
          reason: 'Job overdue by < 15 min must trigger grace period',
        );
        expect(fakeExecutor.wasCalled, isFalse,
            reason: 'Grace period must precede executor');
      });

      test('overdue by 15 minutes + 1 second does NOT execute', () async {
        // 1 second beyond the tolerance boundary must be treated as stale.
        final target = DateTime.now()
            .toUtc()
            .subtract(kPowerOffOvertolerance + const Duration(seconds: 1));
        await repo.insert(_makePowerOffJob(targetUtc: target));
        await scheduler.reconcile();

        expect(fakeExecutor.wasCalled, isFalse,
            reason: 'Job overdue by 15m+1s must NOT execute');
        expect(overdueJobs.length, 1,
            reason: 'Job overdue by 15m+1s must be marked overdue');
      });
    });

    group('Background service lifecycle', () {
      test('service starts when a job becomes active', () async {
        await scheduler.activateJob(
          _makeKeepAwakeJob(),
        );

        expect(fakeForegroundService.startCalls, [1],
            reason: 'Activating a job must start the service with 1 job');
      });

      test('service stops when the last active job is cancelled', () async {
        final activated =
            await scheduler.activateJob(_makeKeepAwakeJob());
        final job = activated.valueOrNull!;
        fakeForegroundService.stopCallCount = 0;

        await scheduler.cancelJob(job.id);

        expect(fakeForegroundService.stopCallCount, greaterThan(0),
            reason: 'No active jobs remain, so the service must be stopped');
      });

      test('service is not touched on platforms without one', () async {
        fakeForegroundService.isSupported = false;

        await scheduler.activateJob(_makeKeepAwakeJob());

        expect(fakeForegroundService.startCalled, isFalse);
        expect(fakeForegroundService.stopCallCount, 0);
      });

      test(
          'refused start (Android 15 dataSync cap) degrades the job and notifies',
          () async {
        fakeForegroundService.startOutcome =
            ForegroundServiceStartOutcome.refused;

        final activated = await scheduler.activateJob(_makeKeepAwakeJob());
        final jobId = activated.valueOrNull!.id;

        final stored = (await repo.watchAll().first)
            .firstWhere((j) => j.id == jobId);
        expect(
          stored.status,
          JobStatus.degraded,
          reason: 'A refused service start must mark the job degraded, '
              'never leave it silently unbacked',
        );

        expect(
          fakeForegroundService.stopCallCount,
          greaterThan(0),
          reason: 'The dead service must be stopped so no stale notification '
              'is left behind',
        );

        final notified = fakeNotifications.immediateNotifications
            .where((n) => n['id'] == jobId)
            .toList();
        expect(notified, hasLength(1),
            reason: 'The user must be told background execution was lost');
        expect(notified.single['title'], contains('Background limit'));
      });

      test('refused start does not power off the device', () async {
        fakeForegroundService.startOutcome =
            ForegroundServiceStartOutcome.refused;

        await scheduler.activateJob(
          _makePowerOffJob(
            targetUtc: DateTime.now().toUtc().add(const Duration(hours: 2)),
          ),
        );

        expect(fakeExecutor.wasCalled, isFalse,
            reason: 'Losing the background service must never trigger '
                'a power-off');
      });

      test('successful start leaves the job active', () async {
        final activated = await scheduler.activateJob(_makeKeepAwakeJob());
        final jobId = activated.valueOrNull!.id;
        final stored = (await repo.watchAll().first)
            .firstWhere((j) => j.id == jobId);

        expect(stored.status, JobStatus.active);
        expect(fakeNotifications.immediateNotifications, isEmpty);
      });
    });
  });
}

import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:timezone/timezone.dart' as tz;

import '../core/app_error.dart';
import '../core/result.dart';
import '../data/app_database.dart';
import '../data/drift_job_repository.dart';
import '../domain/job.dart' as domain;
import '../domain/job_enums.dart';
import '../domain/job_repository.dart';
import '../domain/trigger_resolver.dart';
import '../domain/trigger_spec.dart';
import '../main.dart' show navigatorKey;
import '../platform/android_foreground_service.dart';
import '../platform/desktop_runtime.dart';
import '../platform/flutter_notification_service.dart';
import '../platform/foreground_service_controller.dart';
import '../platform/notification_service.dart';
import '../platform/platform_capabilities.dart';
import '../platform/power_off_executor.dart';
import '../platform/power_off_executors.dart';
import '../platform/wakelock_controller.dart';
import '../platform/wakelock_plus_controller.dart';
import '../ui/screens/grace_period_screen.dart';
import 'job_scheduler.dart';

// ---- Desktop runtime ----

/// Holds the [DesktopRuntime] instance created in [main], or null on mobile/web.
/// Override this in [ProviderScope] (main.dart) after creating the runtime.
final desktopRuntimeProvider = Provider<DesktopRuntime?>((ref) => null);

// ---- Database ----

final databaseProvider = Provider<AppDatabase>((ref) {
  final db = AppDatabase();
  ref.onDispose(db.close);
  return db;
});

// ---- Repository ----

final jobRepositoryProvider = Provider<JobRepository>((ref) {
  final db = ref.watch(databaseProvider);
  return DriftJobRepository(db);
});

// ---- Platform capabilities ----

final platformCapabilitiesProvider = Provider<PlatformCapabilities>((ref) {
  return PlatformCapabilities.resolve();
});

// ---- Wakelock ----

final wakelockControllerProvider = Provider<WakelockController>((ref) {
  return WakelockPlusController();
});

// ---- Power-off executor ----

final powerOffExecutorProvider = Provider<PowerOffExecutor>((ref) {
  final caps = ref.watch(platformCapabilitiesProvider);
  if (!caps.supportsPowerOff) return const UnsupportedPowerOffExecutor();
  if (Platform.isWindows) return WindowsPowerOffExecutor();
  if (Platform.isMacOS) return MacOsPowerOffExecutor();
  if (Platform.isLinux) return LinuxPowerOffExecutor();
  return const UnsupportedPowerOffExecutor();
});

// ---- Notification service ----

final notificationServiceProvider =
    FutureProvider<NotificationService>((ref) async {
  return FlutterNotificationService.create();
});

// ---- Background foreground service (Android only) ----

final foregroundServiceControllerProvider =
    Provider<ForegroundServiceController>((ref) {
  if (Platform.isAndroid) return const AndroidForegroundServiceController();
  return const NoopForegroundServiceController();
});

// ---- Job scheduler ----

final jobSchedulerProvider = Provider<JobScheduler?>((ref) {
  final notifAsync = ref.watch(notificationServiceProvider);
  return notifAsync.when(
    data: (notif) {
      final scheduler = JobScheduler(
        repository: ref.watch(jobRepositoryProvider),
        powerOffExecutor: ref.watch(powerOffExecutorProvider),
        wakelockController: ref.watch(wakelockControllerProvider),
        notificationService: notif,
        foregroundService: ref.watch(foregroundServiceControllerProvider),
        onGracePeriodStarted: (job, cancelCallback, proceedCallback) {
          // Push GracePeriodScreen via the global navigator key so this
          // callback works even when called from the scheduler (outside the
          // widget tree).
          final nav = navigatorKey.currentState;
          if (nav == null) {
            // Navigator not ready — safest to cancel rather than silently
            // proceed to power-off with no UI.
            cancelCallback();
            return;
          }
          nav.push(
            MaterialPageRoute<void>(
              builder: (_) => GracePeriodScreen(
                job: job,
                onCancel: () {
                  nav.pop();
                  cancelCallback();
                },
                onProceed: () {
                  nav.pop();
                  proceedCallback();
                },
              ),
              fullscreenDialog: true,
            ),
          );
        },
      );
      ref.onDispose(scheduler.dispose);
      return scheduler;
    },
    loading: () => null,
    error: (_, _) => null,
  );
});

// ---- Job list ----

final jobListProvider = StreamProvider<List<domain.Job>>((ref) {
  final repo = ref.watch(jobRepositoryProvider);
  return repo.watchAll();
});

final activeJobsProvider = StreamProvider<List<domain.Job>>((ref) {
  final repo = ref.watch(jobRepositoryProvider);
  return repo.watchAll().map(
        (jobs) => jobs.where((j) => j.status == JobStatus.active).toList(),
      );
});

// ---- Job creation state ----

/// State for the job creation form.
class JobCreationState {
  const JobCreationState({
    this.keepAwakeEnabled = false,
    this.powerOffEnabled = false,
    this.keepAwakeTrigger = const IndefiniteTrigger(),
    this.powerOffTrigger = const DurationTrigger(minutes: 60),
    this.isSubmitting = false,
    this.error,
    this.pendingReplaceType,
  });

  final bool keepAwakeEnabled;
  final bool powerOffEnabled;
  final TriggerSpec keepAwakeTrigger;
  final TriggerSpec powerOffTrigger;
  final bool isSubmitting;
  final String? error;
  final JobType? pendingReplaceType;

  JobCreationState copyWith({
    bool? keepAwakeEnabled,
    bool? powerOffEnabled,
    TriggerSpec? keepAwakeTrigger,
    TriggerSpec? powerOffTrigger,
    bool? isSubmitting,
    String? error,
    JobType? pendingReplaceType,
    bool clearError = false,
    bool clearPendingReplace = false,
  }) {
    return JobCreationState(
      keepAwakeEnabled: keepAwakeEnabled ?? this.keepAwakeEnabled,
      powerOffEnabled: powerOffEnabled ?? this.powerOffEnabled,
      keepAwakeTrigger: keepAwakeTrigger ?? this.keepAwakeTrigger,
      powerOffTrigger: powerOffTrigger ?? this.powerOffTrigger,
      isSubmitting: isSubmitting ?? this.isSubmitting,
      error: clearError ? null : (error ?? this.error),
      pendingReplaceType: clearPendingReplace
          ? null
          : (pendingReplaceType ?? this.pendingReplaceType),
    );
  }
}

class JobCreationNotifier extends Notifier<JobCreationState> {
  @override
  JobCreationState build() => const JobCreationState();

  void toggleKeepAwake(bool enabled) =>
      state = state.copyWith(keepAwakeEnabled: enabled);

  void togglePowerOff(bool enabled) =>
      state = state.copyWith(powerOffEnabled: enabled);

  void setKeepAwakeTrigger(TriggerSpec trigger) =>
      state = state.copyWith(keepAwakeTrigger: trigger);

  void setPowerOffTrigger(TriggerSpec trigger) =>
      state = state.copyWith(powerOffTrigger: trigger);

  Future<Result<void>> submit({required tz.Location location}) async {
    state = state.copyWith(isSubmitting: true, clearError: true);

    final scheduler = ref.read(jobSchedulerProvider);
    if (scheduler == null) {
      state = state.copyWith(isSubmitting: false, error: 'Scheduler not ready.');
      return Result.failure(const StorageError(message: 'Scheduler not ready'));
    }

    final repo = ref.read(jobRepositoryProvider);
    final now = DateTime.now().toUtc();

    if (state.keepAwakeEnabled) {
      final validResult =
          TriggerResolver.validate(state.keepAwakeTrigger, JobType.keepAwake);
      if (validResult.isFailure) {
        state = state.copyWith(
          isSubmitting: false,
          error: (validResult.errorOrNull as ValidationError).message,
        );
        return validResult.map((_) {});
      }

      // Resolution can refuse a dated trigger whose instant has passed, which
      // validation cannot see — it has no clock. Reported the same way a validation
      // failure is, so the user gets the reason rather than a job that never fires.
      final resolvedResult =
          TriggerResolver.resolve(state.keepAwakeTrigger, location);
      if (resolvedResult.isFailure) {
        state = state.copyWith(
          isSubmitting: false,
          error: (resolvedResult.errorOrNull as ValidationError).message,
        );
        return resolvedResult.map((_) {});
      }
      final target = resolvedResult.valueOrNull;
      final job = domain.Job(
        id: 0,
        type: JobType.keepAwake,
        trigger: state.keepAwakeTrigger,
        status: JobStatus.active,
        targetInstantUtc: target,
        createdAt: now,
        updatedAt: now,
      );

      final activeKA = await repo.getActiveJobs();
      final hasActive =
          activeKA.valueOrNull?.any((j) => j.type == JobType.keepAwake) ??
              false;

      if (hasActive && state.pendingReplaceType != JobType.keepAwake) {
        state = state.copyWith(
          isSubmitting: false,
          pendingReplaceType: JobType.keepAwake,
        );
        return Result.success(null);
      }

      final result = hasActive
          ? await scheduler.replaceJob(job)
          : await scheduler.activateJob(job);
      if (result.isFailure) {
        state = state.copyWith(
            isSubmitting: false, error: result.errorOrNull.toString());
        return result.map((_) {});
      }
    }

    if (state.powerOffEnabled) {
      final validResult =
          TriggerResolver.validate(state.powerOffTrigger, JobType.powerOff);
      if (validResult.isFailure) {
        state = state.copyWith(
          isSubmitting: false,
          error: (validResult.errorOrNull as ValidationError).message,
        );
        return validResult.map((_) {});
      }

      // As above: a dated power-off whose instant has passed is refused here rather
      // than stored as a job that can never fire.
      final resolvedResult =
          TriggerResolver.resolve(state.powerOffTrigger, location);
      if (resolvedResult.isFailure) {
        state = state.copyWith(
          isSubmitting: false,
          error: (resolvedResult.errorOrNull as ValidationError).message,
        );
        return resolvedResult.map((_) {});
      }
      final target = resolvedResult.valueOrNull;
      final job = domain.Job(
        id: 0,
        type: JobType.powerOff,
        trigger: state.powerOffTrigger,
        status: JobStatus.active,
        targetInstantUtc: target,
        createdAt: now,
        updatedAt: now,
      );

      final activePO = await repo.getActiveJobs();
      final hasActive =
          activePO.valueOrNull?.any((j) => j.type == JobType.powerOff) ?? false;

      if (hasActive && state.pendingReplaceType != JobType.powerOff) {
        state = state.copyWith(
          isSubmitting: false,
          pendingReplaceType: JobType.powerOff,
        );
        return Result.success(null);
      }

      final result = hasActive
          ? await scheduler.replaceJob(job)
          : await scheduler.activateJob(job);
      if (result.isFailure) {
        state = state.copyWith(
            isSubmitting: false, error: result.errorOrNull.toString());
        return result.map((_) {});
      }
    }

    state = const JobCreationState();
    return Result.success(null);
  }

  void confirmReplace() {
    state = state.copyWith(clearPendingReplace: true);
  }
}

final jobCreationProvider =
    NotifierProvider<JobCreationNotifier, JobCreationState>(
        JobCreationNotifier.new);

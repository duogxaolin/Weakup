import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:timezone/timezone.dart' as tz;

import '../core/app_error.dart';
import '../core/result.dart';
import '../data/app_database.dart';
import '../data/drift_job_repository.dart';
import '../data/pairing_store.dart';
import '../domain/device_id.dart';
import '../domain/device_identity.dart';
import '../domain/job.dart' as domain;
import '../domain/job_enums.dart';
import '../domain/job_repository.dart';
import '../domain/signature.dart';
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
import '../platform/secret_store.dart';
import '../platform/wakelock_controller.dart';
import '../platform/wakelock_plus_controller.dart';
import '../ui/screens/grace_period_screen.dart';
import '../ui/screens/paired_devices_screen.dart';
import 'job_scheduler.dart';
import 'pairing_flow.dart';

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

// ---- Pairing and remote control (task 7.4) ----

/*
 * The remote surface's providers.
 *
 * Note what is absent: nothing here holds a `PowerOffExecutor`. A remote *request* creates a
 * signed envelope; whether a machine acts on it is the target's decision, and the countdown
 * the target then runs is not shortenable from this side. The one executor on this device is
 * held by `JobScheduler` for this device's own jobs.
 *
 * Remote control defaults to disabled here too, for the same reason it does on the desktop: a
 * device nobody has configured must not be commandable. The mobile app is not currently a
 * command *target* — it has no arriving-command loop — so the setting is read but not yet
 * enforced anywhere on this side. Stated rather than implied, because a setting that looks
 * enforced and is not is worse than one that is plainly absent.
 */

/// Where this device's pairings and identity record live.
final pairingStoreProvider = Provider<PairingStore>((ref) {
  return PairingStore(ref.watch(databaseProvider));
});

/// The credential store the signing key is kept in.
///
/// Never the database and never a file: the key is what makes a command from this device
/// genuine, and a copy of it on disk beside the pairings would make one leak enough for both.
final secretStoreProvider = Provider<SecretStore>((ref) {
  return FlutterSecureStorageSecretStore();
});

/// This device's signing identity, generated on first use and reused thereafter.
///
/// A failure is **not** recovered by generating a replacement. A new identity has a different
/// [DeviceId], so every peer that paired with this device would keep trusting a key it no
/// longer holds — its commands refused as coming from an unknown sender, with every pairing
/// needing to be redone by hand at each peer. `loadOrGenerateIdentity` enforces that; this
/// provider surfaces the error.
final deviceIdentityProvider = FutureProvider<Result<DeviceIdentity>>((ref) async {
  return loadOrGenerateIdentity(
    records: ref.watch(pairingStoreProvider),
    secrets: ref.watch(secretStoreProvider),
  );
});

/// Holds the outstanding pairing codes this device has issued.
///
/// One per app lifetime, and swept: `forgetStale` is called before each use, so the map does
/// not grow for the life of the process. See `PairingCodeIssuer.forgetStale` for why a spent
/// code is kept for a while rather than dropped the instant it is used.
final pairingCodeIssuerProvider = Provider<PairingCodeIssuer>((ref) {
  return PairingCodeIssuer();
});

/// The peers this device is paired with, revoked ones included.
///
/// Revoked pairings are returned rather than filtered. A row that vanished on revocation would
/// make "was this device ever paired?" unanswerable — the question that matters most after a
/// device is lost. The screen shows them greyed and offers no controls on them.
final pairedPeersProvider = FutureProvider<List<PairedPeer>>((ref) async {
  final store = ref.watch(pairingStoreProvider);
  final pairings = await store.listPairings();

  final records = pairings.valueOrNull ?? const <PairingRecord>[];

  // `lastReportedAt` is null for every peer until a transport is configured, which the
  // presence rule turns into `offline` — the honest answer for a device this app has no way
  // to hear from. Deliberately not defaulted to `now`, which would show every peer as online.
  return records
      .map(
        (record) => PairedPeer(
          deviceId: record.peer.value,
          displayName: record.peer.value,
          lastReportedAt: null,
          revoked: record.revoked,
        ),
      )
      .toList();
});

/// Runs the pairing exchange for a code the user typed into this device.
///
/// # Ordering, and why it is fixed
///
/// Design D7: the issuing side records the requester and only then produces its own identity,
/// and this side records the issuer only on receipt of that identity. If this side's write then
/// fails, the issuer's record is withdrawn before the failure is reported. A half-recorded
/// pairing is the worst outcome available — this device would believe it is authorized, send
/// commands, and see every one refused for a reason visible nowhere in either interface.
///
/// The exchange is run in-process here, which is the honest shape for "the peer's details were
/// typed in by hand": the peer's identity arrives with the request rather than over a channel.
/// `undo` is still supplied rather than skipped, because in the two-device case it is a real
/// round trip that can itself fail, and a caller that omits it loses the guarantee silently.
///
/// Expiry, single use, the alphabet, and normalisation are **not** decided here. They live in
/// `PairingCode.parse` and `evaluateGrant`, which the shared vectors pin.
Future<PairingOutcome> runPairingExchange({
  required WidgetRef ref,
  required String enteredCode,
  required String peerDeviceId,
  required String peerVerifyingKeyHex,
}) async {
  final parsed = PairingCode.parse(enteredCode);
  if (parsed.errorOrNull case final error?) {
    // Surfaced as a thrown message rather than a `RefusedOutcome`. The outcome type carries a
    // `GrantRejection`, which is the domain's verdict on a code it *recognised* — and a code
    // that is not even the right shape never reached the grant rule. Reporting it as
    // `noSuchGrant` would tell the user "that code was not issued here" when what happened is
    // "that is not a code", and they are two different things to act on.
    throw StateError('$error');
  }

  final identityResult = await ref.read(deviceIdentityProvider.future);
  if (identityResult.errorOrNull case final error?) {
    throw StateError('this device has no usable identity: $error');
  }
  final identity = identityResult.valueOrNull!;

  final keyBytes = decodeHexBytes(peerVerifyingKeyHex);
  if (keyBytes == null) {
    throw StateError(
      "that device's key is not in the expected form; nothing was paired",
    );
  }
  final peerKey = VerifyingKey.fromBytes(keyBytes);
  if (peerKey == null) {
    throw StateError(
      'that device presented a key this build cannot accept; nothing was paired',
    );
  }

  final store = ref.read(pairingStoreProvider);
  final issuer = ref.read(pairingCodeIssuerProvider);
  final now = DateTime.now().toUtc();

  // Swept before use rather than on a timer: the issuer has no clock, and `forgetStale` takes
  // `now` precisely so the decision about when belongs to a caller that has one. Without this
  // the outstanding map grows for the life of the process.
  issuer.forgetStale(now);

  final peer = PairingIdentity(
    deviceId: DeviceId(peerDeviceId),
    verifyingKey: peerKey,
  );
  final own = PairingIdentity(
    deviceId: identity.deviceId,
    verifyingKey: identity.verifyingKey,
  );

  final response = await acceptPairingAtIssuer(
    issuer: issuer,
    store: store,
    ownIdentity: own,
    requester: peer,
    code: parsed.valueOrNull!,
    now: now,
  );
  if (response.errorOrNull case final error?) {
    throw StateError('$error');
  }

  final outcome = await completePairingAtRequester(
    store: store,
    response: response.valueOrNull!,
    now: now,
    // Withdraws the record the issuing side made, so neither side is left believing in a
    // pairing the other has no record of.
    undo: () => withdrawPairing(store: store, peer: peer.deviceId, at: now),
  );

  if (outcome.errorOrNull case final error?) {
    throw StateError('$error');
  }
  return outcome.valueOrNull!;
}

/// Decodes lowercase or uppercase hex, or null for anything malformed.
///
/// Strict about everything: an odd length, a non-hex digit, and an empty string are all null. A
/// permissive decoder that skipped a bad digit would produce *a* key — just not the peer's — and
/// the pairing would record something the peer cannot sign as.
///
/// Length is not checked against the key size here; `VerifyingKey.fromBytes` is the one place
/// that decides what a valid key is, and a second rule here could disagree with it.
List<int>? decodeHexBytes(String text) {
  if (text.isEmpty || text.length % 2 != 0) return null;

  final out = <int>[];
  for (var i = 0; i < text.length; i += 2) {
    final byte = int.tryParse(text.substring(i, i + 2), radix: 16);
    if (byte == null) return null;
    out.add(byte);
  }
  return out;
}

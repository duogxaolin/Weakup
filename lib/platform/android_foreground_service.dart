import 'dart:io';

import 'package:flutter_foreground_task/flutter_foreground_task.dart';

import 'foreground_service_controller.dart';

/// Stable non-zero notification ID for the background service.
const int kForegroundServiceNotificationId = 9901;

/// Android implementation of [ForegroundServiceController], backed by
/// `flutter_foreground_task`.
///
/// [init] must be called once at app startup (before [start]) so the plugin
/// has its notification channel and task options registered.
final class AndroidForegroundServiceController
    implements ForegroundServiceController {
  const AndroidForegroundServiceController();

  /// One-time plugin initialization. Must be called before any [start] call.
  static void init() {
    if (!Platform.isAndroid) return;

    FlutterForegroundTask.init(
      androidNotificationOptions: AndroidNotificationOptions(
        channelId: 'weakup_fg_service',
        channelName: 'Weakup background service',
        channelDescription:
            'Keeps active power-schedule jobs running in the background.',
        channelImportance: NotificationChannelImportance.LOW,
        priority: NotificationPriority.LOW,
        onlyAlertOnce: true,
      ),
      iosNotificationOptions: const IOSNotificationOptions(
        showNotification: false,
        playSound: false,
      ),
      foregroundTaskOptions: ForegroundTaskOptions(
        // No repeat event needed — the Dart-side timer handles scheduling.
        eventAction: ForegroundTaskEventAction.nothing(),
        allowWakeLock: true,
        // Do NOT auto-restart on boot — the user must reopen the app; the
        // reconcile() call on resume handles re-arming.
        autoRunOnBoot: false,
        // Allow auto-restart after being killed by the system so active jobs
        // survive transient system pressure (outside the Android 15 cap window).
        allowAutoRestart: true,
      ),
    );
  }

  @override
  bool get isSupported => Platform.isAndroid;

  @override
  Future<ForegroundServiceStartOutcome> start({
    required int activeJobCount,
  }) async {
    if (!Platform.isAndroid) {
      return ForegroundServiceStartOutcome.notApplicable;
    }

    final text = _notificationText(activeJobCount);

    try {
      if (await FlutterForegroundTask.isRunningService) {
        // Already running — just refresh the notification text. A failure here
        // is cosmetic; the service itself is still alive.
        await FlutterForegroundTask.updateService(
          notificationTitle: 'Weakup',
          notificationText: text,
        );
        return ForegroundServiceStartOutcome.running;
      }

      final result = await FlutterForegroundTask.startService(
        serviceId: kForegroundServiceNotificationId,
        // dataSync matches android:foregroundServiceType="dataSync" in the
        // manifest. A mismatch is a hard crash on API 34+.
        serviceTypes: [ForegroundServiceTypes.dataSync],
        notificationTitle: 'Weakup',
        notificationText: text,
      );

      // startService() converts every internal throw into a
      // ServiceRequestFailure, so a failure covers the Android 15 dataSync cap
      // (ForegroundServiceStartNotAllowedException surfacing through the
      // platform channel), a start timeout, and permission denial alike.
      // We deliberately do not try to distinguish them: the Dart layer has no
      // reliable signal, and every one of them means the same thing to the
      // user — background execution is gone and the job can no longer be
      // trusted to fire on time.
      return switch (result) {
        ServiceRequestSuccess() => ForegroundServiceStartOutcome.running,
        ServiceRequestFailure() => ForegroundServiceStartOutcome.refused,
      };
    } catch (_) {
      // isRunningService / updateService can throw on platform-channel errors.
      return ForegroundServiceStartOutcome.refused;
    }
  }

  @override
  Future<void> stop() async {
    if (!Platform.isAndroid) return;
    try {
      if (await FlutterForegroundTask.isRunningService) {
        await FlutterForegroundTask.stopService();
      }
    } catch (_) {
      // Nothing useful to do — the service is either already gone or the
      // channel is unavailable. Both are acceptable outcomes for "stop".
    }
  }

  String _notificationText(int count) =>
      '$count active job${count == 1 ? '' : 's'}';
}

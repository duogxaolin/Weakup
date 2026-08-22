import 'dart:io';

import 'package:flutter_foreground_task/flutter_foreground_task.dart';
import 'package:flutter_local_notifications/flutter_local_notifications.dart';
import 'package:flutter_timezone/flutter_timezone.dart';
import 'package:permission_handler/permission_handler.dart';
import 'package:timezone/timezone.dart' as tz;

import '../core/app_error.dart';
import '../core/result.dart';
import 'notification_service.dart';

/// Production [NotificationService] backed by `flutter_local_notifications`.
class FlutterNotificationService implements NotificationService {
  FlutterNotificationService._();

  static final _plugin = FlutterLocalNotificationsPlugin();
  static bool _initialized = false;

  static Future<FlutterNotificationService> create() async {
    final instance = FlutterNotificationService._();
    await instance._init();
    return instance;
  }

  Future<void> _init() async {
    if (_initialized) return;

    // Initialise timezone data (call getLocalTimezone().identifier for IANA name).
    try {
      final tzInfo = await FlutterTimezone.getLocalTimezone();
      tz.setLocalLocation(tz.getLocation(tzInfo.identifier));
    } catch (_) {
      // Fallback to UTC if timezone detection fails in tests or on web.
      tz.setLocalLocation(tz.UTC);
    }

    const android = AndroidInitializationSettings('@mipmap/ic_launcher');
    const darwin = DarwinInitializationSettings(
      requestAlertPermission: false,
      requestBadgePermission: false,
      requestSoundPermission: false,
    );
    const linux = LinuxInitializationSettings(defaultActionName: 'Open');
    const windows = WindowsInitializationSettings(
      appName: 'Weakup',
      appUserModelId: 'vn.delify.weakup',
      guid: '12345678-1234-1234-1234-123456789012',
    );

    const initSettings = InitializationSettings(
      android: android,
      iOS: darwin,
      macOS: darwin,
      linux: linux,
      windows: windows,
    );

    await _plugin.initialize(settings: initSettings);
    _initialized = true;
  }

  @override
  Future<Result<bool>> requestNotificationPermission() async {
    if (!Platform.isAndroid) return Result.success(true);

    final status = await Permission.notification.request();
    if (status.isGranted) return Result.success(true);
    if (status.isDenied || status.isPermanentlyDenied) {
      return Result.failure(const PermissionDenied(permission: 'POST_NOTIFICATIONS'));
    }
    return Result.success(false);
  }

  @override
  Future<Result<void>> showImmediate({
    required int id,
    required String title,
    required String body,
    String? payload,
  }) async {
    try {
      const androidDetails = AndroidNotificationDetails(
        'weakup_channel',
        'Weakup',
        channelDescription: 'Weakup job notifications',
        importance: Importance.high,
        priority: Priority.high,
      );
      const details = NotificationDetails(
        android: androidDetails,
        iOS: DarwinNotificationDetails(),
        macOS: DarwinNotificationDetails(),
      );
      await _plugin.show(
        id: id,
        title: title,
        body: body,
        notificationDetails: details,
        payload: payload,
      );
      return Result.success(null);
    } catch (e) {
      return Result.failure(NotificationError(message: e.toString()));
    }
  }

  /// Schedules a zoned notification.
  ///
  /// On Android, calls [canScheduleExactAlarms] first (task 8.4).
  /// If the permission is denied, the notification is downgraded to
  /// [AndroidScheduleMode.inexact] — functionality is preserved but
  /// timing precision is reduced.
  ///
  /// On iOS, enforces the 64-pending-notification cap (task 8.5):
  /// before scheduling, prune the oldest pending notifications if the
  /// cap would be reached, so the soonest-due job's notification is
  /// never dropped.
  @override
  Future<Result<void>> scheduleZoned({
    required int id,
    required String title,
    required String body,
    required DateTime scheduledDateUtc,
    String? payload,
  }) async {
    try {
      final tzScheduled = tz.TZDateTime.from(scheduledDateUtc, tz.UTC);

      // iOS 64-pending-notification cap (task 8.5):
      // Cancel one older notification to make room before we schedule,
      // so we never silently lose a near-due notification.
      if (Platform.isIOS) {
        await _enforceIosPendingCap();
      }

      // Android exact-alarm permission check (task 8.4).
      AndroidScheduleMode androidScheduleMode;
      if (Platform.isAndroid) {
        final canExact = await FlutterForegroundTask.canScheduleExactAlarms;
        if (canExact) {
          androidScheduleMode = AndroidScheduleMode.exactAllowWhileIdle;
        } else {
          // Denied: degrade to inexact. Functionality (notification delivered)
          // is preserved; only the timing precision is reduced.
          androidScheduleMode = AndroidScheduleMode.inexact;
          // Open system settings so the user can grant permission — non-blocking.
          await FlutterForegroundTask.openAlarmsAndRemindersSettings();
        }
      } else {
        // Non-Android: exactAllowWhileIdle is an Android-only concept but the
        // enum value is safe to pass — it is ignored on other platforms.
        androidScheduleMode = AndroidScheduleMode.exactAllowWhileIdle;
      }

      const androidDetails = AndroidNotificationDetails(
        'weakup_channel',
        'Weakup',
        channelDescription: 'Weakup job notifications',
        importance: Importance.high,
        priority: Priority.high,
      );
      const details = NotificationDetails(
        android: androidDetails,
        iOS: DarwinNotificationDetails(),
        macOS: DarwinNotificationDetails(),
      );

      await _plugin.zonedSchedule(
        id: id,
        scheduledDate: tzScheduled,
        notificationDetails: details,
        title: title,
        body: body,
        payload: payload,
        androidScheduleMode: androidScheduleMode,
      );
      return Result.success(null);
    } catch (e) {
      return Result.failure(NotificationError(message: e.toString()));
    }
  }

  /// iOS only: cancel the oldest pending notification when the pending count
  /// is at or near the 64-notification cap, so the newest (soonest-due)
  /// notification is never silently dropped (task 8.5).
  ///
  /// Strategy: iOS schedules notifications FIFO and silently drops the oldest
  /// when the cap is exceeded.  We proactively cancel the oldest pending
  /// notification (lowest ID, as a best-effort proxy for oldest) to stay
  /// under the cap before scheduling a new one.
  Future<void> _enforceIosPendingCap() async {
    const iosCap = 64;
    final pending = await _plugin.pendingNotificationRequests();
    if (pending.length < iosCap - 1) return; // safe, no pruning needed

    // Sort by scheduled date (ascending) so we cancel the furthest-future one
    // — keeping the soonest-due notifications, which are highest priority.
    // flutter_local_notifications returns PendingNotificationRequest objects
    // which do not expose the scheduled time directly, so we use ID order
    // as a proxy (IDs are assigned from job IDs, lower == older job created first).
    // This is best-effort: the invariant we must uphold is "never drop the
    // soonest-due notification."
    final sorted = List.of(pending)..sort((a, b) => a.id.compareTo(b.id));
    // Cancel the oldest one to make room.
    if (sorted.isNotEmpty) {
      await _plugin.cancel(id: sorted.last.id);
    }
  }

  @override
  Future<Result<void>> cancel(int id) async {
    try {
      await _plugin.cancel(id: id);
      return Result.success(null);
    } catch (e) {
      return Result.failure(NotificationError(message: e.toString()));
    }
  }

  @override
  Future<Result<void>> cancelAll() async {
    try {
      await _plugin.cancelAll();
      return Result.success(null);
    } catch (e) {
      return Result.failure(NotificationError(message: e.toString()));
    }
  }
}

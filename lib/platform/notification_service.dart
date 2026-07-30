import '../core/result.dart';

/// Interface for immediate and scheduled (zoned) notifications.
abstract interface class NotificationService {
  /// Requests the POST_NOTIFICATIONS runtime permission (Android 13+).
  /// No-op on platforms that don't need it.
  Future<Result<bool>> requestNotificationPermission();

  /// Shows an immediate notification.
  Future<Result<void>> showImmediate({
    required int id,
    required String title,
    required String body,
    String? payload,
  });

  /// Schedules a zoned notification at [scheduledDate] (UTC).
  Future<Result<void>> scheduleZoned({
    required int id,
    required String title,
    required String body,
    required DateTime scheduledDateUtc,
    String? payload,
  });

  /// Cancels a pending scheduled notification by [id].
  Future<Result<void>> cancel(int id);

  /// Cancels all pending scheduled notifications.
  Future<Result<void>> cancelAll();
}

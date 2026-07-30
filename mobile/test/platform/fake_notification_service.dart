import 'package:weakup/core/app_error.dart';
import 'package:weakup/core/result.dart';
import 'package:weakup/platform/notification_service.dart';

class FakeNotificationService implements NotificationService {
  final List<Map<String, dynamic>> immediateNotifications = [];
  final List<Map<String, dynamic>> scheduledNotifications = [];
  final List<int> cancelledIds = [];
  bool permissionGranted = true;
  bool cancelAllCalled = false;

  @override
  Future<Result<bool>> requestNotificationPermission() async {
    if (!permissionGranted) {
      return Result.failure(
          const PermissionDenied(permission: 'POST_NOTIFICATIONS'));
    }
    return Result.success(true);
  }

  @override
  Future<Result<void>> showImmediate({
    required int id,
    required String title,
    required String body,
    String? payload,
  }) async {
    immediateNotifications.add({
      'id': id,
      'title': title,
      'body': body,
      'payload': payload,
    });
    return Result.success(null);
  }

  @override
  Future<Result<void>> scheduleZoned({
    required int id,
    required String title,
    required String body,
    required DateTime scheduledDateUtc,
    String? payload,
  }) async {
    scheduledNotifications.add({
      'id': id,
      'title': title,
      'body': body,
      'scheduledDate': scheduledDateUtc,
      'payload': payload,
    });
    return Result.success(null);
  }

  @override
  Future<Result<void>> cancel(int id) async {
    cancelledIds.add(id);
    scheduledNotifications.removeWhere((n) => n['id'] == id);
    return Result.success(null);
  }

  @override
  Future<Result<void>> cancelAll() async {
    cancelAllCalled = true;
    scheduledNotifications.clear();
    return Result.success(null);
  }
}

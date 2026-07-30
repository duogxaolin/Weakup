import 'package:wakelock_plus/wakelock_plus.dart';

import '../core/app_error.dart';
import '../core/result.dart';
import 'wakelock_controller.dart';

/// Production [WakelockController] backed by `wakelock_plus`.
class WakelockPlusController implements WakelockController {
  bool _held = false;

  @override
  bool get isHeld => _held;

  @override
  Future<Result<void>> acquire() async {
    try {
      await WakelockPlus.enable();
      _held = true;
      return Result.success(null);
    } catch (e) {
      return Result.failure(
        StorageError(message: 'Failed to acquire wakelock: $e'),
      );
    }
  }

  @override
  Future<Result<void>> release() async {
    try {
      await WakelockPlus.disable();
      _held = false;
      return Result.success(null);
    } catch (e) {
      return Result.failure(
        StorageError(message: 'Failed to release wakelock: $e'),
      );
    }
  }
}

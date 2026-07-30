import 'package:weakup/core/result.dart';
import 'package:weakup/platform/wakelock_controller.dart';

/// Test double for [WakelockController].
class FakeWakelockController implements WakelockController {
  bool _held = false;
  int _acquireCount = 0;
  int _releaseCount = 0;

  @override
  bool get isHeld => _held;

  int get acquireCount => _acquireCount;
  int get releaseCount => _releaseCount;

  @override
  Future<Result<void>> acquire() async {
    _held = true;
    _acquireCount++;
    return Result.success(null);
  }

  @override
  Future<Result<void>> release() async {
    _held = false;
    _releaseCount++;
    return Result.success(null);
  }

  void reset() {
    _held = false;
    _acquireCount = 0;
    _releaseCount = 0;
  }
}

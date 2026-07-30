import 'package:weakup/core/result.dart';
import 'package:weakup/platform/power_off_executor.dart';

/// Test double for [PowerOffExecutor].
/// Records invocations without shutting down the machine.
class FakePowerOffExecutor implements PowerOffExecutor {
  final List<DateTime> invocations = [];
  bool _isSupported;
  Result<void> _nextResult;

  FakePowerOffExecutor({
    this._isSupported = true,
    Result<void>? nextResult,
  }) : _nextResult = nextResult ?? Result.success(null);

  @override
  bool get isSupported => _isSupported;

  set isSupported(bool value) => _isSupported = value;

  /// Configure what the next call to [powerOff] returns.
  void setNextResult(Result<void> result) => _nextResult = result;

  @override
  Future<Result<void>> powerOff() async {
    invocations.add(DateTime.now());
    return _nextResult;
  }

  int get callCount => invocations.length;
  bool get wasCalled => invocations.isNotEmpty;
}

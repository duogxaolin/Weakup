import 'package:weakup/platform/foreground_service_controller.dart';

/// Test double for [ForegroundServiceController].
///
/// Records start/stop calls and lets a test force the `refused` outcome that
/// Android 15's 6-hour `dataSync` cap produces, which is otherwise
/// unreachable without a live Android device.
class FakeForegroundServiceController implements ForegroundServiceController {
  FakeForegroundServiceController({this.isSupported = true});

  @override
  bool isSupported;

  /// Outcome returned by [start]. Set to
  /// [ForegroundServiceStartOutcome.refused] to simulate the cap.
  ForegroundServiceStartOutcome startOutcome =
      ForegroundServiceStartOutcome.running;

  /// `activeJobCount` for each [start] call, in order.
  final List<int> startCalls = [];

  int stopCallCount = 0;

  bool get startCalled => startCalls.isNotEmpty;

  @override
  Future<ForegroundServiceStartOutcome> start({
    required int activeJobCount,
  }) async {
    startCalls.add(activeJobCount);
    return startOutcome;
  }

  @override
  Future<void> stop() async {
    stopCallCount++;
  }
}

import '../core/result.dart';

/// Interface for acquiring/releasing the OS screen-awake assertion.
abstract interface class WakelockController {
  /// Acquires the screen-awake assertion.
  Future<Result<void>> acquire();

  /// Releases the screen-awake assertion.
  Future<Result<void>> release();

  /// Whether the wakelock is currently held.
  bool get isHeld;
}

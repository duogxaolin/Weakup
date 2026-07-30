import '../core/result.dart';

/// Injectable interface for executing a platform power-off.
///
/// All scheduling and grace-period logic depends on this interface,
/// not on a concrete platform implementation.
abstract interface class PowerOffExecutor {
  /// Executes the platform-specific power-off command.
  Future<Result<void>> powerOff();

  /// Whether this platform supports power-off.
  bool get isSupported;
}

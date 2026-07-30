import 'dart:io';

import '../core/app_error.dart';
import '../core/result.dart';
import 'power_off_executor.dart';

/// Windows: invokes `shutdown /s /t 0`.
class WindowsPowerOffExecutor implements PowerOffExecutor {
  @override
  bool get isSupported => true;

  @override
  Future<Result<void>> powerOff() async {
    try {
      final result = await Process.run('shutdown', ['/s', '/t', '0']);
      if (result.exitCode == 0) {
        return Result.success(null);
      }
      final stderr = result.stderr.toString();
      // ERROR_PRIVILEGE_NOT_HELD = 1314 = 0x522
      if (stderr.contains('1314') || stderr.contains('ERROR_PRIVILEGE_NOT_HELD')) {
        return Result.failure(PowerOffPrivilegeDenied(detail: stderr));
      }
      return Result.failure(PowerOffFailed(message: stderr));
    } catch (e) {
      return Result.failure(PowerOffFailed(message: e.toString()));
    }
  }
}

/// macOS: invokes AppleScript `tell application "System Events" to shut down`.
class MacOsPowerOffExecutor implements PowerOffExecutor {
  @override
  bool get isSupported => true;

  @override
  Future<Result<void>> powerOff() async {
    try {
      final result = await Process.run('osascript', [
        '-e',
        'tell application "System Events" to shut down',
      ]);
      if (result.exitCode == 0) {
        return Result.success(null);
      }
      final stderr = result.stderr.toString();
      final stdout = result.stdout.toString();
      final output = '$stderr $stdout';
      // TCC denial produces "Not authorized" or "not allowed" error messages.
      if (output.toLowerCase().contains('not authorized') ||
          output.toLowerCase().contains('not allowed') ||
          output.toLowerCase().contains('access denied')) {
        return Result.failure(PowerOffConsentDenied(detail: output));
      }
      return Result.failure(PowerOffFailed(message: output));
    } catch (e) {
      return Result.failure(PowerOffFailed(message: e.toString()));
    }
  }
}

/// Linux: invokes `systemctl poweroff`.
class LinuxPowerOffExecutor implements PowerOffExecutor {
  @override
  bool get isSupported => true;

  @override
  Future<Result<void>> powerOff() async {
    try {
      final result = await Process.run('systemctl', ['poweroff']);
      if (result.exitCode == 0) {
        return Result.success(null);
      }
      final stderr = result.stderr.toString();
      // polkit denial produces "access denied" or "interactive authentication required"
      if (stderr.toLowerCase().contains('access denied') ||
          stderr.toLowerCase().contains('polkit') ||
          stderr.toLowerCase().contains('interactive auth')) {
        return Result.failure(PowerOffPolicyDenied(detail: stderr));
      }
      return Result.failure(PowerOffFailed(message: stderr));
    } catch (e) {
      return Result.failure(PowerOffFailed(message: e.toString()));
    }
  }
}

/// Android and iOS: power-off is categorically impossible.
/// Returns [PowerOffUnsupported] and NEVER invokes any shutdown API.
class UnsupportedPowerOffExecutor implements PowerOffExecutor {
  const UnsupportedPowerOffExecutor();

  @override
  bool get isSupported => false;

  @override
  Future<Result<void>> powerOff() async {
    return Result.failure(const PowerOffUnsupported());
  }
}

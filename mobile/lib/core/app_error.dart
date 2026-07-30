/// Typed error hierarchy for all fallible operations.
/// Each variant carries enough information for a specific, actionable user message.
sealed class AppError {
  const AppError();
}

// Power-off errors

/// Android or iOS: power-off is impossible on this platform.
final class PowerOffUnsupported extends AppError {
  const PowerOffUnsupported();

  @override
  String toString() => 'PowerOffUnsupported';
}

/// Windows: `ERROR_PRIVILEGE_NOT_HELD` — domain policy stripped SeShutdownPrivilege.
final class PowerOffPrivilegeDenied extends AppError {
  const PowerOffPrivilegeDenied({this.detail});
  final String? detail;

  @override
  String toString() => 'PowerOffPrivilegeDenied(${detail ?? ''})';
}

/// macOS: TCC Automation consent for System Events was denied.
final class PowerOffConsentDenied extends AppError {
  const PowerOffConsentDenied({this.detail});
  final String? detail;

  @override
  String toString() => 'PowerOffConsentDenied(${detail ?? ''})';
}

/// Linux: polkit denied the `org.freedesktop.login1.power-off` action.
final class PowerOffPolicyDenied extends AppError {
  const PowerOffPolicyDenied({this.detail});
  final String? detail;

  @override
  String toString() => 'PowerOffPolicyDenied(${detail ?? ''})';
}

/// A power-off command failed for an unclassified reason.
final class PowerOffFailed extends AppError {
  const PowerOffFailed({required this.message});
  final String message;

  @override
  String toString() => 'PowerOffFailed($message)';
}

// Permission errors

/// A runtime OS permission was denied by the user.
final class PermissionDenied extends AppError {
  const PermissionDenied({required this.permission});
  final String permission;

  @override
  String toString() => 'PermissionDenied($permission)';
}

// Storage errors

final class StorageError extends AppError {
  const StorageError({required this.message});
  final String message;

  @override
  String toString() => 'StorageError($message)';
}

// Notification errors

final class NotificationError extends AppError {
  const NotificationError({required this.message});
  final String message;

  @override
  String toString() => 'NotificationError($message)';
}

// Validation errors

/// User input is invalid — message is directly displayable.
final class ValidationError extends AppError {
  const ValidationError({required this.message});
  final String message;

  @override
  String toString() => 'ValidationError($message)';
}

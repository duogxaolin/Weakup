import 'dart:io';

/// Single source of truth for what the current platform can do.
///
/// Resolved once at startup via [PlatformCapabilities.resolve].
/// All UI and scheduling logic branches on this model, never on raw
/// [Platform.isX] checks scattered across the codebase.
///
/// Constructible explicitly for tests, so platform-specific branches are
/// testable on any development machine.
final class PlatformCapabilities {
  const PlatformCapabilities({
    required this.supportsPowerOff,
    required this.keepAwakePersistsInBackground,
    required this.supportsAutostart,
    required this.requiresRuntimeSchedulingPermission,
    required this.canBeRemoteTarget,
    required this.isDesktop,
    required this.isMobile,
  });

  /// True on Windows, macOS, and Linux; false on Android and iOS.
  final bool supportsPowerOff;

  /// True on Windows, macOS, Linux, and Android (with foreground service);
  /// false on iOS.
  final bool keepAwakePersistsInBackground;

  /// True on Windows, macOS, and Linux; false on Android and iOS.
  final bool supportsAutostart;

  /// True on Android 13+ (exact alarms require runtime permission).
  final bool requiresRuntimeSchedulingPermission;

  /// Whether this device can act as a remote-control target.
  ///
  /// True on Windows, macOS, and Linux; false on Android and iOS. A device can only be a
  /// target if it can both keep a background presence and perform the actions a remote
  /// command would ask for — on mobile the OS suspends the app and forbids power-off, so
  /// a command sent to one could neither be received reliably nor carried out.
  ///
  /// That asymmetry is the point of the feature rather than a limitation of it: a phone
  /// is a useful remote *controller* precisely because it need not be a *target*.
  ///
  /// Remote-command authorization branches on this rather than on a [Platform.isX] check
  /// of its own — though note it takes the *target's* value over the wire, since the
  /// target is generally not this device.
  final bool canBeRemoteTarget;

  /// True on Windows, macOS, Linux.
  final bool isDesktop;

  /// True on Android, iOS.
  final bool isMobile;

  /// Resolves capabilities for the current running platform.
  static PlatformCapabilities resolve() {
    if (Platform.isWindows || Platform.isMacOS || Platform.isLinux) {
      return const PlatformCapabilities(
        supportsPowerOff: true,
        keepAwakePersistsInBackground: true,
        supportsAutostart: true,
        requiresRuntimeSchedulingPermission: false,
        canBeRemoteTarget: true,
        isDesktop: true,
        isMobile: false,
      );
    } else if (Platform.isAndroid) {
      return const PlatformCapabilities(
        supportsPowerOff: false,
        // Background keep-awake available only while foreground service runs.
        // Treated as true here (the foreground service is managed separately).
        keepAwakePersistsInBackground: true,
        supportsAutostart: false,
        // Android 13+ requires SCHEDULE_EXACT_ALARM at runtime.
        requiresRuntimeSchedulingPermission: true,
        canBeRemoteTarget: false,
        isDesktop: false,
        isMobile: true,
      );
    } else if (Platform.isIOS) {
      return const PlatformCapabilities(
        supportsPowerOff: false,
        keepAwakePersistsInBackground: false,
        supportsAutostart: false,
        requiresRuntimeSchedulingPermission: false,
        canBeRemoteTarget: false,
        isDesktop: false,
        isMobile: true,
      );
    } else {
      // Web or unknown — treat as most restrictive.
      return const PlatformCapabilities(
        supportsPowerOff: false,
        keepAwakePersistsInBackground: false,
        supportsAutostart: false,
        requiresRuntimeSchedulingPermission: false,
        canBeRemoteTarget: false,
        isDesktop: false,
        isMobile: false,
      );
    }
  }

  /// Capability set for Windows — for tests and injection.
  static const PlatformCapabilities windows = PlatformCapabilities(
    supportsPowerOff: true,
    keepAwakePersistsInBackground: true,
    supportsAutostart: true,
    requiresRuntimeSchedulingPermission: false,
    canBeRemoteTarget: true,
    isDesktop: true,
    isMobile: false,
  );

  /// Capability set for macOS — for tests and injection.
  static const PlatformCapabilities macos = PlatformCapabilities(
    supportsPowerOff: true,
    keepAwakePersistsInBackground: true,
    supportsAutostart: true,
    requiresRuntimeSchedulingPermission: false,
    canBeRemoteTarget: true,
    isDesktop: true,
    isMobile: false,
  );

  /// Capability set for Linux — for tests and injection.
  static const PlatformCapabilities linux = PlatformCapabilities(
    supportsPowerOff: true,
    keepAwakePersistsInBackground: true,
    supportsAutostart: true,
    requiresRuntimeSchedulingPermission: false,
    canBeRemoteTarget: true,
    isDesktop: true,
    isMobile: false,
  );

  /// Capability set for Android — for tests and injection.
  static const PlatformCapabilities android = PlatformCapabilities(
    supportsPowerOff: false,
    keepAwakePersistsInBackground: true,
    supportsAutostart: false,
    requiresRuntimeSchedulingPermission: true,
    canBeRemoteTarget: false,
    isDesktop: false,
    isMobile: true,
  );

  /// Capability set for iOS — for tests and injection.
  static const PlatformCapabilities ios = PlatformCapabilities(
    supportsPowerOff: false,
    keepAwakePersistsInBackground: false,
    supportsAutostart: false,
    requiresRuntimeSchedulingPermission: false,
    canBeRemoteTarget: false,
    isDesktop: false,
    isMobile: true,
  );

  @override
  String toString() => 'PlatformCapabilities('
      'powerOff=$supportsPowerOff, '
      'bgKeepAwake=$keepAwakePersistsInBackground, '
      'autostart=$supportsAutostart, '
      'runtimePerm=$requiresRuntimeSchedulingPermission, '
      'remoteTarget=$canBeRemoteTarget, '
      'desktop=$isDesktop, '
      'mobile=$isMobile)';
}

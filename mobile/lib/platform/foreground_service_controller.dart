/// Outcome of a request to start the background foreground service.
enum ForegroundServiceStartOutcome {
  /// The service is running (either newly started or already running and
  /// updated in place).
  running,

  /// The platform refused to start the service. On Android 15+ the most
  /// likely cause is the 6-hour/day `dataSync` cap, but the Dart layer cannot
  /// distinguish the cap from other start denials, so callers must treat this
  /// as "background execution is not available" and degrade rather than
  /// silently drop the job.
  refused,

  /// This platform has no foreground service concept (desktop, iOS, web).
  /// Not an error; nothing was attempted and nothing needs to degrade.
  notApplicable,
}

/// Controls the OS background-execution service that keeps active jobs alive
/// while the app is not in the foreground.
///
/// Abstracted so [JobScheduler] can be tested without a live Android
/// platform channel — the cap/refusal path is otherwise unreachable in tests.
abstract interface class ForegroundServiceController {
  /// Whether this platform has a foreground service to manage at all.
  bool get isSupported;

  /// Starts the service, or updates its notification if already running.
  ///
  /// [activeJobCount] is used only for the notification text.
  Future<ForegroundServiceStartOutcome> start({required int activeJobCount});

  /// Stops the service if it is running. Safe to call when not running.
  Future<void> stop();
}

/// No-op controller for platforms without a foreground service, and the
/// default so callers that do not care about background execution (widget
/// tests, desktop) need no wiring.
final class NoopForegroundServiceController
    implements ForegroundServiceController {
  const NoopForegroundServiceController();

  @override
  bool get isSupported => false;

  @override
  Future<ForegroundServiceStartOutcome> start({
    required int activeJobCount,
  }) async =>
      ForegroundServiceStartOutcome.notApplicable;

  @override
  Future<void> stop() async {}
}

import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_timezone/flutter_timezone.dart';
import 'package:timezone/data/latest.dart' as tz_data;
import 'package:timezone/timezone.dart' as tz;

import 'application/job_scheduler.dart';
import 'application/providers.dart';
import 'platform/android_foreground_service.dart';
import 'platform/desktop_runtime.dart';
import 'ui/screens/home_screen.dart';
import 'ui/theme/theme.dart';

/// Global navigator key so code outside the widget tree (e.g. JobScheduler)
/// can push routes such as GracePeriodScreen.
final navigatorKey = GlobalKey<NavigatorState>();

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  tz_data.initializeTimeZones();

  // Initialize the Android foreground service once before runApp so that
  // start() calls from JobScheduler work immediately.
  if (Platform.isAndroid) {
    AndroidForegroundServiceController.init();
  }

  // Initialize desktop runtime (tray, window-manager) before runApp so the
  // window manager is ready before the first frame.
  DesktopRuntime? desktopRuntime;
  if (Platform.isWindows || Platform.isMacOS || Platform.isLinux) {
    desktopRuntime = DesktopRuntime(() {
      // Quit callback: nothing to do — destroy() already closes the process.
    });
    await desktopRuntime.init();
  }

  runApp(ProviderScope(
    overrides: [
      if (desktopRuntime != null)
        desktopRuntimeProvider.overrideWithValue(desktopRuntime),
    ],
    child: WeakupApp(desktopRuntime: desktopRuntime),
  ));
}

class WeakupApp extends ConsumerStatefulWidget {
  const WeakupApp({this.desktopRuntime, super.key});

  final DesktopRuntime? desktopRuntime;

  @override
  ConsumerState<WeakupApp> createState() => _WeakupAppState();
}

class _WeakupAppState extends ConsumerState<WeakupApp>
    with WidgetsBindingObserver {
  /// Pending one-shot subscription used when the scheduler is not ready yet.
  /// Held so repeated resumes cannot accumulate listeners.
  ProviderSubscription<JobScheduler?>? _pendingSchedulerSub;

  /// Last timezone we resolved, so we only re-resolve jobs on a real change.
  String? _lastTimezoneId;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    // Cold-start reconcile: run after the first frame so that providers
    // (especially the async notificationServiceProvider) have settled.
    WidgetsBinding.instance.addPostFrameCallback((_) => _onForeground());
  }

  @override
  void dispose() {
    _pendingSchedulerSub?.close();
    WidgetsBinding.instance.removeObserver(this);
    widget.desktopRuntime?.dispose();
    super.dispose();
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state == AppLifecycleState.resumed) {
      _onForeground();
    }
  }

  @override
  void didChangeLocales(List<Locale>? locales) {
    // A locale change often accompanies a region/timezone change. This is a
    // secondary trigger only — resume-time detection is the reliable one,
    // since a timezone change does not always change the locale.
    super.didChangeLocales(locales);
    _checkTimezoneChange();
  }

  /// Runs every time the app comes to the foreground (and once on cold start).
  ///
  /// Reconciles overdue/pending jobs first, then checks whether the device
  /// timezone moved while we were backgrounded.
  void _onForeground() {
    _withScheduler((scheduler) async {
      await scheduler.reconcile();
      await _resolveAndApplyTimezone(scheduler);
    });
  }

  void _checkTimezoneChange() {
    _withScheduler(_resolveAndApplyTimezone);
  }

  /// Reads the current device timezone and, if it differs from the last one we
  /// saw, re-resolves absolute-time jobs against it.
  Future<void> _resolveAndApplyTimezone(JobScheduler scheduler) async {
    final tz.Location location;
    final String id;
    try {
      final info = await FlutterTimezone.getLocalTimezone();
      id = info.identifier;
      location = tz.getLocation(id);
    } catch (_) {
      // Timezone unavailable (test env / web). Leave jobs as they are rather
      // than re-resolving against a wrong zone.
      return;
    }

    // First observation establishes the baseline; jobs were already resolved
    // against this zone at creation time, so there is nothing to re-resolve.
    final previous = _lastTimezoneId;
    _lastTimezoneId = id;
    if (previous == null || previous == id) return;

    tz.setLocalLocation(location);
    await scheduler.handleTimezoneChange(location);
  }

  /// Runs [action] with the scheduler, waiting for it if the async notification
  /// service has not resolved yet. At most one pending wait exists at a time.
  void _withScheduler(void Function(JobScheduler scheduler) action) {
    final scheduler = ref.read(jobSchedulerProvider);
    if (scheduler != null) {
      action(scheduler);
      return;
    }

    // Scheduler not ready yet (notification service still loading). Replace any
    // earlier pending wait so listeners cannot accumulate across resumes.
    _pendingSchedulerSub?.close();
    _pendingSchedulerSub = ref.listenManual(
      jobSchedulerProvider,
      (_, next) {
        if (next == null) return;
        _pendingSchedulerSub?.close();
        _pendingSchedulerSub = null;
        action(next);
      },
      fireImmediately: false,
    );
  }

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      navigatorKey: navigatorKey,
      title: 'Weakup',
      theme: WeakupTheme.light,
      home: const HomeScreen(),
      debugShowCheckedModeBanner: false,
    );
  }
}

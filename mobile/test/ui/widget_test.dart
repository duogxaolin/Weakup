import 'package:drift/drift.dart' show driftRuntimeOptions;
import 'package:drift/native.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:timezone/data/latest.dart' as tz_data;
import 'package:weakup/application/job_scheduler.dart';
import 'package:weakup/application/providers.dart';
import 'package:weakup/data/app_database.dart';
import 'package:weakup/data/drift_job_repository.dart';
import 'package:weakup/domain/job.dart';
import 'package:weakup/domain/job_enums.dart';
import 'package:weakup/domain/job_repository.dart';
import 'package:weakup/domain/trigger_spec.dart';
import 'package:weakup/platform/notification_service.dart';
import 'package:weakup/platform/platform_capabilities.dart';
import 'package:weakup/platform/power_off_executor.dart';
import 'package:weakup/platform/wakelock_controller.dart';
import 'package:weakup/ui/screens/create_job_screen.dart';
import 'package:weakup/ui/screens/grace_period_screen.dart';
import 'package:weakup/ui/theme/theme.dart';
import 'package:weakup/ui/widgets/job_list_tile.dart';

import '../platform/fake_notification_service.dart';
import '../platform/fake_power_off_executor.dart';
import '../platform/fake_wakelock_controller.dart';

AppDatabase _testDb() => AppDatabase(NativeDatabase.memory());

/// A Notifier pre-loaded with a given state for widget tests.
class _PreloadedJobCreationNotifier extends JobCreationNotifier {
  _PreloadedJobCreationNotifier(this._initial);
  final JobCreationState _initial;

  @override
  JobCreationState build() => _initial;
}

/// Builds a widget with faked providers for testing.
/// [initialCreationState] pre-loads the job creation form state.
Widget _testApp({
  required Widget child,
  PlatformCapabilities? capabilities,
  JobRepository? repo,
  JobScheduler? scheduler,
  NotificationService? notificationService,
  WakelockController? wakelockController,
  PowerOffExecutor? powerOffExecutor,
  JobCreationState? initialCreationState,
}) {
  final db = _testDb();
  final fakeRepo = repo ?? DriftJobRepository(db);
  final fakeCaps = capabilities ?? PlatformCapabilities.macos;
  final fakeNotif = notificationService ?? FakeNotificationService();
  final fakeWakelock = wakelockController ?? FakeWakelockController();
  final fakeExecutor = powerOffExecutor ?? FakePowerOffExecutor();
  final fakeScheduler = scheduler ??
      JobScheduler(
        repository: fakeRepo,
        powerOffExecutor: fakeExecutor,
        wakelockController: fakeWakelock,
        notificationService: fakeNotif,
      );

  final initialState = initialCreationState;

  return ProviderScope(
    overrides: [
      platformCapabilitiesProvider.overrideWithValue(fakeCaps),
      jobRepositoryProvider.overrideWithValue(fakeRepo),
      wakelockControllerProvider.overrideWithValue(fakeWakelock),
      powerOffExecutorProvider.overrideWithValue(fakeExecutor),
      notificationServiceProvider.overrideWith((ref) async => fakeNotif),
      jobSchedulerProvider.overrideWithValue(fakeScheduler),
      if (initialState != null)
        jobCreationProvider.overrideWith(
            () => _PreloadedJobCreationNotifier(initialState)),
    ],
    child: MaterialApp(
      theme: WeakupTheme.light,
      home: child,
    ),
  );
}

void main() {
  setUpAll(() {
    tz_data.initializeTimeZones();
    // Each test builds its own AppDatabase over a *separate* in-memory
    // executor, so Drift's shared-executor race warning does not apply here.
    // Silence it so it does not bury real failures in the test output.
    driftRuntimeOptions.dontWarnAboutMultipleDatabases = true;
  });

  group('CreateJobScreen', () {
    testWidgets('16.2: Validation error for zero duration keep-awake',
        (tester) async {
      // Pre-load the notifier with keepAwake enabled and DurationTrigger(0).
      await tester.pumpWidget(_testApp(
        child: const CreateJobScreen(),
        initialCreationState: const JobCreationState(
          keepAwakeEnabled: true,
          keepAwakeTrigger: DurationTrigger(minutes: 0),
        ),
      ));
      await tester.pumpAndSettle();

      // Tap Create — triggers validation for zero duration
      await tester.tap(find.byKey(const Key('btn_create')));
      await tester.pumpAndSettle();

      // Validation error must be visible
      expect(find.byKey(const Key('error_message')), findsOneWidget,
          reason:
              'Zero duration must produce a visible validation error message');
    });

    testWidgets('16.2: Validation error for over-maximum duration (1441 min)',
        (tester) async {
      await tester.pumpWidget(_testApp(
        child: const CreateJobScreen(),
        initialCreationState: const JobCreationState(
          keepAwakeEnabled: true,
          keepAwakeTrigger: DurationTrigger(minutes: 1441),
        ),
      ));
      await tester.pumpAndSettle();

      await tester.tap(find.byKey(const Key('btn_create')));
      await tester.pumpAndSettle();

      expect(find.byKey(const Key('error_message')), findsOneWidget,
          reason:
              'Over-maximum duration must produce a visible validation error message');
    });

    testWidgets(
        '16.6: Mobile power-off limitation notice shown BEFORE Create button is used',
        (tester) async {
      // Test with Android capability — power-off unsupported.
      await tester.pumpWidget(_testApp(
        child: const CreateJobScreen(),
        capabilities: PlatformCapabilities.android,
        initialCreationState: const JobCreationState(
          powerOffEnabled: true,
          powerOffTrigger: DurationTrigger(minutes: 60),
        ),
      ));
      await tester.pumpAndSettle();

      // Limitation notice must be visible WITHOUT having tapped Create yet.
      expect(
        find.byKey(const Key('notice_mobile_poweroff')),
        findsOneWidget,
        reason:
            'Mobile power-off limitation notice must appear before the Create button is tapped',
      );

      expect(
        find.textContaining('does not allow apps to power off'),
        findsOneWidget,
      );
    });

    testWidgets(
        '16.6: iOS keep-awake foreground-only notice shown before Create',
        (tester) async {
      await tester.pumpWidget(_testApp(
        child: const CreateJobScreen(),
        capabilities: PlatformCapabilities.ios,
        initialCreationState: const JobCreationState(
          keepAwakeEnabled: true,
        ),
      ));
      await tester.pumpAndSettle();

      expect(
        find.byKey(const Key('notice_ios_keepawake')),
        findsOneWidget,
        reason: 'iOS foreground-only notice must appear before Create is tapped',
      );
    });

    testWidgets(
        '16.6: iOS power-off notice shown before Create on iOS',
        (tester) async {
      // iOS also shows the power-off reminder notice
      await tester.pumpWidget(_testApp(
        child: const CreateJobScreen(),
        capabilities: PlatformCapabilities.ios,
        initialCreationState: const JobCreationState(
          powerOffEnabled: true,
          powerOffTrigger: DurationTrigger(minutes: 60),
        ),
      ));
      await tester.pumpAndSettle();

      expect(
        find.byKey(const Key('notice_mobile_poweroff')),
        findsOneWidget,
        reason:
            'iOS power-off limitation notice must appear before Create is tapped',
      );
    });
  });

  group('GracePeriodScreen', () {
    testWidgets(
        '16.5: Grace-period countdown renders with Cancel and cancelling aborts',
        (tester) async {
      bool cancelCalled = false;
      bool proceedCalled = false;

      final now = DateTime.now().toUtc();
      final job = Job(
        id: 1,
        type: JobType.powerOff,
        trigger: const DurationTrigger(minutes: 1),
        status: JobStatus.active,
        targetInstantUtc: now.add(const Duration(minutes: 1)),
        createdAt: now,
        updatedAt: now,
      );

      await tester.pumpWidget(MaterialApp(
        home: GracePeriodScreen(
          job: job,
          onCancel: () => cancelCalled = true,
          onProceed: () => proceedCalled = true,
        ),
      ));
      await tester.pump();

      // Cancel button must be present.
      expect(find.byKey(const Key('btn_cancel_poweroff')), findsOneWidget);

      // Countdown shows 60.
      expect(find.textContaining('60'), findsWidgets);

      // Tapping Cancel must invoke onCancel and must NOT invoke onProceed.
      await tester.tap(find.byKey(const Key('btn_cancel_poweroff')));
      await tester.pump();
      expect(cancelCalled, isTrue,
          reason: 'Cancel button must invoke cancel callback');
      expect(proceedCalled, isFalse,
          reason:
              'Cancel must NOT invoke proceed — this would allow power-off without grace period');
    });
  });

  group('HomeScreen — job list (16.1)', () {
    testWidgets('16.1: keep-awake-only job: form submits and navigator pops',
        (tester) async {
      final db = _testDb();
      final fakeRepo = DriftJobRepository(db);
      final fakeNotif = FakeNotificationService();
      final fakeWakelock = FakeWakelockController();
      final fakeExecutor = FakePowerOffExecutor();
      final fakeScheduler = JobScheduler(
        repository: fakeRepo,
        powerOffExecutor: fakeExecutor,
        wakelockController: fakeWakelock,
        notificationService: fakeNotif,
      );

      await tester.pumpWidget(_testApp(
        child: const CreateJobScreen(),
        repo: fakeRepo,
        scheduler: fakeScheduler,
        initialCreationState: const JobCreationState(
          keepAwakeEnabled: true,
          keepAwakeTrigger: IndefiniteTrigger(),
        ),
      ));
      await tester.pumpAndSettle();

      // Create button must be present.
      expect(find.byKey(const Key('btn_create')), findsOneWidget);
      // Keep-awake section is expanded.
      expect(find.byKey(const Key('trigger_keepawake')), findsOneWidget);
      // Power-off section is NOT expanded.
      expect(find.byKey(const Key('trigger_poweroff')), findsNothing);
    });

    testWidgets('16.1: power-off-only job: form shows power-off trigger only',
        (tester) async {
      await tester.pumpWidget(_testApp(
        child: const CreateJobScreen(),
        initialCreationState: const JobCreationState(
          powerOffEnabled: true,
          powerOffTrigger: DurationTrigger(minutes: 60),
        ),
      ));
      await tester.pumpAndSettle();

      expect(find.byKey(const Key('trigger_poweroff')), findsOneWidget);
      expect(find.byKey(const Key('trigger_keepawake')), findsNothing);
    });

    testWidgets(
        '16.1: both keep-awake and power-off: form shows both trigger inputs',
        (tester) async {
      await tester.pumpWidget(_testApp(
        child: const CreateJobScreen(),
        initialCreationState: const JobCreationState(
          keepAwakeEnabled: true,
          powerOffEnabled: true,
          powerOffTrigger: DurationTrigger(minutes: 60),
        ),
      ));
      await tester.pumpAndSettle();

      expect(find.byKey(const Key('trigger_keepawake')), findsOneWidget);
      expect(find.byKey(const Key('trigger_poweroff')), findsOneWidget);
    });
  });

  group('Replace-confirmation banner (16.3)', () {
    testWidgets(
        '16.3: banner appears when creating a second keep-awake job',
        (tester) async {
      await tester.pumpWidget(_testApp(
        child: const CreateJobScreen(),
        // Pre-load state: keepAwake enabled and already pending replace
        initialCreationState: const JobCreationState(
          keepAwakeEnabled: true,
          pendingReplaceType: JobType.keepAwake,
        ),
      ));
      await tester.pumpAndSettle();

      // Replace confirmation banner must be visible.
      expect(
        find.byKey(const Key('replace_confirm_title_keepAwake')),
        findsOneWidget,
        reason: 'Replace confirmation banner must appear for keepAwake type',
      );
      expect(
        find.byKey(const Key('btn_confirm_replace_keepAwake')),
        findsOneWidget,
      );
    });

    testWidgets('16.3: banner appears for power-off replace', (tester) async {
      await tester.pumpWidget(_testApp(
        child: const CreateJobScreen(),
        initialCreationState: const JobCreationState(
          powerOffEnabled: true,
          pendingReplaceType: JobType.powerOff,
        ),
      ));
      await tester.pumpAndSettle();

      expect(
        find.byKey(const Key('replace_confirm_title_powerOff')),
        findsOneWidget,
        reason: 'Replace confirmation banner must appear for powerOff type',
      );
    });
  });

  group('Job actions from list tile (16.4)', () {
    Job makeActiveJob({required int id, required JobType type}) {
      final now = DateTime.now().toUtc();
      return Job(
        id: id,
        type: type,
        trigger: const IndefiniteTrigger(),
        status: JobStatus.active,
        targetInstantUtc: null,
        createdAt: now,
        updatedAt: now,
      );
    }

    testWidgets(
        '16.4: pause and cancel buttons are shown for an active job tile',
        (tester) async {
      final job = makeActiveJob(id: 1, type: JobType.keepAwake);

      await tester.pumpWidget(_testApp(
        child: Scaffold(body: JobListTile(job: job)),
      ));
      await tester.pump(); // one frame — don't pumpAndSettle (avoids timer flush)

      // Both pause and cancel buttons must be visible for an active job.
      expect(find.byIcon(Icons.pause), findsOneWidget,
          reason: 'Pause button must be visible for an active job');
      expect(find.byIcon(Icons.cancel_outlined), findsOneWidget,
          reason: 'Cancel button must be visible for an active job');
    });

    testWidgets('16.4: pause button is absent for a paused job tile',
        (tester) async {
      final now = DateTime.now().toUtc();
      final job = Job(
        id: 2,
        type: JobType.keepAwake,
        trigger: const IndefiniteTrigger(),
        status: JobStatus.paused,
        targetInstantUtc: null,
        createdAt: now,
        updatedAt: now,
      );

      await tester.pumpWidget(_testApp(
        child: Scaffold(body: JobListTile(job: job)),
      ));
      await tester.pump();

      // Paused job shows resume but not pause.
      expect(find.byIcon(Icons.play_arrow), findsOneWidget,
          reason: 'Resume button must be visible for a paused job');
      expect(find.byIcon(Icons.pause), findsNothing,
          reason: 'Pause button must NOT be visible for a paused job');
    });
  });
}

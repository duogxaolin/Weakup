## 1. Project scaffolding and dependencies

- [x] 1.1 Run `flutter create` in `/Users/duogxaolin/Code/Weakup` with org `com.weakup`, project name `weakup`, and platforms `windows,macos,linux,android,ios` (do not enable web as a shipping target, but leave it buildable for fast test runs)
- [x] 1.2 Add all runtime dependencies to `pubspec.yaml` at the pinned versions from design.md D1–D8: `flutter_riverpod ^3.4.2`, `drift ^2.34.3`, `drift_flutter`, `sqlite3_flutter_libs`, `wakelock_plus ^1.7.0`, `tray_manager ^0.5.3`, `window_manager ^0.5.2`, `launch_at_startup ^0.5.1`, `flutter_foreground_task ^10.0.0`, `flutter_local_notifications ^22.2.0`, `timezone ^0.11.1`, `flutter_timezone ^5.1.0`
- [x] 1.3 Add dev dependencies: `build_runner`, `drift_dev`, `flutter_test`, `flutter_lints`
- [x] 1.4 Create the layered directory structure under `lib/` per design.md D17: `core/`, `data/`, `domain/`, `platform/`, `application/`, `ui/`
- [x] 1.5 Run `flutter pub get` and confirm zero dependency resolution conflicts ← (verify: `flutter pub get` exits 0, `flutter analyze` reports no errors on the fresh scaffold)

## 2. Core primitives

- [x] 2.1 Implement `Result<T>` as a sealed class with `Success<T>` and `Failure<T>` variants in `lib/core/result.dart` per design.md D4
- [x] 2.2 Implement a sealed `AppError` hierarchy in `lib/core/app_error.dart` with distinct variants for at minimum: `PowerOffUnsupported`, `PowerOffPrivilegeDenied` (Windows), `PowerOffConsentDenied` (macOS), `PowerOffPolicyDenied` (Linux), `PermissionDenied`, `StorageError`, `NotificationError`
- [x] 2.3 Write unit tests for `Result` construction, mapping, and folding ← (verify: tests cover both Success and Failure paths, all pass)

## 3. Domain model and trigger resolution

- [x] 3.1 Define `JobType` enum (`keepAwake`, `powerOff`) and `JobStatus` enum (`active`, `paused`, `completed`, `cancelled`, `failed`, `overdue`, `degraded`) in `lib/domain/`
- [x] 3.2 Define a sealed `TriggerSpec` with `IndefiniteTrigger`, `DurationTrigger(minutes)`, and `AbsoluteTimeTrigger(hour, minute)` variants
- [x] 3.3 Define the `Job` entity with `id`, `type`, `trigger`, `status`, `targetInstantUtc`, `createdAt`, `updatedAt` — no remaining-seconds field, per power-job-scheduling spec
- [x] 3.4 Implement `TriggerResolver` as pure functions (no Flutter or plugin imports) that resolve a `TriggerSpec` to a `TZDateTime` target using the device IANA zone
- [x] 3.5 Implement validation: reject `IndefiniteTrigger` for `powerOff`; reject duration `<= 0` or `> 1440` minutes
- [x] 3.6 Implement absolute-time resolution rules: past time-of-day rolls to tomorrow; DST spring-forward gap resolves to the jump instant; DST fall-back overlap resolves to the earlier occurrence
- [x] 3.7 Write unit tests for `TriggerResolver` covering: future time today, past time rolls to tomorrow, duration boundary values (1, 1440, 0, -1, 1441), spring-forward gap, fall-back overlap ← (verify: every scenario in specs/power-job-scheduling/spec.md for trigger validation and absolute-time resolution has a corresponding passing test, including both DST cases)

## 4. Persistence layer

- [x] 4.1 Define the Drift `Jobs` table in `lib/data/` with columns matching the `Job` entity, storing `targetInstantUtc` as a UTC timestamp and the trigger as a discriminator plus parameters
- [x] 4.2 Implement the Drift database class with schema version 1 and a `drift_flutter` connection that works on desktop, Android, and iOS
- [x] 4.3 Implement `JobDao` with: watch all jobs (streaming), watch active jobs by type, insert, update status, delete, and an atomic `replaceActiveJobOfType` transaction
- [x] 4.4 Implement mappers between Drift rows and domain `Job` entities
- [x] 4.5 Define `JobRepository` interface in `lib/domain/` and its Drift-backed implementation in `lib/data/`
- [x] 4.6 Run `dart run build_runner build` and commit the generated Drift code
- [x] 4.7 Write unit tests for `JobDao` against an in-memory Drift database, including the atomic replace transaction ← (verify: `replaceActiveJobOfType` leaves exactly one active job of that type; no intermediate state with zero or two is observable)

## 5. Platform capability probe

- [x] 5.1 Define the `PlatformCapabilities` model in `lib/platform/` with at minimum `supportsPowerOff`, `keepAwakePersistsInBackground`, `supportsAutostart`, `requiresRuntimeSchedulingPermission` — constructible explicitly for tests
- [x] 5.2 Implement resolution of the real values per the verified matrix: power-off true on Windows/macOS/Linux and false on Android/iOS; autostart true on desktop only; background keep-awake false on iOS and conditional on Android
- [x] 5.3 Expose `PlatformCapabilities` as a Riverpod provider resolved once at startup
- [x] 5.4 Write unit tests constructing fake capability sets for each of the five platforms and asserting the values match the matrix in specs/platform-capability-probe/spec.md ← (verify: Android and iOS capability instances report `supportsPowerOff: false`; no production code outside this file branches on `Platform.isX` for these capabilities)

## 6. Wakelock controller

- [x] 6.1 Define a `WakelockController` interface in `lib/platform/` with acquire/release and a queryable held state
- [x] 6.2 Implement it over `wakelock_plus`, returning `Result` on failure
- [x] 6.3 Wire acquire on `keepAwake` job activation and release on completion, cancellation, and pause
- [x] 6.4 Implement re-assertion on app relaunch when a `keepAwake` job's window is still active, and no re-assertion when the window has elapsed
- [x] 6.5 Write unit tests with a fake `WakelockController` for: acquire on activation, release on each of completion/cancel/pause, re-assert on relaunch within window, no re-assert after window elapsed ← (verify: every scenario in specs/screen-wakelock/spec.md for acquisition and re-assertion has a passing test; no path leaves the wakelock held after a job ends)

## 7. Power-off executor

- [x] 7.1 Define the `PowerOffExecutor` interface in `lib/platform/` with `Future<Result<void>> powerOff()` and `bool get isSupported`
- [x] 7.2 Implement `WindowsPowerOffExecutor` invoking `shutdown /s /t 0` via `Process.run`, mapping `ERROR_PRIVILEGE_NOT_HELD` to `PowerOffPrivilegeDenied`
- [x] 7.3 Implement `MacOsPowerOffExecutor` invoking `osascript -e 'tell application "System Events" to shut down'`, mapping consent denial to `PowerOffConsentDenied`
- [x] 7.4 Implement `LinuxPowerOffExecutor` invoking `systemctl poweroff` via `Process.run`, mapping polkit denial to `PowerOffPolicyDenied`
- [x] 7.5 Implement `UnsupportedPowerOffExecutor` for Android and iOS returning `Failure(PowerOffUnsupported)` with `isSupported == false` — it must never invoke any shutdown API
- [x] 7.6 Implement `FakePowerOffExecutor` in `test/` that records invocations without shutting down
- [x] 7.7 Select the executor via the capability provider, never via inline platform checks in UI code
- [x] 7.8 Write unit tests asserting each executor's error mapping and that `UnsupportedPowerOffExecutor` makes no process call ← (verify: the full schedule-to-execute path runs in tests using `FakePowerOffExecutor` and never shuts down the test machine; each of the three platform-specific error variants is asserted)

## 8. Notification service

- [x] 8.1 Define a `NotificationService` interface in `lib/platform/` for immediate and scheduled (zoned) notifications, returning `Result`
- [x] 8.2 Implement it over `flutter_local_notifications` with `timezone`/`flutter_timezone` initialization for DST-correct zoned scheduling
- [x] 8.3 Implement Android runtime `POST_NOTIFICATIONS` request at the point of first notification scheduling, not at app launch
- [x] 8.4 Implement Android `canScheduleExactAlarms()` check with a settings-routing request for `SCHEDULE_EXACT_ALARM`, and graceful degradation to in-process timing on denial
- [x] 8.5 Implement the iOS 64-pending-notification cap: prioritize nearest-due, never drop the soonest-due job's notification
- [x] 8.6 Implement the mobile power-off reminder notification, explicitly labeled as a reminder
- [x] 8.7 Write unit tests with a fake notification service for scheduling, permission denial degradation, and the 64-cap prioritization ← (verify: on simulated permission denial the job is still created and still fires via in-process timer; the soonest-due notification is never the one dropped at the cap)

## 9. Job scheduler

- [x] 9.1 Implement `JobScheduler` in `lib/application/` holding a single in-process timer armed for the nearest pending job, re-arming after each fire
- [x] 9.2 Register OS-level scheduling (Android exact alarm / local notification) as a durability backstop alongside the in-process timer, with the database as the authoritative record
- [x] 9.3 Implement resume reconciliation: overdue `keepAwake` completes and releases the wakelock; overdue `powerOff` within 15 minutes proceeds to grace countdown; overdue `powerOff` beyond 15 minutes is marked overdue, does not execute, and notifies
- [x] 9.4 Implement system clock / timezone change detection that re-resolves `absoluteTime` job targets and notifies the user when a target moves, while leaving `duration` job targets unchanged
- [x] 9.5 Implement the one-active-job-per-type rule using the atomic replace transaction from 4.3
- [x] 9.6 Implement job pause (release resources, retain job), resume (recompute target from now for duration triggers), and cancel (release resources, stop foreground service if no jobs remain)
- [x] 9.7 Write unit tests with fake clock, fake executor, fake wakelock, and fake notifications covering: fire on target, all three overdue reconciliation branches, timezone change re-resolution, duration job unaffected by timezone change, pause/resume/cancel resource handling ← (verify: every scenario in specs/power-job-scheduling/spec.md for overdue reconciliation and clock/timezone change has a passing test; the long-overdue power-off case asserts the executor was NOT called)

## 10. Grace-period countdown before power-off

- [x] 10.1 Implement a 60-second grace-period countdown that runs when a `powerOff` job fires, before `PowerOffExecutor` is invoked
- [x] 10.2 Ensure the countdown duration is not configurable to zero and cannot be disabled
- [x] 10.3 Implement the Cancel action that aborts power-off, marks the job cancelled, and prevents any shutdown call
- [x] 10.4 Make Cancel the initially focused element and operable by keyboard
- [x] 10.5 On executor failure, set job status to failed and surface the specific `AppError` message immediately — never fail silently
- [x] 10.6 Write tests for: countdown precedes execution, cancel prevents execution, expiry triggers execution, failure sets failed status and surfaces the specific error ← (verify: no code path invokes `PowerOffExecutor.powerOff()` without first completing the 60-second countdown; failure never leaves the job in pending or completed)

## 11. Desktop background runtime

- [x] 11.1 Implement hide-to-tray using `window_manager` `setPreventClose(true)` and an `onWindowClose` handler calling `hide()`
- [x] 11.2 Implement the `tray_manager` tray icon with a menu exposing Show, and Quit
- [x] 11.3 Implement tray icon click to show and focus the main window
- [x] 11.4 Implement Quit using `windowManager.destroy()` — explicitly NOT `close()`, which `setPreventClose(true)` intercepts
- [x] 11.5 Verify active jobs keep running while the window is hidden to tray
- [x] 11.6 Implement the launch-at-startup toggle over `launch_at_startup`, defaulting to off, with registration and deregistration ← (verify: Quit actually terminates the process rather than hiding the window; a `keepAwake` job's wakelock stays held after hide-to-tray)

## 12. Android background runtime

- [x] 12.1 Configure `flutter_foreground_task` with `serviceTypes: [dataSync]` and a user-visible notification
- [x] 12.2 Add `AndroidManifest.xml` entries: `FOREGROUND_SERVICE`, `FOREGROUND_SERVICE_DATA_SYNC`, `POST_NOTIFICATIONS`, `SCHEDULE_EXACT_ALARM`, `RECEIVE_BOOT_COMPLETED`, and `android:foregroundServiceType="dataSync"` on the service
- [x] 12.3 Start the foreground service when a job becomes active and stop it when no active jobs remain
- [x] 12.4 Implement boot-completed re-registration of pending jobs
- [x] 12.5 Implement the Android 15 `dataSync` 6-hour/day cap handling: stop the service, mark the job degraded, notify the user — no silent job loss ← (verify: manifest declares both the permission and the matching `foregroundServiceType`, so `startForeground()` cannot throw `MissingForegroundServiceTypeException` on API 34+; service stops when the last job ends). Implemented behind the injectable `ForegroundServiceController` so the refusal path is unit-testable; the Dart layer cannot distinguish the cap from other start denials, so any refusal degrades the job rather than guessing the cause.

## 13. iOS background limitations

- [x] 13.1 Configure iOS notification permissions and `Info.plist` entries required by `flutter_local_notifications`
- [x] 13.2 Use scheduled local notifications as the primary background delivery mechanism on iOS (not `BGTaskScheduler`)
- [x] 13.3 Surface in the UI that iOS background timing is not guaranteed, that force-quitting stops delivery, and that jobs do not survive a device reboot without reopening the app

## 14. Riverpod wiring

- [x] 14.1 Define providers for the database, `JobRepository`, `PlatformCapabilities`, `WakelockController`, `PowerOffExecutor`, `NotificationService`, and `JobScheduler`
- [x] 14.2 Implement a job-list `AsyncNotifier`/`StreamProvider` backed by the Drift streaming query
- [x] 14.3 Implement a job-creation notifier handling validation, the replace-confirmation flow, and activation
- [x] 14.4 Ensure the scheduler and tray callbacks can read providers without a `BuildContext`

## 15. User interface

- [x] 15.1 Implement the app theme with WCAG 2.1 AA contrast (4.5:1 normal text, 3:1 large text and meaningful icons) and visible focus indicators on all focusable elements
- [x] 15.2 Implement the job creation screen allowing keep-awake only, power-off only, or both in one session, each with independent trigger configuration
- [x] 15.3 Implement trigger input widgets for indefinite, duration (minutes/hours), and absolute clock time, hiding indefinite for `powerOff`
- [x] 15.4 Implement capability-limitation notices shown BEFORE the Create action: mobile power-off substitution, iOS foreground-only keep-awake, macOS sleep-override limits, macOS Automation consent pre-explanation
- [x] 15.5 Implement the job list with type, trigger description, live countdown updating at least once per second, "indefinite" text for indefinite triggers, and status conveyed by text and/or icon independent of the countdown
- [x] 15.6 Label `powerOff` jobs as reminders on Android and iOS, visually and textually distinct from a real power-off
- [x] 15.7 Implement job detail view with pause, resume, and cancel actions (also available from the list)
- [x] 15.8 Implement the replace-confirmation dialog when creating a second job of an existing active type
- [x] 15.9 Implement all component states for job surfaces: loading, error, empty, active, disabled, and text-overflow handling
- [x] 15.10 Add `Semantics` labels to every button, input, and status indicator so screen readers announce job type, trigger, and status
- [x] 15.11 Ensure full keyboard operability: Tab order reaches every control, Enter/Space activate, and no action is mouse- or touch-only ← (verify: job creation can be completed end-to-end with keyboard alone; every scenario in specs/job-management-ui/spec.md for accessibility and capability notices is satisfiable by inspection of the built UI)

## 16. Widget tests

- [x] 16.1 Widget test: create a keep-awake job, a power-off job, and both together
- [x] 16.2 Widget test: validation errors shown for indefinite power-off, zero duration, and over-maximum duration
- [x] 16.3 Widget test: replace-confirmation dialog appears and confirming replaces atomically
- [x] 16.4 Widget test: pause, resume, and cancel from both list and detail views
- [x] 16.5 Widget test: grace-period countdown renders with Cancel focused, and cancelling aborts
- [x] 16.6 Widget test: with a fake Android/iOS capability set, power-off jobs render as labeled reminders and the limitation notice appears before Create ← (verify: mobile limitation notice is asserted present pre-creation, not post-hoc; no test relies on running on an actual mobile device)

## 17. Build verification and honest reporting

- [x] 17.1 Run `flutter analyze` and resolve all errors and warnings
- [x] 17.2 Run `flutter test` and confirm all unit and widget tests pass
- [ ] 17.3 Build and verify macOS desktop (`flutter build macos`) — BLOCKED on this machine: `xcode-select` points at Command Line Tools only, so `xcodebuild` is unavailable and CocoaPods/`macos/Podfile` were never generated. Requires a machine with full Xcode. Disclosed in README.md.
- [x] 17.4 Build and verify Android (`flutter build apk --debug`)
- [x] 17.5 Build web as a fast smoke surface (`flutter build web`) — not a shipping target
- [x] 17.6 Write `README.md` documenting the per-platform capability matrix, the Android/iOS power-off impossibility with its reason, the macOS TCC consent requirement, the Windows domain-policy caveat, the Android OEM battery-manager caveat, and the Android 15 6-hour cap
- [x] 17.7 Record explicitly in `README.md` that iOS was NOT built or verified (no Xcode on the build machine) and that Windows and Linux desktop were NOT compiled (cannot cross-compile from macOS) — do not claim verification for any platform that was not compiled ← (verify: analyze and test both clean; macOS and Android builds succeed; README states the unverified platforms honestly and does not overclaim)

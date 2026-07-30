# Weakup

A cross-platform Flutter app that keeps your screen awake and schedules power-off events on Windows, macOS, Linux, Android, and iOS.

## What it does

- **Keep-awake jobs**: prevent the screen from sleeping for a set duration or indefinitely.
- **Power-off jobs**: schedule a system shutdown at a specific clock time or after a countdown, with a mandatory 60-second grace-period countdown before any irreversible action.

## Platform capability matrix

| Capability | Windows | macOS | Linux | Android | iOS |
|---|---|---|---|---|---|
| Power off system | Yes | Yes | Yes | **No** | **No** |
| Keep-awake persists in background | Yes | Yes | Yes | Yes (foreground service) | **No** |
| Launch at startup | Yes | Yes | Yes | No | No |
| Exact-alarm scheduling | Yes | Yes | Yes | Yes (requires permission) | Approximate only |
| Background wakelock | Yes | Yes | Yes | Yes | Foreground only |

## Android and iOS power-off: hard OS constraint

Android and iOS do not expose any API that allows a third-party app to power off the device. This is an intentional OS security constraint documented by both Apple and Google, not a bug or limitation of this app. On these platforms:

- The `UnsupportedPowerOffExecutor` returns `Failure(PowerOffUnsupported)` immediately.
- No shutdown API is ever called.
- A reminder notification is sent instead so the user can power off manually.

There is no workaround and no plan to add one.

## macOS: TCC Automation consent

On macOS, the shutdown command is issued via `osascript` targeting `System Events`. The first time a power-off job fires, macOS will present a TCC (Transparency, Consent, and Control) dialog asking whether Weakup may control System Events. If you deny this prompt, the job will fail with `PowerOffConsentDenied`. To re-enable it, go to System Settings > Privacy & Security > Automation.

The entitlements files (`DebugProfile.entitlements` and `Release.entitlements`) include `com.apple.security.automation.apple-events` to allow this consent to be granted.

## Windows: domain policy caveat

On domain-joined Windows machines, Group Policy may prevent standard user accounts from initiating a shutdown via `shutdown /s /t 0`. If this applies to you, the job will fail with `PowerOffPrivilegeDenied`. The specific policy is `SE_SHUTDOWN_PRIVILEGE`. Contact your system administrator to resolve this.

## Android: OEM battery-manager caveat

Some Android OEMs (Xiaomi, Huawei, Samsung, etc.) ship aggressive battery-manager software that kills background services, including foreground services, to extend battery life. On affected devices, a long-running keep-awake job or a power-off job scheduled far in the future may not fire while the screen is off. Weakup uses `flutter_foreground_task` with a user-visible persistent notification to reduce the likelihood of being killed, but OEM battery management is outside Flutter's control. If reliability matters, add Weakup to your OEM's battery-optimization exception list.

## Android 15: foreground service data-sync 6-hour cap

Android 15 (API 35) introduces a 6-hour-per-day cap for foreground services of type `dataSync`. When the daily cap is exhausted, the foreground service will be stopped automatically by the OS. Weakup handles this by:

1. Marking affected jobs as `degraded`.
2. Sending a notification informing the user.
3. Never silently losing a job.

The `AndroidManifest.xml` declares both the `FOREGROUND_SERVICE_DATA_SYNC` permission and `android:foregroundServiceType="dataSync"` on the service element. Without both declarations, `startForeground()` throws `MissingForegroundServiceTypeException` on API 34+.

## Build status

| Platform | Status |
|---|---|
| macOS | NOT verified — no full Xcode installation on this build machine (`xcode-select` points to Command Line Tools only, which is insufficient for `xcodebuild`) |
| Android (debug APK) | Built and verified: `flutter build apk --debug` succeeds |
| Web | Built and verified: `flutter build web` succeeds |
| Windows | NOT compiled — cross-compilation from macOS to Windows is not supported by Flutter |
| Linux | NOT compiled — cross-compilation from macOS to Linux is not supported by Flutter |
| iOS | NOT built or verified — no Xcode on the build machine |

`flutter analyze` reports zero issues. `flutter test` reports 100 passing tests, zero failures.

## Architecture

```
lib/
  core/         Result<T>, AppError sealed hierarchy
  domain/       Job entity, TriggerSpec, JobRepository interface, TriggerResolver
  data/         Drift tables, DAO, JobRepository implementation, mappers
  platform/     PlatformCapabilities, WakelockController, PowerOffExecutor, NotificationService
  application/  JobScheduler, Riverpod providers
  ui/           Screens, widgets, theme
test/
  core/         Result unit tests
  domain/       TriggerResolver unit tests (including DST spring-forward and fall-back cases)
  data/         JobDao unit tests (in-memory Drift, atomic replace)
  application/  JobScheduler unit tests (fake executor, fake wakelock, overdue branches)
  platform/     Fake implementations: FakePowerOffExecutor, FakeNotificationService, FakeWakelockController
  ui/           Widget tests (CreateJobScreen validation, GracePeriodScreen cancel)
```

Key design decisions:

- `targetInstantUtc` is the source of truth. No decrementing counter is stored; remaining time is computed from `targetInstantUtc - now` at read time.
- `PowerOffExecutor` is injectable. Tests always use `FakePowerOffExecutor` to avoid shutting down the machine.
- Overdue power-off beyond 15 minutes is dropped (marked `overdue`) to prevent a stale intent from executing an irreversible action after the user forgot about it.
- The 60-second grace-period countdown is mandatory and non-configurable. No code path reaches `PowerOffExecutor.powerOff()` without completing it.
- `@DataClassName('JobRow')` is required on the Drift `Jobs` table to avoid a name collision between the Drift-generated `Job` class and the domain `Job` entity.
- `UnsupportedPowerOffExecutor` returns `Failure(PowerOffUnsupported)` and never invokes any OS command. This is the only executor used on Android and iOS.

## Running locally

```sh
flutter pub get
dart run build_runner build --delete-conflicting-outputs
flutter run -d macos     # macOS desktop (requires full Xcode)
flutter run -d android   # Android device or emulator
flutter test             # all tests
flutter analyze          # static analysis
flutter build web        # web smoke build
```

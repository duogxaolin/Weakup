## Context

Greenfield project. The repository contains only `.claude/` and `openspec/` — no existing code, conventions, or patterns to follow, so every architectural choice here is a first decision rather than an adaptation.

The application must deliver two device-power behaviors (keep-screen-awake, scheduled power-off) across five operating systems with sharply different capabilities. The dominant design force is not UI or data modelling — it is that **the same user-facing feature is fully available on three platforms and categorically impossible on two**. Every structural decision below flows from containing that asymmetry in one place instead of letting it leak across the codebase.

### Verified platform capability matrix

| Platform | Keep-awake | Keep-awake in background | Power-off | Autostart |
| --- | --- | --- | --- | --- |
| Windows | Yes | Yes (tray process) | Yes, unprivileged¹ | Yes (registry Run key) |
| macOS | Yes² | Yes | Yes, after TCC consent³ | Yes (LaunchAtLogin) |
| Linux | Yes | Yes | Yes, unprivileged⁴ | Yes (`~/.config/autostart`) |
| Android | Yes | Foreground service only | **Impossible⁵** | Boot receiver re-registration |
| iOS | Foreground only⁶ | **No** | **Impossible⁷** | No |

1. `shutdown /s /t 0`. Standard `Users` hold `SeShutdownPrivilege` by default on client editions — no UAC prompt. Domain policy commonly strips it → `ERROR_PRIVILEGE_NOT_HELD`.
2. `IOPMAssertionCreateWithName` with `kIOPMAssertionTypeNoDisplaySleep`. Cannot block user-initiated sleep, lid close, thermal shutdown, or low-battery shutdown.
3. `osascript -e 'tell application "System Events" to shut down'`. Requires a one-time TCC Automation consent dialog that cannot be pre-granted or bypassed.
4. `systemctl poweroff` — polkit action `org.freedesktop.login1.power-off` ships `allow_active=yes` upstream, so the active local session succeeds without a password.
5. No public power-off API exists in the Android SDK. Enterprise device-owner apps get only `DevicePolicyManager.reboot()`; there is no `shutdown()`/`powerOff()`. True power-off needs a platform-signed system app or root.
6. `UIApplication.isIdleTimerDisabled` has zero effect once the app backgrounds.
7. No public or private API. MDM `ShutDownDevice` is server-initiated and requires Supervised enrollment; App Review rejects any app attempting this.

### Toolchain state on the development machine

Flutter 3.44.8 / Dart 3.12.2, Android SDK 36 with JDK 21, and Chrome are installed and verified. **Xcode is not installed** (CommandLineTools only). Consequently macOS desktop, Android, and web are compilable and verifiable here; **iOS cannot be built**, and Windows/Linux desktop cannot be compiled on macOS at all.

## Goals / Non-Goals

**Goals:**

- One Flutter codebase producing all five targets, with platform divergence isolated behind interfaces rather than scattered `Platform.isX` checks.
- Jobs survive app restart and — where the OS allows — device reboot, because a scheduler that forgets its work when the process dies is not a scheduler.
- Absolute-time triggers fire at the correct wall-clock instant across DST transitions and timezone changes.
- Power-off is always cancellable during a grace period before the irreversible command runs.
- Where a capability is unavailable, the UI says so in plain language and offers the best available substitute. No dead controls, no silent no-ops.
- Power-off execution is injectable so the full scheduling path is testable without shutting down the test machine.

**Non-Goals:**

- Rooted-Android or jailbroken-iOS power-off paths. Out of scope permanently, not deferred.
- MDM/enterprise device-owner provisioning to unlock Android reboot.
- Remote or cross-device control, accounts, sync, or any network service. The app is entirely local.
- Recurring/repeating jobs (daily, weekdays). The specified triggers are one-shot; recurrence is a future change.
- Sleep/hibernate/lock/restart as separate actions. Only power-off is in scope.
- Preventing *system* sleep independently of *display* sleep. Only the display assertion is specified.
- Web as a shipping target. It stays compilable as a fast test surface, but neither feature works meaningfully in a browser.

## Decisions

### D1: Flutter over Tauri, Electron, .NET MAUI, or React Native

Flutter is the only mature option that reaches all five targets from one codebase **and** has production-grade desktop tray/window plugins. Alternatives considered: Tauri v2 (excellent desktop, mobile support still immature for background services); Electron (no mobile at all); React Native (desktop via community forks, weak and unstable); .NET MAUI (no Linux target); Kotlin Multiplatform (would require separate UI per desktop OS). Flutter's weakness — large binaries — is irrelevant for a utility app.

### D2: A capability probe is the single source of truth for platform divergence

`PlatformCapabilities` is resolved once at startup and exposed via Riverpod. It answers: can this platform power off? can keep-awake persist in the background? is autostart available? does scheduling need a runtime permission?

The UI and the scheduler both branch on **capability**, never on `Platform.isAndroid`. This is the core decision that keeps the Android/iOS power-off limitation from metastasizing into dozens of conditionals. It also makes the divergence directly unit-testable by injecting a fake capability set — otherwise the impossible-platform behavior could only be tested on the impossible platform.

Alternative rejected: inline platform checks at each call site. Cheaper initially, but the limitation appears in job creation, validation, list rendering, notification copy, tray menus, and settings — six places that would drift apart.

### D3: Power-off on Android/iOS degrades to a labeled reminder notification

The honest options were: (a) hide the power-off feature entirely on mobile, (b) show a disabled control, (c) schedule a reminder notification and state the limitation.

(c) is chosen. The user's underlying intent — "I want this device off at 23:00" — is still partly served by a reliable reminder, whereas (a) silently drops a requested feature and (b) is a dead end for the user. The distinction from a workaround is that the substitution is **named in the UI**: the job is labeled a reminder, not a power-off, and the reason is shown. `PowerOffExecutor` on mobile returns an explicit `PowerOffUnsupported` outcome that the notification layer handles — it never pretends to have shut down.

This is a conscious tradeoff with a stated limit: on mobile the device is **not** powered off, and the app cannot make it so.

### D4: `Result<T>` sealed classes, not exceptions, for all fallible operations

Every operation that can fail for environmental reasons — power-off execution, permission requests, DB access, notification scheduling — returns `Result<T>` (`Success` | `Failure` with a typed `AppError`). Exceptions are reserved for programmer errors (invariant violations), which should crash in debug.

Rationale: the failure modes here are numerous, expected, and each needs specific user-facing copy (`ERROR_PRIVILEGE_NOT_HELD` differs from macOS consent denial differs from exact-alarm refusal). Sealed results make the compiler enforce that each is handled; a `try`/`catch` at a boundary would collapse them into one generic "something failed" message, which is exactly the silent-failure outcome the requirements forbid.

### D5: Store the absolute target instant, never a remaining-seconds countdown

A job persists `targetInstantUtc` plus the trigger definition that produced it. Remaining time is always computed as `target − now`.

Storing a decrementing counter would be wrong the moment the process is suspended, the device sleeps, or the app is killed — precisely the conditions this app runs under. Absolute instants also make overdue detection trivial (`target < now` on resume) and survive restart with no reconstruction logic.

### D6: Timezone-aware resolution via `timezone` + `flutter_timezone`

Absolute clock-time triggers resolve through `TZDateTime` in the device's IANA zone, not naive `DateTime`. A "shut down at 02:30" job scheduled across a spring-forward transition would misfire by an hour with naive local time. `flutter_timezone` supplies the IANA zone name on all five platforms; `timezone` performs the arithmetic.

DST edge handling is specified explicitly: a target time that does not exist (spring-forward gap) resolves to the instant the clock jumps to; a target time that occurs twice (fall-back overlap) resolves to the **first** occurrence, so the job never fires later than the user expects.

### D7: Drift for persistence over Isar, Hive, or bare sqflite

Drift 2.34.3 is actively maintained (published days before this design), covers all five platforms plus web, and gives type-safe queries with streaming — a natural fit for a reactive job list. Isar's stable release is roughly three years old with v4 perpetually in pre-release; Hive is abandoned by its own README (`hive_ce` is a viable fork but is key-value only, a poor fit for queryable job records with timestamps and status). Bare `sqflite` works but means hand-written SQL and manual migration plumbing. Drift's `build_runner` step is the accepted cost.

### D8: Riverpod 3.x for state management

Async-first (`AsyncNotifier`, `StreamProvider`) which maps directly onto Drift's streaming queries; no `BuildContext` needed to read providers, which matters because the scheduler and tray callbacks run outside the widget tree. Provider is explicitly succeeded by Riverpod per its own author; Bloc's ceremony is disproportionate for a single-developer utility; GetX is not recommended for new projects and its v5 prerelease is stalled.

### D9: A single in-process `JobScheduler` owns all timers; OS scheduling is a durability backstop

One Dart-side scheduler holds a timer for the nearest pending job and re-arms on each fire. It is the primary mechanism while the process lives. OS-level scheduling (Android exact alarms, local notifications) exists to cover process death, not to replace the in-process timer.

Rationale: `Timer` cannot be trusted across suspension, and OS alarms cannot be trusted for precision or availability (Android 13+ may refuse exact alarms outright). Using both, with the DB as the authoritative record and reconciliation on every resume, means neither mechanism's failure loses a job.

### D10: Overdue jobs are reconciled on every app resume, per job type

On resume the scheduler compares each pending job's target to now.

- **Keep-awake, target passed** → the window is over; complete the job and release the wakelock. Nothing is owed.
- **Power-off, target passed within a bounded grace window (≤ 15 minutes)** → the user's intent is still current; run the normal grace-period countdown, giving them the cancel affordance.
- **Power-off, target passed by more than the grace window** → do **not** shut down. Mark the job overdue and notify. Powering off a machine the user has since returned to and is actively working on would destroy work; a stale intent must not trigger an irreversible action.

The asymmetry is deliberate: missing a keep-awake window is harmless, whereas a surprise shutdown is not.

### D11: One active job per type; creating a second replaces it after explicit confirmation

Two simultaneous power-off jobs are incoherent (which time wins?), and two keep-awake jobs are redundant. Rejecting the second outright would force the user to hunt for and cancel the old one. Instead, creation surfaces the existing job and requires confirmation to replace it. Replacement is atomic in a single transaction so a crash cannot leave zero or two active jobs.

### D12: Mandatory, non-skippable grace-period countdown before power-off

When a power-off job fires, a full-screen (desktop) or notification-plus-screen (mobile) countdown of 60 seconds runs with a prominent Cancel control before `PowerOffExecutor` is invoked. The countdown cannot be configured to zero.

Rationale: power-off is irreversible and can destroy unsaved work. The countdown is the only defense against a mis-set time, and making it non-skippable is a deliberate refusal to let the user disable their own safety net.

### D13: `PowerOffExecutor` is an injectable interface

```
abstract interface class PowerOffExecutor {
  Future<Result<void>> powerOff();
  bool get isSupported;
}
```

Implementations: `WindowsPowerOffExecutor` (`shutdown /s /t 0`), `MacOsPowerOffExecutor` (`osascript`), `LinuxPowerOffExecutor` (`systemctl poweroff`), `UnsupportedPowerOffExecutor` (Android/iOS, returns `PowerOffUnsupported`), and `FakePowerOffExecutor` for tests.

Without this seam the scheduling path could not be tested — every test run would shut down the machine. Desktop implementations shell out via `Process.run`; the `dbus` package is not used, since a one-shot `systemctl poweroff` needs no D-Bus client lifecycle, signal subscriptions, or property monitoring.

### D14: Desktop hide-to-tray, with `destroy()` for quit

`windowManager.setPreventClose(true)` plus an `onWindowClose` handler that calls `hide()`. The tray menu's Quit item must call `windowManager.destroy()` — **not** `close()`, because `setPreventClose(true)` also intercepts programmatic `close()` and would make Quit a no-op. This is a documented plugin behavior worth recording, as it is a non-obvious trap.

### D15: Android foreground service only while a job is pending

`flutter_foreground_task` with `serviceTypes: [dataSync]`, plus the matching manifest `android:foregroundServiceType` and `FOREGROUND_SERVICE_DATA_SYNC` permission (mandatory on API 34+, or `startForeground()` throws `MissingForegroundServiceTypeException`). The service starts when a job becomes pending and stops when none remain, rather than running permanently, to limit battery cost and Play Store policy exposure.

Android 15's 6-hour/day `dataSync` cap is handled explicitly: the service stops at the cap, the job is marked degraded, and the user is notified — the app does not silently lose the job.

### D16: Permissions are requested lazily at the point of need, with graceful denial

`SCHEDULE_EXACT_ALARM` (denied by default on Android 13+) and `POST_NOTIFICATIONS` are requested when the user first creates a job needing them, not at first launch. `canScheduleExactAlarms()` is checked before every exact schedule. On denial the job is still created and still fires via the in-process timer while the app lives — the user is told that timing may drift if the app is killed. Denial degrades precision, never functionality.

### D17: Layered architecture

```
lib/
  core/          Result, AppError, typedefs, extensions
  data/          Drift database, tables, DAOs, mappers
  domain/        Job entity, TriggerSpec, resolution logic, repository interface
  platform/      PlatformCapabilities, PowerOffExecutor, WakelockController,
                 NotificationService, BackgroundRuntime  (interfaces + per-OS impls)
  application/   JobScheduler, Riverpod providers/notifiers
  ui/            screens, widgets, theme
```

Dependencies point inward: `ui` → `application` → `domain` ← `data`, `platform`. `domain` holds the trigger-resolution logic as pure functions with no Flutter or plugin imports, which is what makes the DST, past-time, and overdue cases testable without a device.

## Risks / Trade-offs

**Mobile power-off cannot work → mitigated, not solved.** The reminder-notification substitution serves part of the intent and the UI states the limitation plainly. Residual risk: users may still perceive the mobile app as incomplete. Accepted — the alternative is misleading them.

**iOS keep-awake dies on backgrounding → surfaced in-app.** The iOS job screen states that the screen stays awake only while the app is open and in the foreground. No workaround exists.

**iOS delivers no reliable background execution → notifications only.** `BGTaskScheduler` offers no timing guarantee and does not run after a user force-kill. Scheduled local notifications are the most reliable available mechanism, bounded by the 64-pending-notification queue limit, which the notification layer must respect when scheduling.

**Windows power-off may be blocked by domain policy → explicit actionable error.** `ERROR_PRIVILEGE_NOT_HELD` is detected and reported as "your organization's policy prevents this app from shutting down the PC," not a generic failure.

**macOS TCC consent may be denied → pre-explained and recoverable.** The UI explains the upcoming system dialog before triggering it, and on denial shows the exact System Settings path to re-enable. Cannot be bypassed by design.

**Aggressive OEM Android battery managers may kill the service → documented.** Xiaomi/Huawei/OnePlus/Samsung ROMs can stop the foreground service or block boot re-registration after a force-stop. Mitigation is a battery-optimization exemption prompt and honest documentation; the app cannot override vendor ROM behavior.

**Three of five platforms cannot be verified on this machine → stated, never masked.** iOS (no Xcode), Windows and Linux (cannot cross-compile from macOS). Their code is written to spec and reviewed, but will be reported as **unverified**, and no task will claim otherwise. This is a real gap in confidence, not a formality.

**Grace-period countdown lengthens every power-off by 60 s → accepted deliberately.** The safety of a cancellable irreversible action outweighs punctuality; users wanting exact-second shutdown are not served, and that is the correct trade.

**System clock changes can invalidate pending schedules → reconciled on resume.** Duration-derived targets are anchored to a stored absolute instant; on a detected clock or timezone change, absolute-time jobs are re-resolved against the new zone and the user is notified if a target moved.

**Drift code generation adds a build step → accepted.** `build_runner` must run after schema edits. The type-safe queries and migrations are worth the friction versus hand-written SQL.

## Migration Plan

Not applicable — greenfield project with no existing users, data, or deployment. No rollback path is needed; the change is additive from an empty repository.

## Open Questions

None blocking. All platform capabilities were verified against official documentation, all package versions confirmed on pub.dev, and every requirement decision listed above is settled. The only unresolved item is environmental rather than a design question: iOS, Windows, and Linux builds require machines this project does not have, and are reported as unverified accordingly.

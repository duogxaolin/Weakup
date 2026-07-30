## Why

Users need a single tool to control two related device-power behaviors — keeping the screen awake during long unattended tasks (downloads, renders, presentations, reading) and powering the machine off at a chosen time — without hunting through per-OS system settings or leaving a laptop running all night. No project exists yet; this change creates the application from scratch.

The two behaviors are naturally paired: "keep the screen on while this job runs, then shut down when it finishes." Today that requires two separate utilities on desktop and has no equivalent on mobile.

## What Changes

- **New Flutter application** ("Weakup") scaffolded from scratch in an empty repository, targeting Windows, macOS, Linux, Android, and iOS from one codebase.
- **Keep-screen-awake jobs** with three trigger modes: indefinite (until manually stopped), a duration (e.g. 2 hours), or an absolute clock time (e.g. until 23:30).
- **Power-off jobs** with two trigger modes: a duration or an absolute clock time. Indefinite is rejected as meaningless for power-off.
- **Independent concurrent jobs** — a keep-awake job and a power-off job may be active at the same time, or either alone.
- **Job persistence** in a local database so scheduled jobs survive app restart, and are re-registered after device reboot where the OS permits.
- **Background operation** — desktop apps hide to a system tray/menu-bar icon instead of quitting; Android uses a foreground service to stay alive while a job is pending.
- **Honest per-platform capability reporting** — a runtime capability probe drives the UI so unavailable actions are explained, never silently broken.
- **Power-off degrades to a scheduled reminder notification on Android and iOS.** Both operating systems have no API that lets an app power the device off (see Impact). Rather than shipping a dead button, the mobile builds schedule a clearly-labeled "time to power off your device" notification and state plainly in the UI that the OS does not permit apps to do this automatically. This is a conscious tradeoff forced by the platforms, not a deferred implementation.
- **Local notifications** for job-fired, job-completed, and power-off-imminent events.
- **Mandatory grace-period countdown before any power-off executes**, with a visible cancel control, so an unattended shutdown can always be aborted.

## Capabilities

### New Capabilities

- `power-job-scheduling`: The job domain model and trigger semantics — job types (keep-awake, power-off), trigger kinds (indefinite, duration, absolute clock time), timezone-aware target-instant resolution, validation rules, persistence, restart/reboot recovery, and overdue-job catch-up behavior.
- `screen-wakelock`: Acquiring and releasing the OS screen-awake assertion for the lifetime of an active keep-awake job, including re-assertion after app relaunch and the documented foreground-only limits on mobile.
- `device-power-off`: Executing (or, where impossible, substituting a reminder for) a scheduled device power-off, including the pre-shutdown grace-period countdown, cancellation, and actionable failure reporting when the OS denies the request.
- `platform-capability-probe`: A single runtime source of truth for what the current platform actually supports, so UI and scheduling logic branch on capability rather than scattered platform checks.
- `background-runtime`: Keeping the app alive and its jobs firing while not in the foreground — desktop tray/hide-to-tray with correct quit semantics, optional launch-at-startup, and the Android foreground service.
- `job-management-ui`: The user-facing surfaces for creating, viewing, editing, pausing, cancelling, and completing jobs, including countdown display, all component states, and accessibility requirements.

### Modified Capabilities

None. This is a greenfield project with no existing specs.

## Impact

**New codebase.** The repository currently contains only `.claude/` and `openspec/`. This change creates the entire Flutter project: `pubspec.yaml`, per-platform runner directories, `lib/` source tree, and `test/`.

**Dependencies added** (versions verified on pub.dev, July 2026):

| Package | Version | Purpose |
| --- | --- | --- |
| `flutter_riverpod` | ^3.4.2 | State management |
| `drift`, `drift_flutter`, `sqlite3_flutter_libs` | ^2.34.3 | Job persistence |
| `wakelock_plus` | ^1.7.0 | Screen-awake assertion, all platforms |
| `tray_manager` | ^0.5.3 | Desktop tray icon |
| `window_manager` | ^0.5.2 | Desktop hide-to-tray window control |
| `launch_at_startup` | ^0.5.1 | Desktop autostart |
| `flutter_foreground_task` | ^10.0.0 | Android foreground service |
| `flutter_local_notifications` | ^22.2.0 | Scheduled and immediate notifications |
| `timezone`, `flutter_timezone` | ^0.11.1 / ^5.1.0 | DST-correct absolute-time scheduling |
| `build_runner`, `drift_dev` | dev | Code generation |

**Hard OS constraints that shape scope** (each verified against official platform documentation):

- **Android power-off is impossible.** The public SDK exposes no power-off API. Even enterprise device-owner apps get only `DevicePolicyManager.reboot()`; there is no `shutdown()` or `powerOff()`. A true power-off requires a platform-signed system app or root.
- **iOS power-off is impossible.** No public or private API exists. The MDM `ShutDownDevice` command is server-initiated and requires Supervised enrollment. Any App Store submission attempting this is rejected.
- **Mobile keep-awake is foreground-only.** iOS `isIdleTimerDisabled` has zero effect once the app backgrounds. Android's supported `FLAG_KEEP_SCREEN_ON` is tied to a visible Activity; the deprecated background wake-lock constants must not be used.
- **Windows power-off can fail on managed machines.** Standard users hold `SeShutdownPrivilege` by default on client editions, but domain policy commonly strips it, yielding `ERROR_PRIVILEGE_NOT_HELD`.
- **macOS power-off requires one-time user consent.** The TCC Automation prompt for System Events cannot be pre-granted or bypassed; if denied, the feature stays broken until re-enabled in System Settings.
- **Android 13+ denies exact alarms by default.** `SCHEDULE_EXACT_ALARM` must be requested and can be refused; `POST_NOTIFICATIONS` is also runtime-gated.
- **Android 15+ caps `dataSync` foreground services at 6 hours/day.**
- **iOS caps pending notifications at 64.**

**Verification boundary.** This machine can compile and verify macOS desktop, Android, and web. Xcode is not installed (CommandLineTools only), so **iOS cannot be built or verified here**. Windows and Linux desktop **cannot be compiled on macOS**. Code for those three targets will be written to specification but must be labeled unverified — no claim of verification will be made for a platform that was not compiled.

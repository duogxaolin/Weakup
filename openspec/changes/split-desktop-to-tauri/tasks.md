## 1. Repository restructure (baseline)

- [x] 1.1 Initialize git and commit the Flutter baseline so every later step is reversible
- [x] 1.2 Extend `.gitignore` for Rust/Tauri artifacts (`target/`, `node_modules/`, `dist/`, `gen/schemas/`)
- [x] 1.3 Move the Flutter app into `mobile/` and re-verify from the new path (`flutter analyze` clean, 100 tests passing)
- [x] 1.4 Install `cargo-tauri` v2 and confirm the version
- [x] 1.5 Flag the superseded Flutter desktop runners (`mobile/macos`, `mobile/windows`, `mobile/linux`) for the user to delete — documented in the root README; not deleted

## 2. Prove the desktop build-and-run loop before porting any logic

- [x] 2.1 Scaffold `desktop/` as a Tauri v2 app with a plain HTML/CSS/TS front end (no framework)
- [x] 2.2 Configure `desktop/src-tauri/Cargo.toml` with the verified crate versions
- [x] 2.3 `cargo build` the empty app on macOS
- [x] 2.4 Launch the built macOS app and confirm a window appears — this gate must pass before any logic is ported
- [x] 2.5 Record the working build and run commands in `desktop/README.md`

## 3. `core/` — Result and error model

- [x] 3.1 `AppError` enum with variants mirroring the Dart `AppError` cases
- [x] 3.2 Standardize on `Result<T, AppError>` as the fallible-operation return type
- [x] 3.3 Unit-test error classification and display strings

## 4. `domain/` — job model and trigger resolution

- [x] 4.1 `JobType`, `TriggerKind`, `JobStatus` enums matching the Dart domain
- [x] 4.2 `TriggerSpec` with the validation rules (reject indefinite power-off; reject duration ≤ 0 or > 1440 minutes)
- [x] 4.3 `Job` struct with `target_instant_utc` as the persisted absolute target
- [x] 4.4 `TriggerResolver` using `chrono-tz`, including spring-forward gap and fall-back overlap handling
- [x] 4.5 Unit-test validation boundaries (0, negative, 1440, 1441 minutes)
- [x] 4.6 Unit-test DST gap and overlap resolution directly

## 5. `shared/testvectors/` — the cross-language contract

- [x] 5.1 Define the vector JSON format for target-instant resolution
- [x] 5.2 Define the vector JSON format for overdue reconciliation, making the 15-minute boundary inclusivity explicit
- [x] 5.3 Author the resolution vectors (future-today, past-rolls-tomorrow, spring-forward, fall-back, duration)
- [x] 5.4 Author the reconciliation vectors (keep-awake overdue, power-off 5 min, power-off 30 min, power-off exactly 15 min)
- [x] 5.5 Rust test harness that parses and executes every vector file
- [x] 5.6 Dart test harness in `mobile/test/` that parses and executes the same files
- [x] 5.7 Confirm both suites pass the same vectors, and deliberately break one rule in each language to confirm the vectors actually fail

## 6. `data/` — persistence

- [x] 6.1 Decide `rusqlite` versus `tauri-plugin-sql` and record the decision in `design.md`
- [x] 6.2 Schema for jobs, persisting the trigger definition alongside `target_instant_utc`
- [x] 6.3 `JobRepository` with insert, update status, update target, delete, list active
- [x] 6.4 Atomic replace-active-job-of-type in a single transaction
- [x] 6.5 Test that the replacement transaction never leaves zero or two active jobs of a type
- [x] 6.6 Test that a pending job round-trips with its target instant intact

## 7. `platform/` — power-off, per OS behind a trait

- [x] 7.1 `PowerOffExecutor` trait returning `Result<(), AppError>`
- [x] 7.2 macOS executor via `osascript` System Events, classifying consent denial
- [x] 7.3 Windows executor via `shutdown /s /t 0`, classifying `ERROR_PRIVILEGE_NOT_HELD`
- [x] 7.4 Linux executor via `systemctl poweroff`, classifying polkit denial
- [x] 7.5 Select the executor by `#[cfg(target_os = ...)]`; no other OS's executor in the binary
- [x] 7.6 Recording fake executor for tests — the test suite SHALL never bind a real executor
- [x] 7.7 Classify failures from captured exit status and stderr, not from a predicted permission state

## 8. `platform/` — keep-awake assertion, per OS

- [x] 8.1 Keep-awake trait with acquire and release
- [x] 8.2 macOS `IOPMAssertion` with `NoDisplaySleepAssertion`
- [x] 8.3 Windows `SetThreadExecutionState` with `ES_DISPLAY_REQUIRED`
- [x] 8.4 Linux freedesktop D-Bus idle inhibition with a `systemd-inhibit` fallback
- [x] 8.5 Release on job cancel, complete, and pause; assert no assertion outlives the last active job
- [x] 8.6 Report keep-awake unavailable honestly when no Linux mechanism is available

## 9. `platform/` — tray, window, autostart, notifications

- [x] 9.1 Tray via Tauri core `TrayIconBuilder`, icon supplied as data through the app handle
- [x] 9.2 Left-click tray restores the window (unminimize, show, focus)
- [x] 9.3 Tray menu with Quit via `AppHandle::exit`, warning and confirming when a job is active
- [x] 9.4 Intercept window close to hide instead of terminating
- [x] 9.5 Autostart via `tauri-plugin-autostart`, off by default
- [x] 9.6 Querying autostart state on a fresh install returns disabled and does not error
- [x] 9.7 Notifications via `tauri-plugin-notification`
- [x] 9.8 Every init step degrades independently: log, mark capability unavailable, return `Ok(())` — startup must never abort
- [x] 9.9 Test that a simulated tray init failure still yields a successful setup result

## 10. `application/` — the scheduler

- [x] 10.1 `JobScheduler` on a `tokio` runtime as the single owner of all timers and the only writer of job state
- [x] 10.2 Mandatory non-skippable 60-second grace countdown before any power-off, with cancel
- [x] 10.3 Assert no code path reaches power-off without the grace period
- [x] 10.4 Overdue reconciliation on startup and on resume from suspension, per job type
- [x] 10.5 Test that a power-off overdue by more than 15 minutes does NOT invoke the executor
- [x] 10.6 Test that a power-off overdue within tolerance proceeds through the grace period
- [x] 10.7 Recompute remaining time from the absolute target after machine sleep; never trust a running timer
- [x] 10.8 Re-resolve absolute-time jobs on timezone change; leave duration jobs unchanged
- [x] 10.9 One active job per type, replaced only after explicit confirmation
- [x] 10.10 Persist and report power-off failures; never fail silently

## 11. `commands/` — the IPC surface

- [x] 11.1 Commands for create, list, pause, resume, cancel, and settings
- [x] 11.2 Command returning the resolved target instant so the web view never resolves triggers itself
- [x] 11.3 Command exposing current capability state including degraded components
- [x] 11.4 Confirm no command executes power-off directly, bypassing the grace period
- [x] 11.5 Command to cancel an in-progress grace countdown

## 12. Desktop UI

- [x] 12.1 Job creation form supporting either or both job types independently — two independent fieldsets, submitted in one pass; `inert` keeps an unwanted job's fields out of the tab order
- [x] 12.2 Job list with countdown derived from the absolute target instant — never a decremented counter; clock skew between Rust and the web view is subtracted out from the target/remaining pair
- [x] 12.3 Grace-period countdown with a prominent cancel control — sticky banner, `role="alert"`, focus moves to the cancel button when it appears
- [x] 12.4 Degraded-capability messages in plain language with their consequences — Rust's reason plus what it costs the user; essential capabilities say they are essential
- [x] 12.5 Full keyboard operability for every control — native controls only, asserted by `wiring.test.js` (no positive tabindex, no hand-set `tabIndex`, no suppressed outline)
- [x] 12.6 Accessible labels and AA contrast — every pair measured, lowest text pair 6.4:1; no colour carries meaning alone

## 13. macOS packaging and consent

- [x] 13.1 Declare `NSAppleEventsUsageDescription` — via `src-tauri/Info.plist`, which Tauri merges into the bundle; there is no config key for arbitrary plist entries
- [x] 13.2 Confirm the App Sandbox is not enabled for the macOS build — the release binary has no entitlements and the bundle has no `com.apple.security.app-sandbox` key
- [x] 13.3 Verify the built bundle's `Info.plist` contains a non-empty usage description — confirmed with `PlistBuddy` against `Weakup.app/Contents/Info.plist`
- [x] 13.4 Generate real tray and app icons, replacing the Tauri placeholder — one regenerable 1024 px source produces the `.png`, `.ico`, and `.icns` sets
- [x] 13.5 Resolve the open question: an adhoc-signed app with the usage description prompts rather than terminating — confirmed with a harmless real Apple Event to the same System Events target; tccd attributed `osascript` to the fresh probe bundle, created a `kTCCServiceAppleEvents` decision, and the process resumed successfully

## 14. Verification

- [x] 14.1 `cargo test` — 218 host tests and 6 shared-vector harness tests pass
- [x] 14.2 `cargo clippy --all-targets -- -D warnings` clean
- [x] 14.3 Release `.app` built for macOS and launch-smoked past the former startup crash, reporting `BOOT_OK visible=true size=520x680`
- [ ] 14.4 Exercise tray, hide-to-tray, restore, autostart toggle, and grace-period cancel by hand — blocked in this session because the automation shell has neither macOS Accessibility nor event-posting permission; no substitute is claimed as a manual test
- [x] 14.5 Windows adapter source passes `cargo check --target aarch64-pc-windows-msvc --all-targets` through `platform-check` — type-check only, not a build or run
- [x] 14.6 Linux adapter source passes `cargo check --target aarch64-unknown-linux-gnu --all-targets` through `platform-check` — type-check only, not a build or run
- [x] 14.7 `flutter analyze` clean and `flutter test` passes 107 tests in `mobile/`
- [x] 14.8 Confirm no test binds or invokes a real power-off executor; scheduler/grace tests use `FakePowerOffExecutor`

## 15. Documentation

- [x] 15.1 Root `README.md` describes the three-folder layout and which app owns which platforms
- [x] 15.2 Verification table states exactly what was compiled and run versus type-checked versus untouched
- [x] 15.3 Shared-vector contract requires a vector executed by both suites for every new shared rule
- [x] 15.4 Known Rust/Dart scheduling duplication and the rejected FFI trade-off are stated plainly

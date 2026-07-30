## 1. Repository restructure (baseline)

- [x] 1.1 Initialize git and commit the Flutter baseline so every later step is reversible
- [x] 1.2 Extend `.gitignore` for Rust/Tauri artifacts (`target/`, `node_modules/`, `dist/`, `gen/schemas/`)
- [x] 1.3 Move the Flutter app into `mobile/` and re-verify from the new path (`flutter analyze` clean, 100 tests passing)
- [x] 1.4 Install `cargo-tauri` v2 and confirm the version
- [ ] 1.5 Flag the superseded Flutter desktop runners (`mobile/macos`, `mobile/windows`, `mobile/linux`) for the user to delete — do not delete them

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

- [ ] 8.1 Keep-awake trait with acquire and release
- [ ] 8.2 macOS `IOPMAssertion` with `NoDisplaySleepAssertion`
- [ ] 8.3 Windows `SetThreadExecutionState` with `ES_DISPLAY_REQUIRED`
- [ ] 8.4 Linux freedesktop D-Bus idle inhibition with a `systemd-inhibit` fallback
- [ ] 8.5 Release on job cancel, complete, and pause; assert no assertion outlives the last active job
- [ ] 8.6 Report keep-awake unavailable honestly when no Linux mechanism is available

## 9. `platform/` — tray, window, autostart, notifications

- [ ] 9.1 Tray via Tauri core `TrayIconBuilder`, icon supplied as data through the app handle
- [ ] 9.2 Left-click tray restores the window (unminimize, show, focus)
- [ ] 9.3 Tray menu with Quit via `AppHandle::exit`, warning and confirming when a job is active
- [ ] 9.4 Intercept window close to hide instead of terminating
- [ ] 9.5 Autostart via `tauri-plugin-autostart`, off by default
- [ ] 9.6 Querying autostart state on a fresh install returns disabled and does not error
- [ ] 9.7 Notifications via `tauri-plugin-notification`
- [ ] 9.8 Every init step degrades independently: log, mark capability unavailable, return `Ok(())` — startup must never abort
- [ ] 9.9 Test that a simulated tray init failure still yields a successful setup result

## 10. `application/` — the scheduler

- [ ] 10.1 `JobScheduler` on a `tokio` runtime as the single owner of all timers and the only writer of job state
- [ ] 10.2 Mandatory non-skippable 60-second grace countdown before any power-off, with cancel
- [ ] 10.3 Assert no code path reaches power-off without the grace period
- [ ] 10.4 Overdue reconciliation on startup and on resume from suspension, per job type
- [ ] 10.5 Test that a power-off overdue by more than 15 minutes does NOT invoke the executor
- [ ] 10.6 Test that a power-off overdue within tolerance proceeds through the grace period
- [ ] 10.7 Recompute remaining time from the absolute target after machine sleep; never trust a running timer
- [ ] 10.8 Re-resolve absolute-time jobs on timezone change; leave duration jobs unchanged
- [ ] 10.9 One active job per type, replaced only after explicit confirmation
- [ ] 10.10 Persist and report power-off failures; never fail silently

## 11. `commands/` — the IPC surface

- [ ] 11.1 Commands for create, list, pause, resume, cancel, and settings
- [ ] 11.2 Command returning the resolved target instant so the web view never resolves triggers itself
- [ ] 11.3 Command exposing current capability state including degraded components
- [ ] 11.4 Confirm no command executes power-off directly, bypassing the grace period
- [ ] 11.5 Command to cancel an in-progress grace countdown

## 12. Desktop UI

- [ ] 12.1 Job creation form supporting either or both job types independently
- [ ] 12.2 Job list with countdown derived from the absolute target instant
- [ ] 12.3 Grace-period countdown with a prominent cancel control
- [ ] 12.4 Degraded-capability messages in plain language with their consequences
- [ ] 12.5 Full keyboard operability for every control
- [ ] 12.6 Accessible labels and AA contrast

## 13. macOS packaging and consent

- [x] 13.1 Declare `NSAppleEventsUsageDescription` — via `src-tauri/Info.plist`, which Tauri merges into the bundle; there is no config key for arbitrary plist entries
- [ ] 13.2 Confirm the App Sandbox is not enabled for the macOS build
- [ ] 13.3 Verify the built bundle's `Info.plist` contains a non-empty usage description
- [ ] 13.4 Generate real tray and app icons, replacing the 79-byte blank placeholder
- [ ] 13.5 Resolve the open question: does an unsigned app with the usage description prompt, or terminate?

## 14. Verification

- [ ] 14.1 `cargo test` — the full desktop suite passes
- [ ] 14.2 `cargo clippy` clean
- [ ] 14.3 `cargo build --release` for macOS
- [ ] 14.4 Launch the release macOS app and exercise tray, hide-to-tray, restore, autostart toggle, and grace-period cancel by hand
- [ ] 14.5 `cargo check --target aarch64-pc-windows-msvc` — type-check only, explicitly not a build
- [ ] 14.6 `cargo check --target aarch64-unknown-linux-gnu` — type-check only, explicitly not a build
- [ ] 14.7 Re-run `flutter analyze` and `flutter test` in `mobile/` to confirm the restructure left it intact
- [ ] 14.8 Confirm no test binds a real power-off executor

## 15. Documentation

- [ ] 15.1 Root `README.md` describing the three-folder layout and which app owns which platforms
- [ ] 15.2 Verification table stating exactly what was compiled and run versus type-checked versus untouched
- [ ] 15.3 Document the shared-vector contract and the requirement to add a vector for any new shared rule
- [ ] 15.4 State the known duplication of scheduling logic across two languages and why FFI was rejected

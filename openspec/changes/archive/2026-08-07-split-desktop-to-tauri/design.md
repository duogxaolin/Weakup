## Context

The Flutter implementation of this project is complete, archived, and passing 100 tests — but only Android and web were ever compiled. The desktop targets were written to specification and never executed, and an audit found four blocking defects in that never-run code (enumerated in `proposal.md`). Three of the four are the kind of mistake that only surfaces on first launch.

The reason desktop was never run is environmental: this machine has Command Line Tools but not Xcode, so `flutter build macos` fails at `xcrun: error: unable to find utility "xcodebuild"`. There is no path to running Flutter desktop code here.

### Toolchain state on the development machine (verified by execution, July 2026)

| Tool | Version | Verified how |
| --- | --- | --- |
| rustc / cargo | 1.95.0 | `rustc --version` |
| cargo-tauri | 2.11.4 | `cargo install tauri-cli --locked --version "^2"` completed |
| node / npm | 22.22.3 / 10.9.8 | `node --version` |
| clang | 21.0.0 | links a release binary successfully |
| Flutter / Dart | 3.44.8 / 3.12.2 | `flutter test` → 100 passing from `mobile/` |
| Rust targets installed | `aarch64-apple-darwin`, `aarch64-pc-windows-msvc`, `aarch64-unknown-linux-gnu` | `rustup target list --installed` |
| macOS packaging tools | `hdiutil`, `codesign`, `sips`, `iconutil`, `pkgbuild` all present | probed |
| Xcode | **absent** (CLT only) | `xcrun` fails on `xcodebuild` |

Crate versions confirmed on crates.io at authoring time: `tauri` 2.11.5, `tauri-plugin-autostart` 2.5.1, `tauri-plugin-notification` 2.3.3, `tauri-plugin-sql` 2.4.0, `chrono` 0.4.45, `chrono-tz` 0.10.4, `rusqlite` 0.40.1, `tokio` 1.53.1.

### The decisive asymmetry

Tauri needs only Command Line Tools to build and run a macOS app. Flutter desktop needs full Xcode. Everything else about this decision is secondary to that: one stack can be executed on the machine where it is being written, and the other cannot.

## Goals / Non-Goals

**Goals**

- Desktop (Windows, macOS, Linux) implemented in Rust + Tauri v2, with macOS actually built and launched during development rather than only at the end.
- Preserve every safety-critical scheduling rule already specified and tested, with no weakening: absolute target instants, the 15-minute overdue refusal, the mandatory 60-second grace countdown, one active job per type with atomic replacement.
- Confine OS divergence to one thin adapter layer, so three OSes share one scheduler.
- Bind the Rust and Dart implementations with shared test vectors so behavioral drift fails a test instead of reaching a user.
- Leave `mobile/` functionally untouched — it holds the only verified builds in the project.

**Non-Goals**

- Rewriting mobile in Tauri. Tauri v2's Android background-execution story is weak and its foreground-service support is near-DIY; the Flutter implementation already works and is tested.
- Verifying Windows or Linux on this machine. Not possible; Tauri does not cross-compile from macOS.
- iOS work of any kind. No Xcode.
- Eliminating logic duplication via FFI (see D3).
- Feature changes. This change is a re-platform of desktop, not new behavior.

## Decisions

### D1: Rust + Tauri v2 for desktop, Flutter retained for mobile

Two stacks, each where it is strongest. Tauri gives a buildable-and-runnable macOS target on a CLT-only machine and ships small native binaries; Flutter keeps the verified Android build and its foreground service.

Rejected: **fixing Flutter desktop in place.** The four defects are individually about an hour's work, but the fix would again be unverifiable on this machine — the same conditions that produced the defects would still hold. Rejected: **Electron** (large runtime, no advantage here). Rejected: **all-Tauri including mobile** (regresses working, tested Android background execution).

### D2: Three desktop OSes, one codebase, `#[cfg]` at the adapter boundary

The OSes differ in exactly three behaviors: power-off invocation, tray icon format, autostart mechanism. These live in `desktop/src-tauri/src/platform/` behind traits, selected by `#[cfg(target_os = ...)]`.

Rejected: **separate `desktop-win/`, `desktop-mac/`, `desktop-linux/` folders.** That triples the 928 lines of scheduling logic to vary three functions — the highest-risk code duplicated for the lowest-variance reason. The measured split is roughly 928 lines of OS-independent logic against three genuinely OS-specific operations; per-OS folders invert that ratio.

### D3: Shared JSON test vectors, not FFI, to bind the two implementations

`shared/testvectors/*.json` holds `(now, timezone, trigger) → expected target_instant_utc` cases plus overdue-reconciliation cases. Both the Rust and Dart suites parse and execute them. A rule changed in one language and not the other turns a suite red.

Rejected: **`flutter_rust_bridge` FFI** so the core exists once. It genuinely eliminates duplication, but adds a codegen step, a cross-language debugging boundary, and per-platform build complexity to share 928 lines. Revisit if the shared core grows past a few thousand lines or if vectors start catching real drift frequently.

Accepted cost, stated plainly: the scheduling logic exists twice. Vectors constrain behavior, not structure.

### D4: The grace-period countdown stays mandatory, non-skippable, and cancellable

Unchanged from the Flutter implementation and non-negotiable: no code path reaches power-off without a visible 60-second countdown carrying a cancel control. Re-specified here because it is being reimplemented in a new language, which is exactly when such a rule gets quietly dropped.

### D5: The 15-minute overdue tolerance refuses to power off

An overdue `power_off` job past tolerance is marked overdue and notified — never executed. A machine that was asleep and woke hours later must not shut down without warning. The Rust port keeps a test asserting the executor is **not** invoked.

### D6: `PowerOffExecutor` is a trait, and tests only ever use the fake

A real executor in a test would shut down the developer's machine. The trait is injected; the test suite binds a recording fake. This mirrors the Dart design and carries the same absolute prohibition.

### D7: `Result<T, AppError>` throughout — Rust's native idiom matches the existing design

The Dart implementation deliberately used `Result<T>` sealed classes rather than exceptions. Rust's `Result` is the same shape natively, so the port is direct: `AppError` as an enum with variants mirroring the Dart `AppError` cases.

### D8: Store the absolute target instant, never a countdown

`target_instant_utc` is persisted alongside the original trigger definition. Remaining time is always computed as `target − now` at display time. Sleep, suspend, and clock changes make a stored decrementing counter wrong.

### D9: `chrono-tz` for DST-correct absolute-time resolution

`chrono-tz` provides IANA rules. Spring-forward gaps resolve to the jump instant; fall-back overlaps resolve to the earlier occurrence — matching the spec and the Dart behavior, and covered by shared vectors.

### D10: Tray icon comes from Tauri core, which takes an icon, not a file extension

Verified against the Tauri v2 system-tray docs: `TrayIconBuilder::new().icon(app.default_window_icon().unwrap().clone())`. Because the icon is supplied as data through the app handle rather than a path string, the Windows `.ico` / other-OS `.png` divergence that broke the Flutter build cannot occur in the same way. Tray requires the `tray-icon` feature on the `tauri` crate.

### D11: Runtime init failures must not prevent the window from opening

Tauri's `.setup()` closure returns `Result`, and returning `Err` aborts startup — the same trap as the unguarded `desktopRuntime.init()`. So tray, autostart, and notification initialization each degrade: on failure, log, mark the capability unavailable, and return `Ok(())`. The window opens even with a broken tray, and the UI reports the degraded capability honestly.

### D12: macOS declares `NSAppleEventsUsageDescription`, and the build is not sandboxed by default

The Flutter build had the entitlement but no usage-description string, so macOS terminated the process instead of prompting. The Tauri release bundle is unsandboxed, and `src-tauri/Info.plist` supplies `NSAppleEventsUsageDescription` for Tauri to merge into the built bundle. Both conditions hold in the inspected `.app`, which makes the consent prompt appear and `PowerOffConsentDenied` reachable.

### D13: Capability resolution is compile-time where the OS is known, runtime where consent is involved

Whether power-off is *possible* is known at compile time per target. Whether it is *permitted* (macOS TCC consent, Windows `SeShutdownPrivilege`, Linux polkit) is only knowable at runtime, from the failure. So: `#[cfg]` decides which executor exists; runtime failure classification decides what the UI says.

### D14: Hide-to-tray on window close; quit only from the tray menu

Closing the window hides it so pending jobs keep running. Quit is explicit, from the tray menu, and warns when jobs are active.

### D15: One `JobScheduler` on a `tokio` runtime owns all timers

A single scheduler task holds the timers and is the only writer of job state, avoiding the shared-mutable-state class of bug. OS-level scheduling is a durability backstop, not the primary mechanism — matching the Flutter design.

### D16: Layered, inward-pointing modules

```
desktop/src-tauri/src/
├── core/        # Result, AppError — no deps
├── domain/      # Job, TriggerSpec, TriggerResolver — depends on core
├── data/        # SQLite JobRepository — depends on domain
├── platform/    # power-off, tray, autostart, wakelock — #[cfg] per OS
├── application/ # JobScheduler — orchestrates the above
└── commands/    # Tauri IPC surface for the web view
```

Dependencies point inward only. `domain/` and `core/` are pure and are the modules the shared vectors exercise.

### D17: `rusqlite` directly, not `tauri-plugin-sql`

Settles the open question. All persistence is driven from Rust: the scheduler is the only writer of job state (D15) and the web view never issues SQL, it calls IPC commands. `tauri-plugin-sql` exists to expose a database *to the front end*, which is precisely what this design does not want — it would open a second write path around the scheduler.

`rusqlite` with the `bundled` feature also compiles SQLite from source, so the three desktop OSes get one known version instead of whatever the host ships. That matters more here than usual, because two of the three targets cannot be run on this machine.

The cost is writing SQL by hand where Drift generated it on the Dart side. Accepted: the schema is one table.

### D18: The desktop `Job` carries `updated_at_utc` and `failure_message`

The initial port omitted both. Each is load-bearing rather than cosmetic:

- `failure_message` — `JobStatus::Failed` already exists, and a power-off can fail for reasons the user must be told apart: privileges, consent, or policy. Without the field, the status says "Failed" and the reason is lost at the process boundary.
- `updated_at_utc` — the job list orders by it, so without it "most recently touched" is unorderable. `created_at_utc` is not a substitute: the interesting event is the last state change.

Both mirror columns the Dart schema already has, which keeps the two row formats readable by either implementation.

## Risks / Trade-offs

**Logic now exists in two languages.** Mitigated by shared vectors (D3), not eliminated. This is the main cost of the split and it is accepted knowingly. It stays cheap while mobile is on hold and becomes a real tax if mobile development resumes.

**Windows and Linux stay unverified.** The exact condition that produced the four Flutter defects still applies to two of three desktop OSes. Mitigations: `#[cfg]` blocks are small and isolated (D2); `cargo check --target aarch64-pc-windows-msvc` type-checks Windows code without linking, which is more than the Flutter build ever offered; and every unverified target is labeled as such in the README.

**A rewrite discards working, tested code.** 100 passing tests and an archived spec exist for the Flutter implementation. Mitigated by the specs being implementation-independent — they are the asset, not the Dart code — and by `mobile/` retaining the verified Android and web builds untouched.

**`cargo check` is not `cargo build`.** Type-checking a Windows target on macOS catches type and API errors, not linker or WebView2 runtime problems. No verification claim will be made beyond what was actually compiled and run.

**Two toolchains to maintain.** Rust + node for desktop, Flutter for mobile. Accepted: the desktop toolchain is the one that works on this machine.

## Migration Plan

1. Commit the Flutter baseline before touching anything, so every step is reversible. **Done** — `da1b4c1`, then `4c14e3f` moving Flutter to `mobile/` with 100 tests re-verified from the new path.
2. Scaffold `desktop/` and get an empty window to launch on macOS **before** porting any logic — proving the build-and-run loop works first, since its absence caused the original defects.
3. Port inward-out: `core` → `domain` → shared vectors passing → `data` → `platform` → `application` → `commands` → UI. Run `cargo test` at every step.
4. Add `shared/testvectors/`, then wire the Dart suite to consume the same files so both sides are bound.
5. Build and launch the macOS app; exercise tray, hide-to-tray, autostart, and the grace countdown by hand. `cargo check` the Windows and Linux targets.
6. Update the README verification table to state precisely what was compiled and run, and what was not.

Flutter's now-superseded desktop runner directories (`mobile/macos`, `mobile/windows`, `mobile/linux`) are flagged for the user to delete. This change does not delete them.

## Open Questions

- ~~Does macOS still terminate rather than prompt if `NSAppleEventsUsageDescription` is present but the app is unsigned?~~ **Settled — it prompts.** An adhoc-signed probe with a fresh bundle identifier sent a harmless real Apple Event to System Events. `tccd` attributed `osascript` to the probe, created a `kTCCServiceAppleEvents` decision, and the process resumed successfully rather than terminating.
- ~~Is `tauri-plugin-sql` or direct `rusqlite` the better fit?~~ **Settled — D17: `rusqlite`.** A front-end SQL surface would open a second write path around the scheduler.
- Which Linux idle-inhibition mechanism to use for keep-awake (`systemd-inhibit` versus the D-Bus screensaver interface). Unverifiable here; will implement the D-Bus interface with a `systemd-inhibit` fallback and label it unverified.

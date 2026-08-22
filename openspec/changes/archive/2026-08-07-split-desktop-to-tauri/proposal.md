## Why

The Flutter desktop build cannot be verified on this machine and has accumulated four blocking defects that a passing test suite does not catch, because all four live in code that has never been compiled or executed:

1. `launchAtStartup.setup()` is never called, so `isEnabled()` throws `UnsupportedError` on all three desktops, leaving the Settings toggle permanently disabled.
2. `desktopRuntime.init()` is unguarded, so a tray failure throws before `runApp()` and no window ever opens.
3. The tray icon path is hardcoded to `.png`, but `tray_manager` requires `.ico` on Windows.
4. `macos/Runner/Info.plist` lacks `NSAppleEventsUsageDescription`, so macOS terminates the process on the Apple Events call rather than showing a consent prompt — making the `PowerOffConsentDenied` path unreachable.

This is the same failure mode four times over: components written correctly, never wired into a running app, and invisible to green tests. The root cause is not carelessness — it is that `flutter build macos` fails on this machine (`xcrun: error: unable to find utility "xcodebuild"`, Command Line Tools only, no Xcode), so desktop code cannot be run at all.

Rust + Tauri v2 was verified by actual compilation on this machine and needs only Command Line Tools. That turns macOS desktop from unverifiable into buildable and runnable, which is the only thing that reliably kills this defect class.

## What Changes

- **Repository becomes two apps plus a shared contract**, replacing one codebase that served five platforms:
  - `desktop/` — new Rust + Tauri v2 app for Windows, macOS, Linux.
  - `mobile/` — the existing Flutter app, moved unchanged, now scoped to Android and iOS.
  - `shared/testvectors/` — language-agnostic JSON scenarios both apps execute.
- **Desktop scheduling logic is ported to Rust**, preserving every behavioral rule already specified: absolute `target_instant_utc` persistence, DST-correct absolute-time resolution, the 15-minute overdue tolerance that refuses to power off, the mandatory non-skippable 60-second grace countdown, and one active job per type with atomic replacement.
- **Platform divergence is confined to one adapter layer.** The three desktop OSes differ in exactly three places — power-off command, tray icon format, autostart mechanism — expressed as `#[cfg(target_os = ...)]` in `desktop/src-tauri/src/platform/`, not as three separate codebases.
- **The four defects above are structurally avoided, not patched.** Tauri's tray is core (accepts icon bytes, no extension branching), `tauri-plugin-autostart` has no forgotten-`setup()` footgun, macOS builds are not sandboxed by default, and `init` failures are handled as `Result` at the setup boundary.
- **Shared test vectors bind the two implementations.** The 928 lines of pure scheduling logic now exist in two languages; drift between them is caught by both suites reading the same JSON fixtures rather than discovered in production.
- **Flutter desktop runner directories become dead code.** `mobile/macos`, `mobile/windows`, `mobile/linux` are superseded. They are flagged for the user to delete, not deleted here.

## Capabilities

### Modified Capabilities

- `power-job-scheduling`: Adds the shared-test-vector conformance requirement binding the Rust and Dart implementations to identical target-instant resolution. Existing scheduling requirements are unchanged in meaning and now apply to both implementations.
- `device-power-off`: Adds Rust desktop executor requirements and the macOS Apple Events consent-declaration requirement that the Flutter build was missing. Android/iOS impossibility is unchanged.
- `background-runtime`: Replaces desktop tray, hide-to-tray, and launch-at-startup requirements with Tauri equivalents, and adds the requirement that a failed runtime init must not prevent the window from opening. Android foreground-service requirements are unchanged.
- `platform-capability-probe`: Adds compile-time capability resolution for the Rust desktop build alongside the existing runtime probe.
- `screen-wakelock`: Adds the desktop keep-awake assertion via Rust. Mobile foreground-only limits are unchanged.
- `job-management-ui`: Adds the desktop web-view UI surface. Flutter mobile UI requirements are unchanged.

### New Capabilities

- `cross-implementation-conformance`: The shared test-vector contract — its format, which rules it covers, and the requirement that both implementations execute it in CI.

## Impact

**Repository restructure.** Flutter moves from the repository root into `mobile/` (already done and verified: `flutter analyze` clean, 100 tests passing from the new location). `desktop/` is created from scratch.

**Dependencies added** (desktop only; Rust 1.95.0, cargo 1.95.0, `cargo-tauri` 2.11.4 verified installed):

| Crate | Purpose |
| --- | --- |
| `tauri` v2 | App shell, tray (core), window management |
| `tauri-plugin-autostart` | Launch at startup, all three desktops |
| `tauri-plugin-notification` | Job and grace-period notifications |
| `tauri-plugin-sql` (SQLite) or `rusqlite` | Job persistence |
| `chrono`, `chrono-tz` | DST-correct timezone resolution |
| `tokio` | Timer and scheduling runtime |
| `serde`, `serde_json` | Persistence and test-vector parsing |

**What is preserved.** `mobile/` keeps the only two verified builds in this project (Android debug APK, web) and all 100 passing tests. Nothing about the Flutter app changes in this restructure beyond its path.

**What is knowingly given up.** The 928 lines of pure scheduling logic will exist in Rust and in Dart. Shared test vectors mitigate drift but do not eliminate duplication. FFI via `flutter_rust_bridge` would eliminate it and was rejected as disproportionate at this size (see design D3).

**Verification boundary — unchanged in spirit, improved in coverage.**

- **macOS desktop becomes verifiable here for the first time.** `cargo build --release` and execution were both confirmed on this machine.
- **Windows and Linux desktop remain unverifiable.** Windows needs MSVC + WebView2; Linux needs `libwebkit2gtk-4.1-dev` + `libayatana-appindicator3-dev`. Tauri does not cross-compile from macOS. Code for both is written to specification and labeled unverified.
- **iOS remains unverifiable** (no Xcode) and is out of scope for this change.
- No verification claim will be made for any target not actually compiled.

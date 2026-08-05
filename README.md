# Weakup

Weakup keeps a display awake and schedules power-off jobs. A job may run indefinitely, for a duration, or until a local clock time; keep-awake and power-off jobs can be used separately or together.

Power-off is always guarded by a visible, cancellable 60-second grace period. A power-off job that is more than 15 minutes overdue is marked overdue and **does not run**.

## Repository layout

```text
desktop/            Rust + Tauri v2 app for Windows, macOS, and Linux
mobile/             Flutter app for Android and iOS
shared/testvectors/ JSON cases executed by both implementations
```

The platform ownership is deliberate:

- **Desktop — Windows, macOS, Linux:** Rust owns scheduling, SQLite persistence, and OS adapters; Tauri supplies the window, tray, autostart integration, notifications, and IPC surface.
- **Mobile — Android, iOS:** Flutter is retained because its Android foreground-service implementation already works and is tested. Android and iOS do not permit a normal third-party app to power off the device, so mobile reports that limitation and invokes no shutdown mechanism.
- **Shared contract:** the two implementations do not share binaries or source. They share executable behavior cases in `shared/testvectors/`.

The superseded Flutter desktop runners remain at `mobile/macos`, `mobile/windows`, and `mobile/linux`. They are intentionally only flagged for deletion; this change does not delete user files.

## Safety invariants

- No path reaches a real power-off executor without completing the mandatory 60-second grace period.
- The grace period can be cancelled and cannot be skipped.
- A power-off job overdue by more than 15 minutes never executes.
- Remaining time is derived from the persisted absolute target instant, never from a stored decrementing counter.
- Tests bind recording fake power-off executors. A test must never invoke a real executor.
- Android and iOS use `UnsupportedPowerOffExecutor`; no channel, root helper, or plugin attempts to bypass the OS restriction.

## Verification status

These claims describe what was actually performed on the development machine, not what the code is expected to do elsewhere.

| Target / surface | Verification performed | Status |
| --- | --- | --- |
| macOS desktop (Apple silicon) | Rust host suite run: 263 tests; web-view logic/wiring run: 101 tests; 6 shared-vector harness tests; Clippy with warnings denied; real IOKit keep-awake assertion observed appearing and disappearing through `pmset`; real `AEDeterminePermissionToAutomateTarget` preflight observed returning both `Granted` and the System-Events-not-running case; optimized `.app` release bundle built and launch-smoked | **Compiled and run** |
| macOS packaging | Built bundle inspected: no App Sandbox entitlement, non-empty `NSAppleEventsUsageDescription`; adhoc-signed consent probe issued a harmless real Apple Event to the same System Events target and macOS prompted rather than terminating | **Built and inspected** |
| macOS tray/UI interaction | Source/unit coverage exists, but this session could not click tray, hide/restore, toggle autostart, or cancel a live grace banner because its automation shell lacks macOS Accessibility and event-posting permission | **Manual exercise still required** |
| Windows desktop (`aarch64-pc-windows-msvc`) | Real Windows platform source included through `desktop/platform-check` and passed `cargo check --target ... --all-targets` | **Type-checked only — not built or run** |
| Linux desktop (`aarch64-unknown-linux-gnu`) | Real Linux platform source included through `desktop/platform-check` and passed `cargo check --target ... --all-targets` | **Type-checked only — not built or run** |
| Flutter mobile source | `flutter analyze` reported no issues; `flutter test` ran 107 tests | **Analyzed and tests run** |
| Android app package/runtime | Not rebuilt or run as part of the desktop split | **Untouched in this change** |
| iOS app/runtime | No Xcode is installed on this machine | **Not built or run** |

A cross-target `cargo check` proves type and API compatibility of the isolated adapter source. It does **not** prove linking, packaging, OS permissions, tray behavior, or runtime behavior on Windows or Linux.

## Shared-vector contract

Scheduling rules exist in two languages:

- Rust: `desktop/src-tauri/`
- Dart: `mobile/`

Both suites read the exact JSON files under `shared/testvectors/`. They cover trigger validation, target-instant resolution (including DST gaps and overlaps), and overdue reconciliation (including the inclusive 15-minute boundary).

**Any new or changed rule that must behave the same on desktop and mobile must add or update a shared vector, and both suites must execute it.** A private unit test in only one implementation does not verify cross-implementation behavior.

See [`shared/testvectors/README.md`](shared/testvectors/README.md) for the formats and current cases.

### Why not one shared FFI core?

The known cost of this layout is duplicated scheduling logic in Rust and Dart. FFI through a generated Rust bridge would remove that duplication, but it would add code generation, a cross-language debugging boundary, and per-platform native build complexity to share a comparatively small core. Shared vectors constrain behavior without adding that operational cost. Revisit FFI if the shared core grows substantially or the vectors begin catching frequent drift.

## Build and test

### Desktop

```sh
cd desktop
./verify.sh

cd src-tauri
cargo tauri build --bundles app
```

`desktop/verify.sh` runs the checks available on this macOS host and labels the Windows/Linux steps as type-checks only.

### Mobile

```sh
cd mobile
flutter analyze
flutter test
```

See [`desktop/README.md`](desktop/README.md) and the Flutter project under `mobile/` for component details.

## macOS Automation consent

The desktop app uses `osascript` to ask System Events to shut down the Mac. The release bundle carries `NSAppleEventsUsageDescription`, is not sandboxed, and receives the normal macOS Automation consent flow. If consent is denied, Weakup reports the failure and directs the user to **System Settings → Privacy & Security → Automation**; it never treats a denial as a successful shutdown.

## License

Weakup is available under the permissive [MIT License](LICENSE). You may use, copy, modify, distribute, sublicense, and sell the software subject to the license notice and disclaimer.

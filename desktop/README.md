# Weakup Desktop

Rust + Tauri v2 implementation for Windows, macOS, and Linux.

## Why this exists separately from `mobile/`

The Flutter desktop build could not be compiled on the development machine (Command
Line Tools only, no Xcode), so desktop code was written but never executed. Four
blocking defects accumulated in that never-run code. Tauri needs only Command Line
Tools to build and launch a macOS app, which makes the desktop code runnable where
it is written.

## Prerequisites

| Tool | Verified version |
| --- | --- |
| rustc / cargo | 1.95.0 |
| cargo-tauri | 2.11.4 |

macOS additionally needs Command Line Tools (`xcode-select --install`). Full Xcode is
not required.

## Build and run

```sh
# debug build
cd desktop/src-tauri
cargo build

# run the built binary directly
./target/debug/weakup-desktop

# release app bundle
cargo tauri build --bundles app
```

On a successful start the app prints a `BOOT_OK` line reporting whether the main
window is visible and at what size. A `BOOT_FAIL` line means the window was not
created — treat it as a startup regression.

## Type-check the other desktop targets

Neither Windows nor Linux can be *built* from macOS — Tauri does not cross-compile —
but their code can be type-checked:

```sh
cd desktop/platform-check
cargo check --target aarch64-pc-windows-msvc --all-targets
cargo check --target aarch64-unknown-linux-gnu --all-targets
```

The main Tauri crate cannot be cross-compiled on this host: bundled SQLite needs a
Windows C toolchain and Tauri needs Linux GTK headers. `platform-check` includes the
real per-OS adapter sources without those native dependencies. This catches type and
API errors, not linker, packaging, or runtime problems. It is not a build and must not
be reported as one.

## Layout

```
src/                     front end served into the web view
src-tauri/
├── Info.plist           merged into the macOS bundle (Apple Events usage description)
├── tauri.conf.json      window, bundle, and security configuration
└── src/
    ├── core/            Result and AppError — no dependencies
    ├── domain/          Job, TriggerSpec, TriggerResolver — pure logic
    ├── data/            SQLite persistence
    ├── platform/        power-off, keep-awake, tray, autostart — #[cfg] per OS
    ├── application/     JobScheduler — owns all timers
    └── commands/        Tauri IPC surface
```

Dependencies point inward only. `core/` and `domain/` are the modules exercised by the
shared test vectors in `../shared/testvectors/`.

## Testing rule that is not negotiable

Tests must never bind a real power-off executor. `PowerOffExecutor` is a trait; the
test suite binds a recording fake. A real executor in a test would shut down the
developer's machine.

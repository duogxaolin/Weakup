# CLAUDE.md

Guidance for working on this repository — written so a fresh machine (including Windows)
can build, test, and continue this work without rediscovering the context.

## What this project is

**Weakup** — a power scheduler. It keeps a display awake and schedules power-off jobs
(indefinitely, for a duration, until a wall-clock time, or on a calendar date). Two
clients share one cross-language contract:

| Client | Stack | Location |
| --- | --- | --- |
| Desktop | Tauri 2 + Rust backend, plain HTML/JS frontend (no bundler) | `desktop/` |
| Mobile | Flutter (Android today; iOS code present but untested) | `mobile/` |

Remote control (pair two devices, send signed power-off commands through a Firestore
relay) is implemented and tested end to end against fakes, but **does not work
out of the box** — see "Honest unverified list" below.

## Repo layout

```
desktop/
  src/            # frontend: index.html, main.js, logic.js, i18n.js + node:test suites
  src-tauri/      # Rust: domain/ application/ commands/ data/ platform/
  platform-check/ # type-checks the real Windows/Linux platform sources on a macOS host
mobile/
  lib/            # Flutter app: domain/ application/ data/ ui/
  android/        # Gradle project, applicationId vn.delify.weakup
shared/
  testvectors/    # 11 JSON contract files executed by BOTH language suites
openspec/         # spec-driven: specs/ (14), changes/archive/ (completed changes)
docs/RELEASE.md   # release pipeline status + how to finish the first run
firebase.json, firestore.rules, firestore.indexes.json
```

## The one rule that shapes everything

Scheduling logic exists **twice** — Rust (`desktop/src-tauri/src/domain/`) and Dart
(`mobile/lib/domain/`). Nothing prevents silent drift except `shared/testvectors/*.json`,
which both suites execute. Any wire-shape or behavior change must land in:

1. `shared/testvectors/` (and `shared/testvectors/README.md`, the wire-format doc),
2. the Rust implementation + `desktop/src-tauri/tests/shared_vectors.rs`,
3. the Dart implementation + `mobile/test/shared/shared_vectors_test.dart`,

in the same change, or the two suites diverge while both stay green. Vectors are written
**first**, and existing vector cases are never edited — extension only.

## Toolchain (versions that are known to work)

- Rust **1.95.0**, cargo-tauri **2.11.4** (`cargo install tauri-cli --version "^2" --locked`)
- Node **22** (frontend tests use built-in `node:test`; `desktop/package.json` has zero deps)
- Flutter **3.44.8** stable (pinned in `.github/workflows/release.yml`), Dart SDK ^3.12.2
- Android: Gradle **9.1.0**, AGP **9.0.1**, Kotlin **2.3.20**, Java **21** (temurin),
  compileSdk 36 (build-tools 36.0.0/34.0.0 cover the Firebase plugins)
- macOS dev host additionally needs only Command Line Tools (no full Xcode for desktop)

## Building and testing on Windows

Windows is a **first-class runtime target** (`.msi`/NSIS bundles are built in release CI),
but the repo's main dev host is macOS, so note what transfers:

Prerequisites on Windows:

1. **Rust** with the `x86_64-pc-windows-msvc` (or aarch64) MSVC toolchain + Visual Studio
   Build Tools (link.exe).
2. **WebView2 Runtime** — preinstalled on Windows 11; Tauri needs it.
3. **Node 22**, **Flutter 3.44.8**, **Java 21**, and an **Android SDK** (platform 36,
   build-tools 34/36) for the mobile side.
4. `cargo install tauri-cli --version "^2" --locked`.

What runs natively on Windows vs. what does not:

| Command | On Windows |
| --- | --- |
| `cd desktop/src-tauri && cargo test --lib` | ✅ runs natively (534 tests) |
| `cargo clippy --all-targets -- -D warnings` | ✅ runs natively |
| `cd desktop/src && node --test` | ✅ runs natively (115 tests) |
| `cargo test --test shared_vectors` | ✅ runs natively (24 tests) |
| `cargo test --test end_to_end_authenticity` / `_pairing_to_job` | ✅ runs natively (12 / 9) |
| `cd mobile && flutter test` | ✅ runs natively (402 tests) |
| `cd mobile && flutter build apk --release` | ✅ runs natively |
| `desktop/verify.sh` | ❌ bash script with macOS paths (`/opt/homebrew`, IOKit probe) — run its steps individually instead |
| IOKit assertion probe (`assertion_probe` example) | macOS-only; skip on Windows |
| `platform-check` crate | exists to type-check Windows/Linux sources *from macOS*; on Windows the real code compiles anyway, so it is redundant |

Windows-specific behavior worth knowing when testing:

- Power-off executes `shutdown /s /t 0` (`platform/power_off/windows.rs`); the mandatory
  60 s (local) / 300 s (remote) cancellable grace period always precedes it.
- Secrets (device identity key, OAuth session) go to **Windows Credential Manager** via
  the `keyring` crate.
- Tray, autostart, and notifications degrade gracefully — a missing capability logs a
  warning and the app still schedules (`src/setup.rs`, task 9.8 invariant).

Run `cd desktop && cargo tauri dev` for the desktop app; `cd mobile && flutter run` for
Android. Debug desktop startup prints a `BOOT_OK` / `BOOT_FAIL` line.

## Verification gates (current counts, all verified locally 2026-08)

```
cd desktop/src-tauri && cargo test --lib                          # 534
cd desktop/src-tauri && cargo clippy --all-targets -- -D warnings # clean
cd desktop/src-tauri && cargo test --test shared_vectors          # 24 (11 vector files)
cd desktop/src-tauri && cargo test --test end_to_end_authenticity # 12
cd desktop/src-tauri && cargo test --test end_to_end_pairing_to_job # 9
cd desktop/src        && node --test                              # 115
cd mobile             && flutter analyze --fatal-infos            # clean
cd mobile             && flutter test                             # 402
```

`mobile` drift-generated code: after editing `mobile/lib/data/tables.dart`, run
`dart run build_runner build --delete-conflicting-outputs` **before** `flutter test`
or it will not compile.

Known flaky (do not "fix" by weakening): `mobile/test/application/job_scheduler_test.dart`
"overdue just under 15 minutes" occasionally needs a re-run.

## Gotchas

- `cargo tauri build` **silently rewrites** `desktop/src-tauri/Cargo.toml` (adds
  `features = []`). `git checkout -- desktop/src-tauri/Cargo.toml` after any packaging.
- `capabilities/default.json` and the CSP in `tauri.conf.json` must stay **byte-identical**
  (D2 invariant: all network access is Rust-side; the webview never talks to Firebase).
- The pairing code never travels through the relay (D6) — it is read off one screen and
  typed into the other; only Ed25519-signed commands cross Firestore.
- All three version declarations agree at **0.2.0** (`tauri.conf.json`, `Cargo.toml`,
  `mobile/pubspec.yaml` as `0.2.0+1`). Release builds still override from the git tag
  (`--config '{"version":…}'`, `--build-name`); dev builds report the file values. When
  cutting a release whose version differs from 0.2.0, bump all three files in the same
  change so dev builds never lie about what they are.
- Android signing: with the four `ANDROID_KEYSTORE_*` secrets set, APKs are upload-key
  signed; without them, Gradle falls back to the **debug key** and the filename says
  `-debug-key`. Never invent or commit a keystore (`.gitignore` covers `*.jks`,
  `*.keystore`, `key.properties`, `oauth_client.json`).
- Firebase config (`google-services.json`, iOS plist values) is committed deliberately;
  the OAuth client secret file is not (no secret lives in this repo).
- reqwest 0.13 renamed feature `rustls-tls` → `rustls`.

## Honest unverified list

Do not claim these work without testing them for real:

- **Remote control end to end.** `setup.rs` installs **no transport** (`None`) —
  `list_devices` returns empty and peers show `Offline` by design. Sign-in fails with a
  clear error until a Desktop OAuth client exists. Two manual Firebase console steps are
  required: `openspec/changes/archive/2026-08-07-add-firebase-transport/MANUAL-SETUP.md`.
- **Windows/Linux runtime behavior.** `platform-check` type-checks those sources from
  macOS; nothing has executed them. A Windows machine running the suite above closes
  most of that gap.
- **iOS.** Code present, never built on a real device; plist needs manual Xcode wiring.
- **First release run.** Tag `v0.2.0-rc.1` fired nothing because of the 2026-08-06
  GitHub Actions outage. Recovery steps and first-run failure suspects:
  `docs/RELEASE.md`.

## Conventions

- Respond to the user in **Vietnamese** (owner's preference); code, comments, and repo
  docs in English.
- Error messages in the Rust/Dart domain layer must be **byte-identical across the two
  languages** — the vectors pin them.
- Source-text tests live in separate files (an inline `mod tests` matches its own file's
  source and passes for the wrong reason).
- Spec-driven workflow: changes go through `openspec/changes/…` (proposal → tasks →
  delta specs) and are archived to `openspec/changes/archive/` when verified.
- Safety invariants that must never regress: grace period is always cancellable and
  cannot be skipped; a power-off >15 min overdue does not run; mobile never powers off
  the device (`UnsupportedPowerOffExecutor`) — it reports the limitation instead.

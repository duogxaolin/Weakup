# Tasks

## 1. The preflight itself

- [x] 1.1 `platform/power_off/preflight.rs`: a `PowerOffPermission` enum with the three
      outcomes, `is_granted` / `blocks_scheduling` / `reason`, and
      `check_power_off_permission(ask_user)`.
- [x] 1.2 `classify_automation_status` split out as a **pure** `i32 -> verdict` function,
      compiled on every host. The `unsafe` call above it is left with no branching, and the
      two targets that cannot run macOS still get the mapping tested.
- [x] 1.3 The macOS FFI: `AECreateDesc` addressing System Events by bundle id,
      `AEDeterminePermissionToAutomateTarget`, `AEDisposeDesc` through a drop guard.
      `typeApplicationBundleID` is `'bund'` — an invalid code does not fail `AECreateDesc`,
      it builds a descriptor the permission call reads as garbage.
- [x] 1.4 `AEDesc` declared `#[repr(C, packed(2))]`. It is `#pragma pack(2)` in the header:
      12 bytes, `dataHandle` at offset 4. Plain `#[repr(C)]` corrupts the handle on the
      first move and segfaults inside the framework. Layout test plus a move test.
- [x] 1.5 The non-macOS arm returns `Unknown`, and is registered in `platform-check` so it
      is type-checked for Windows and Linux — the host build never compiles it.

## 2. The command surface

- [x] 2.1 `commands/dto.rs`: `PermissionView` with a `state` string, `blocksScheduling`, and
      an optional reason. camelCase, because `logic.js` reads `blocksScheduling` and a
      snake_case key would be `undefined`, which is falsy — the form would stop blocking.
- [x] 2.2 `commands/mod.rs`: `check_shutdown_permission(ask_user)`. Named to survive the
      existing "nothing the web view can call powers the machine off" filter on its meaning:
      it reports on consent and holds no executor.
- [x] 2.3 Registered in `lib.rs`. A `#[tauri::command]` that is not registered fails only at
      runtime with "command not found".
- [x] 2.4 Verdict mirrored into `CapabilityRegistry`: a denial degrades, a grant clears an
      earlier denial, an undetermined verdict writes nothing.
- [x] 2.5 Tests: the view's three states, the camelCase serialisation, and a source-level
      check that the command body reaches for no executor, `osascript`, or `Command::new`.

## 3. The startup probe

- [x] 3.1 `setup.rs`: probe with `ask_user: false` after the `is_supported` check, degrading
      only on an outright denial. Returns early when the platform cannot shut down at all.
- [x] 3.2 Tests: a source-level assertion that startup passes `false` and never `true`, that
      a denial degrades an essential capability, and that a non-blocking verdict leaves
      power-off available.

## 4. The frontend

- [x] 4.1 `logic.js`: `permissionOutcome`, pure. A denial blocks; undetermined proceeds with
      a warning; a missing or malformed verdict proceeds silently, so a broken check leaves
      the user with the behaviour they had before it existed.
- [x] 4.2 `i18n.js`: `permission.deniedFallback` and `powerOff.consentNote` in both tables.
- [x] 4.3 `index.html`: the consent note in the power-off card, hidden by default, revealed
      from the verdict rather than from the user agent.
- [x] 4.4 `main.js`: ticking Power Off asks with `false` and reveals the note; `submitForm`
      asks with `true` and blocks on a denial. The order is what makes the explanation
      precede the dialog, and it is structural — the checkbox handler cannot prompt.
- [x] 4.5 Tests: six `permissionOutcome` cases in `logic.test.js`; in `wiring.test.js` that
      the check precedes `createOne`, that the explanation cannot be raced by the prompt,
      and that `logic.js` does not reason about OS permissions itself.

## 5. Verification

- [x] 5.1 `node --test`, `cargo test --lib`, `cargo clippy --all-targets -- -D warnings`,
      both cross-target type checks, `./desktop/verify.sh`.
- [x] 5.2 The real FFI exercised on the host: `Granted` with System Events running, and the
      `procNotFound` path without it. `-1743` needs a GUI refusal that cannot be driven from
      a shell; it is covered by the classifier test pinned to the same code `osascript`
      returns at shutdown time.
- [x] 5.3 Mutation-test each new guard: a denial that no longer blocks, an undetermined
      verdict that wrongly blocks, the malformed-verdict guard removed, and startup switched
      to `ask_user: true`. Each must fail the test written for it.
- [x] 5.4 Built `.app` launched: `BOOT_OK`, and no consent dialog raised during startup.
      `git checkout` `Cargo.toml` afterwards — `cargo tauri build` rewrites it.

## 1. Shared vectors first

Both suites must fail for the same reasons before any implementation exists. If either goes green
here, the vectors are not exercising the rules.

- [x] 1.1 Create `shared/testvectors/presence.json` with cases: `online-just-reported`, `online-well-within-window`, `online-exactly-at-threshold` (90s → online, per design D2), `stale-just-past-online-threshold`, `stale-midway`, `stale-exactly-at-offline-threshold` (15min → stale, per D2), `offline-just-past-offline-threshold`, `offline-long-gone`, `offline-never-reported` (null `lastSeen`), `online-last-seen-in-the-future-clamps-to-zero`, `online-last-seen-far-in-the-future-clamps-to-zero`. Every case supplies `now` explicitly and asserts both `expectedState` and `expectedElapsedSeconds`.
- [x] 1.2 Create `shared/testvectors/remote_authorization.json` with cases covering each `DenialReason` in isolation, one `allowed` case per remotely-permitted command, and at least three cases where two reasons hold simultaneously to pin the D5 precedence order (`CommandNotRemotelyAllowed` → `RemoteControlDisabled` → `NotPaired` → `PlatformCannotPerform`). Each case names all five `RemoteCommandContext` fields explicitly.
- [x] 1.3 Add a `presence.json` section to `shared/testvectors/README.md`: the wire format, both threshold values, the inclusive-online boundary rule, and the future-`lastSeen` clamp with its reason.
- [x] 1.4 Add a `remote_authorization.json` section to the README: the wire format, the closed reason set, and the precedence order with the least-disclosure reason from design D5.
- [x] 1.5 Add a presence runner to `desktop/src-tauri/tests/shared_vectors.rs` asserting state and elapsed per case, plus a required-id guard listing the two exact-boundary ids and the two future-clock ids so a parse failure cannot silently skip them.
- [x] 1.6 Add a remote-authorization runner to the same file, asserting the exact `DenialReason` (not merely that it was denied), plus a guard failing any case that omits a required context field.
- [x] 1.7 Mirror both runners in `mobile/test/shared/shared_vectors_test.dart`, with the same required-id guards ← (verify: both suites now fail for missing implementation, not for parse errors; every new case id appears in both required-id lists)

## 2. Rust domain rules

- [x] 2.1 Create `desktop/src-tauri/src/domain/presence.rs`: `PresenceState` enum (`Online`/`Stale`/`Offline`, `#[serde(rename_all = "camelCase")]` per the `JobStatus` idiom), `PresenceEvaluation { state, elapsed }`, `ONLINE_THRESHOLD_SECONDS = 90`, `OFFLINE_THRESHOLD_SECONDS = 900`, and a pure `evaluate(last_seen: Option<DateTime<Utc>>, now: DateTime<Utc>) -> PresenceEvaluation` that clamps a future `last_seen` to zero elapsed.
- [x] 2.2 Add unit tests in the file's own `mod tests` for the boundary and clamp cases, in the existing style — the vectors prove cross-language agreement, these prove the local rule.
- [x] 2.3 Create `desktop/src-tauri/src/domain/remote_command.rs`: `RemoteCommand` enum, `DenialReason` (four closed variants), `RemoteCommandDecision { Allowed, Denied(DenialReason) }`, `RemoteCommandContext` with the five named fields from design D4, and `authorize(context) -> RemoteCommandDecision` implementing the D5 precedence order.
- [x] 2.4 Give `DenialReason` a `user_message()` returning specific prose per variant, matching the wording style of `AppError::user_message`. The remote-control-disabled message must say the setting can only be enabled at the target device.
- [x] 2.5 Add `JobOrigin { Local, Remote }` to `desktop/src-tauri/src/domain/job_enums.rs` and a field on `Job` defaulting to `Local`, changing no existing construction site's behavior.
- [x] 2.6 Register both new modules in `desktop/src-tauri/src/domain/mod.rs`.
- [x] 2.7 Add a test asserting `JobOrigin` defaults to `Local`, with a comment naming the D7 persistence gap it documents ← (verify: no existing Rust test needed editing; `cargo test --lib` and `--test shared_vectors` both green; the four required-id guards actually execute)

## 3. The remote countdown and the structural invariant

- [x] 3.1 Add `REMOTE_GRACE_PERIOD_SECONDS = 300` to `desktop/src-tauri/src/application/grace_period.rs`, leaving `GRACE_PERIOD_SECONDS = 60` untouched in both name and value.
- [x] 3.2 Have `PowerOffGate::run` select the duration from the job's `JobOrigin` rather than from a new parameter. Its signature must gain no duration argument — see design D6 and the module's own doc comment.
- [x] 3.3 Extend the file's doc comment to state that the duration is chosen from the job's origin and that no argument selects it.
- [x] 3.4 Add grace-period tests: a `Local` job counts exactly 60 ticks; a `Remote` job counts exactly 300; a cancel at tick 1 of a remote countdown still reaches no executor; and `REMOTE_GRACE_PERIOD_SECONDS > GRACE_PERIOD_SECONDS`.
- [x] 3.5 Add a source-text test in the style of `no_command_can_reach_a_power_off_executor` that `include_str!`s `remote_command.rs` and fails on `power_off(`, `PowerOffGate`, `PowerOffExecutor`, or `GraceOutcome` ← (verify: every pre-existing grace-period test passes unmodified; `run` has no duration parameter; the new source-text test genuinely fails when a forbidden reference is added)

## 4. Dart domain rules

- [x] 4.1 Create `mobile/lib/domain/presence.dart` mirroring `presence.rs`: same state names, same two threshold constants, same clamp, hand-written `==`/`hashCode` per the `CalendarDate` idiom.
- [x] 4.2 Create `mobile/lib/domain/remote_command.dart` mirroring `remote_command.rs`: sealed `RemoteCommandDecision`, the same four denial reasons, the same context fields, the same precedence order, and denial messages byte-identical to the Rust ones.
- [x] 4.3 Add `JobOrigin` to `mobile/lib/domain/job_enums.dart` and the defaulted field on `Job`, matching Rust.
- [x] 4.4 Export both new files from `mobile/lib/domain/domain.dart`.
- [x] 4.5 Add Dart unit tests mirroring the Rust ones from 2.2 and 2.7 ← (verify: `flutter analyze` clean; `flutter test` green including both new vector runners; denial message strings are identical across languages — diff them, do not eyeball)

## 5. Capability model, both languages

- [x] 5.1 Add the remote-target capability field to `desktop/src-tauri/src/platform/capabilities.rs`: true for Windows/macOS/Linux, false for Android/iOS.
- [x] 5.2 Add the same field to `mobile/lib/platform/platform_capabilities.dart`, including every explicit test-construction helper in that file so none is left with a missing required argument.
- [x] 5.3 Add tests in both languages asserting the desktop-true / mobile-false matrix ← (verify: no `Platform.isX` check was added outside the capability model; every existing capability test still compiles and passes)

## 6. Full verification

- [x] 6.1 Run `cd desktop/src-tauri && cargo test --lib`, `cargo test --test shared_vectors`, and `cargo clippy --all-targets -- -D warnings`.
- [x] 6.2 Run `cd mobile && flutter analyze` and `flutter test`.
- [x] 6.3 Run `cd desktop/src && node --test` to confirm the frontend suite is unaffected by a change that touches no frontend file.
- [x] 6.4 `git diff` both vector files and confirm no pre-existing case was edited — an edit means a format was broken rather than extended.
- [x] 6.5 Confirm both vector suites report the new cases as executed by id, not merely that the files parsed ← (verify: all six commands green; zero edits to pre-existing vector cases; no database schema version changed in either implementation; `capabilities/default.json` untouched)

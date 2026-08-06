## 1. Shared vectors first

Both suites must fail for the same reasons before any implementation exists. If either goes green
here, the vectors are not exercising the rules.

- [x] 1.1 Create `shared/testvectors/command_acceptance.json`. Cases must cover: each `RejectionReason` in isolation; one accepted case per remotely-permitted command; `fresh-exactly-at-window` (120s → accepted, per design D4); `stale-one-second-past-window`; `future-exactly-at-tolerance` (30s → accepted); `future-one-second-past-tolerance` (→ `FutureDated`, not `Stale`); `replay-of-seen-nonce`; `distinct-nonce-same-device-accepted`; and at least four cases where several reasons hold at once to pin the D5 precedence (`AuthenticityUnverified` → `ReplayedNonce` → `FutureDated` → `Stale` → `NotPermitted`), including one where all five hold. Every case supplies `now` explicitly and names every envelope and target-state field.
- [x] 1.2 Create `shared/testvectors/pairing_grant.json`. Cases must cover: valid at-machine grant; valid out-of-band grant; `at-machine-exactly-at-lifetime` (300s → valid, inclusive per D7); `at-machine-one-second-past`; `out-of-band-exactly-at-lifetime` (600s → valid); `out-of-band-one-second-past`; `age-between-the-two-lifetimes-splits-by-delivery` (the case that proves the two lifetimes are actually distinct); already-redeemed before expiry (→ `AlreadyUsed`, not `Expired`); already-redeemed after expiry (pins precedence); and unrecognised grant (→ `NoSuchGrant`, not `Expired`).
- [x] 1.3 Add a `command_acceptance.json` section to `shared/testvectors/README.md`: the wire format, the two time constants with their values, the inclusive-accept boundary rule, the closed reason set, and the D5 precedence order with its no-probing rationale.
- [x] 1.4 Add a `pairing_grant.json` section to the README: the wire format, both lifetimes, the closed reason set, and the D7 note that out-of-band is longer in wall-clock terms and why.
- [x] 1.5 Add a command-acceptance runner to `desktop/src-tauri/tests/shared_vectors.rs`, asserting the exact `RejectionReason` and not merely that the command was rejected, plus a required-id guard listing every boundary id and every multi-reason precedence id so a parse failure cannot silently skip them.
- [x] 1.6 Add a pairing-grant runner to the same file, with a required-id guard listing both exact-lifetime ids and the splits-by-delivery id.
- [x] 1.7 Mirror both runners in `mobile/test/shared/shared_vectors_test.dart` with the same required-id guards ← (verify: both suites now fail for missing implementation, not for parse errors; every new case id appears in both required-id lists)

## 2. Rust domain — identity, envelope, acceptance

- [x] 2.1 Create `desktop/src-tauri/src/domain/device_id.rs`: `DeviceId` newtype over `String`, validating non-empty per design D10, with a constructor returning `AppResult<Self>` and `AppError::validation` on empty. No structure imposed — no account, no platform, no key material.
- [x] 2.2 Create `desktop/src-tauri/src/domain/command_envelope.rs`: `CommandEnvelope { sender: DeviceId, command: RemoteCommand, created_at: DateTime<Utc>, nonce: String, signature_verified: bool }`. Per design D3 it must carry **no** field an intermediary could set — no `relay_attested`, no `server_verified`, no `trusted_source`. Document that omission in the struct's doc comment with the reason.
- [x] 2.3 Create `desktop/src-tauri/src/domain/command_acceptance.rs`: `FRESHNESS_WINDOW_SECONDS = 120`, `FUTURE_TOLERANCE_SECONDS = 30`, `NONCE_RETENTION_SECONDS = 600`; `RejectionReason` (five closed variants); `CommandAcceptance { Accepted, Rejected(RejectionReason) }`; `CommandTargetState` carrying the paired-device set, the seen-nonce set, and the authorization context fields.
- [x] 2.4 Implement `evaluate_command(envelope, target_state, now) -> CommandAcceptance` with the D5 precedence order. It must delegate the permission question to the existing `authorize()` rather than reimplementing it — per design D1 the seam is deliberate.
- [x] 2.5 Add the compile-time assertion from design D6 that `NONCE_RETENTION_SECONDS > FRESHNESS_WINDOW_SECONDS + FUTURE_TOLERANCE_SECONDS`, in the `const _: () = assert!(...)` style already used for the remote countdown. A comment must state that shortening retention reopens the replay window.
- [x] 2.6 Give `RejectionReason` a `user_message()` returning specific prose per variant, matching the wording style of `AppError::user_message` and `DenialReason::user_message`.
- [x] 2.7 Add unit tests in each file's own `mod tests` for the boundaries and the precedence — the vectors prove cross-language agreement, these prove the local rule.
- [x] 2.8 Register the three new modules in `desktop/src-tauri/src/domain/mod.rs` ← (verify: `cargo test --lib` and `--test shared_vectors` green; the compile-time assertion actually fails the build when retention is lowered below the sum — try it and revert)

## 3. Rust domain — pairing grants

- [x] 3.1 Create `desktop/src-tauri/src/domain/pairing_grant.rs`: `GrantDelivery { AtMachine, OutOfBand }`, `AT_MACHINE_GRANT_LIFETIME_SECONDS = 300`, `OUT_OF_BAND_GRANT_LIFETIME_SECONDS = 600`, `GrantRejection` (three closed variants: `Expired`, `AlreadyUsed`, `NoSuchGrant`), `GrantValidity { Valid, Invalid(GrantRejection) }`.
- [x] 3.2 Implement `evaluate_grant(grant, now) -> GrantValidity`, with `AlreadyUsed` taking precedence over `Expired` per the vectors in 1.2 — an already-used grant reports that fact whether or not it has also expired.
- [x] 3.3 Document in the module doc comment why the out-of-band lifetime is longer in wall-clock terms despite resting on weaker evidence, per design D7. This is the value most likely to be "corrected" by a later contributor who has not read the reasoning.
- [x] 3.4 Add unit tests for both boundaries, the splits-by-delivery case, and the precedence pair ← (verify: both lifetime constants appear exactly once each; no test hardcodes 300 or 600 as a literal where the constant should be referenced)

## 4. Persist `JobOrigin` — Rust

This closes the gap named as a blocker by D7 of the previous change. Today
`sqlite_repository.rs:179-181` hardcodes `JobOrigin::default()` on every read, so a remote job that
survives a restart is handed the 60-second countdown instead of 300.

- [x] 4.1 Bump `SCHEMA_VERSION` 3 → 4 in `desktop/src-tauri/src/data/sqlite_repository.rs` and add `ALTER TABLE jobs ADD COLUMN origin TEXT` to the migration path.
- [x] 4.2 **Append** `origin` to the end of `JOB_COLUMNS`, not beside the other job fields — `row_to_job` reads by positional index, so appending makes it index 13 and leaves 0–12 untouched. Carry a comment in the same shape as the existing `trigger_date` comment at `:200` explaining the reason.
- [x] 4.3 Replace the hardcoded `JobOrigin::default()` at `:179-181` with a read of the new column. `NULL` reads as `Local` — every pre-migration row was locally scheduled, so this is a faithful reading rather than a default. An unrecognised non-null value is `AppError::Storage`, matching the file's other "row holds an unknown …" errors.
- [x] 4.4 Widen `insert_job` and any update path so origin is written on create and preserved on update.
- [x] 4.5 Add tests in `sqlite_repository_tests.rs`: a remote job round-trips as remote; a local job round-trips as local; a migration test that opens a v3 file and confirms existing rows read back as `Local`; and a test that a remote job recovered from storage is given `REMOTE_GRACE_PERIOD_SECONDS` by `grace_period_seconds`, which is the actual bug this closes ← (verify: the last test fails if 4.3 is reverted — confirm by reverting temporarily)

## 5. Record every command decision — Rust

- [x] 5.1 Add a `command_decisions` table to the schema created in 4.1: sender device, command, decision, rejection reason (nullable — null means accepted), and decision instant. Per design D9 it is a separate table, not a column on `jobs`, because refused commands create no job and refusals are what must be recorded.
- [x] 5.2 Add repository methods to append a decision and to read recent decisions, in the style of the existing repository surface. Reading must not require a network — it is a local table.
- [x] 5.3 Add tests: an accepted decision is recorded with its sender and instant; a refusal is recorded with its single reason; several refusals from one device are all retained ← (verify: a refusal path writes a record — assert on the stored row, not on a return value)

## 6. Dart domain — mirror everything

- [x] 6.1 Create `mobile/lib/domain/device_id.dart` mirroring `device_id.rs`, hand-written `==`/`hashCode` per the `CalendarDate` idiom.
- [x] 6.2 Create `mobile/lib/domain/command_envelope.dart` mirroring the Rust struct, carrying the same field set and the same documented omission.
- [x] 6.3 Create `mobile/lib/domain/command_acceptance.dart`: the same three constants, sealed `CommandAcceptance`, the same five closed reasons, the same precedence order, and rejection messages **byte-identical** to the Rust ones.
- [x] 6.4 Create `mobile/lib/domain/pairing_grant.dart` mirroring `pairing_grant.rs`, same two lifetimes, same three reasons, same precedence.
- [x] 6.5 Add the Dart equivalent of the D6 retention assertion. Dart has no compile-time assert, so use an `assert` in a top-level initializer plus a test that the relationship holds — and note in a comment that this is weaker than Rust's compile-time version.
- [x] 6.6 Export all four new files from `mobile/lib/domain/domain.dart`.
- [x] 6.7 Add Dart unit tests mirroring 2.7 and 3.4 ← (verify: `flutter analyze` clean; every rejection and grant message string is identical across languages — **diff them mechanically, do not eyeball**)

## 7. Persist `JobOrigin` and decisions — Dart

- [x] 7.1 Add the origin column to `mobile/lib/data/tables.dart` and a `command_decisions` table.
- [x] 7.2 Bump `schemaVersion` 2 → 3 in `mobile/lib/data/app_database.dart` and add the migration step to the **existing** `MigrationStrategy.onUpgrade` at `:20-21` — do not replace the strategy, extend it, or the `trigger_date` migration from the previous change is lost.
- [x] 7.3 Run `dart run build_runner build --delete-conflicting-outputs`. This is drift-generated code; `flutter test` will not compile until it is regenerated.
- [x] 7.4 Update `mobile/lib/data/job_mapper.dart` so origin is written and read.
- [x] 7.5 Add tests mirroring 4.5 and 5.3, including the v2 → v3 migration reading old rows as local ← (verify: `flutter test` green; the migration test actually opens a pre-migration database rather than asserting on a fresh one)

## 8. Wire authenticity ahead of authorization

- [x] 8.1 Add a source-text test in the style of `no_remote_command_type_can_reach_a_power_off_executor` that `include_str!`s `command_acceptance.rs` and fails on `power_off(`, `PowerOffGate`, `PowerOffExecutor`, or `GraceOutcome`. Put it in a separate `command_acceptance_tests.rs` file, not an inline `mod tests` — an inline module's own string literals would self-match the grep, which is the trap the previous change hit.
- [x] 8.2 Add a test asserting that an envelope with `signature_verified: false` is rejected with `AuthenticityUnverified` **and** that no permission reason is reported, even when the command would also fail permission. This is the no-probing requirement from the spec.
- [x] 8.3 Mirror both tests in Dart where applicable (the source-text test is Rust-only; the no-probing test applies to both) ← (verify: the source-text test genuinely fails when a forbidden reference is added to `command_acceptance.rs` — add one, confirm red, revert)

## 9. Full verification

- [x] 9.1 Run `cd desktop/src-tauri && cargo test --lib`, `cargo test --test shared_vectors`, and `cargo clippy --all-targets -- -D warnings`.
- [x] 9.2 Run `cd mobile && flutter analyze` and `flutter test`.
- [x] 9.3 Run `cd desktop/src && node --test` to confirm the frontend suite is unaffected by a change that touches no frontend file.
- [x] 9.4 `git diff` all pre-existing vector files and confirm no existing case was edited — an edit means a format was broken rather than extended.
- [x] 9.5 Confirm both new vector suites report their cases as executed **by id**, not merely that the files parsed.
- [x] 9.6 Diff the rejection-reason and grant-rejection message strings between Rust and Dart mechanically and confirm the diff is empty ← (verify: all six commands green; zero edits to pre-existing vector cases; both schema versions bumped exactly once each; `capabilities/default.json` untouched; no network dependency added to either `Cargo.toml` or `pubspec.yaml`)

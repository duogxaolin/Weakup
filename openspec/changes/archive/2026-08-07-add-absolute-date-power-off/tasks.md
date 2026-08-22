# Tasks

## 1. Shared contract (first, so both suites fail for the same reasons)

- [x] 1.1 Add the dated semantic cases to `shared/testvectors/resolution.json`, including the
      direct counterpart of `absolute-past-rolls-to-tomorrow` — same inputs plus a date,
      opposite outcome — and both DST anomalies with a date.
- [x] 1.2 Add `absolute-dated-accepted` and `keep-awake-dated-absolute-accepted` to
      `shared/testvectors/validation.json`. Malformed dates are **not** vector cases: a
      value like `2026-02-30` cannot be represented in either language's parsed form, so
      well-formedness is enforced at the parse boundary and tested per language.
- [x] 1.3 Add the optional `expectedErrorContains` to `ResolutionCase` in both harnesses,
      since `expectedTargetInstantUtc: null` already means "indefinite" and cannot double as
      "error". Guard that a case sets exactly one of the two expectations.
- [x] 1.4 Add the new dated DST ids to the required-id assertion in both harnesses.
- [x] 1.5 Document `date` in `shared/testvectors/README.md`: the wire format, the two
      semantics, why the past-date rule lives in `resolve` and not `validate`, and why
      malformed dates are absent.

## 2. Rust domain

- [x] 2.1 `domain/trigger_spec.rs`: add `date: Option<NaiveDate>` to `AbsoluteTime` with
      `#[serde(default, skip_serializing_if = "Option::is_none")]`. Add `at_time` and
      `on_date` constructors. Test that the undated form still serialises with no `date` key.
- [x] 2.2 `domain/trigger_resolver.rs`: split the `AbsoluteTime` arm of `resolve` into a
      fixed-date branch (reject a passed target) and the existing roll-forward branch.
      `validate`, `resolve_wall_time`, and `reconcile` keep their signatures and behaviour.
- [x] 2.3 Follow the compiler through the `AbsoluteTime` literals, routing test sites through
      the new constructors.
- [x] 2.4 `domain/job.rs`: add a dated case to the `is_timezone_sensitive` test to pin that
      it needs no change.
- [x] 2.5 `commands/dto.rs`: the same field on `TriggerInput::AbsoluteTime`, forwarded by the
      `From` impl; `trigger_label` gains a dated form. Keep the existing pinned JSON literal
      and add a dated one.
- [x] 2.6 `application/scheduler.rs`: a dated job whose date has passed fails to resume and
      stays `Paused`; a timezone change that pushes a dated target into the past leaves the
      stored target untouched, logs, and continues. Tests for both.
- [x] 2.7 `data/sqlite_repository.rs`: `SCHEMA_VERSION` 2 → 3, `ALTER TABLE jobs ADD COLUMN
      trigger_date TEXT`, the column appended to the **end** of `JOB_COLUMNS` because reads
      are positional. Round-trip test plus a v2-file migration test.

## 3. Dart domain

- [x] 3.1 `mobile/lib/domain/calendar_date.dart`: a `CalendarDate` value type with a
      validating factory. Not a `DateTime` — a wall-clock date must carry no zone.
- [x] 3.2 `mobile/lib/domain/trigger_spec.dart`: an optional `date`, extending `==`,
      `hashCode`, and `toString`.
- [x] 3.3 `mobile/lib/domain/trigger_resolver.dart`: the fixed-date branch, with the message
      byte-identical to Rust's. `_wallTimeOn` untouched.
- [x] 3.4 `tables.dart`, `app_database.dart` (schema 1 → 2 plus a new `MigrationStrategy`),
      and `job_mapper.dart`; then `dart run build_runner build --delete-conflicting-outputs`.
- [x] 3.5 Mirror the new cases in the Dart domain and scheduler tests.

## 4. Desktop frontend

- [x] 4.1 `i18n.js`: `field.date` and `field.dateHint` in both tables, and reword
      `powerOff.modeTime` — "At a time today" is no longer accurate.
- [x] 4.2 `index.html`: a hidden date field inside the power-off card only, with a label and
      a hint. No `value` attribute, so empty stays the default.
- [x] 4.3 `logic.js`: `readTriggerFrom` accepts `dateValue` and includes `date` only when it
      is non-empty. No date validation in JS — that contract is deliberate.
- [x] 4.4 `main.js`: the date ids on the power-off `FORMS` entry only, guarded reads so the
      keep-awake form does not look for an element that does not exist.
- [x] 4.5 Tests: `logic.test.js` for the wire shape, `wiring.test.js` asserting the field is
      in the power-off card and absent from keep-awake.

## 5. Verification

- [x] 5.1 `node --test`, `cargo test --lib`, `cargo test --test shared_vectors`,
      `cargo clippy --all-targets -- -D warnings`, `build_runner`, `flutter test`,
      `./desktop/verify.sh`.
- [x] 5.2 Confirm no existing vector case was edited — an edit means the wire format was
      broken rather than extended.
- [x] 5.3 In the built app: an empty date box still rolls a passed time to tomorrow; the box
      is absent from Keep Awake; a past date shows the rejection live in the preview.

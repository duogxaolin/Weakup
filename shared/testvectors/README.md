# Shared test vectors

Language-agnostic scheduling scenarios executed by **both** implementations:

- Rust desktop — `desktop/src-tauri/tests/shared_vectors.rs`
- Dart mobile — `mobile/test/shared/shared_vectors_test.dart`

## Why these exist

The pure scheduling logic (~928 lines) is implemented twice, in Rust for desktop and
in Dart for mobile. Duplication was accepted deliberately (see
`openspec/changes/split-desktop-to-tauri/design.md`, D3) because FFI was
disproportionate at this size. The risk that decision creates is *silent drift*: a
rule corrected in one language and forgotten in the other, with both test suites
still green because each only checks itself.

These vectors close that gap. Both suites read these exact files, so a rule that
diverges turns one suite red and names the case.

## Rule

Any behavioral rule that both implementations must share needs a vector case here.
Until both suites execute it, the rule is not cross-implementation-verified — no
matter how well either side unit-tests it privately.

## Files

| File | Covers |
| --- | --- |
| `resolution.json` | Trigger → absolute UTC target instant, including both DST anomalies |
| `reconciliation.json` | Overdue job → outcome, including the 15-minute power-off tolerance |
| `validation.json` | Trigger validation accept/reject boundaries |

## Format

All timestamps are ISO-8601 with an explicit `Z` offset. `now` is always supplied
rather than read from the clock, so cases are deterministic and DST edges are
reachable on any machine in any zone.

### `resolution.json`

```json
{
  "id": "unique-case-id",
  "description": "what this case pins down and why it matters",
  "now": "2026-07-30T07:00:00Z",
  "timezone": "Asia/Ho_Chi_Minh",
  "trigger": { "kind": "absoluteTime", "hour": 20, "minute": 0 },
  "expectedTargetInstantUtc": "2026-07-30T13:00:00Z"
}
```

`expectedTargetInstantUtc` is `null` for an indefinite trigger, which has no target
by design.

A case may instead expect a rejection, which a dated `absoluteTime` trigger makes possible:

```json
{
  "id": "absolute-dated-past-rejected",
  "description": "...",
  "now": "2026-07-30T07:00:00Z",
  "timezone": "Asia/Ho_Chi_Minh",
  "trigger": { "kind": "absoluteTime", "hour": 20, "minute": 0, "date": "2026-07-01" },
  "expectedErrorContains": "already passed"
}
```

A case sets **exactly one** of `expectedTargetInstantUtc` and `expectedErrorContains`; both
harnesses fail a case that sets both or neither. The second key is necessary rather than
convenient: `expectedTargetInstantUtc: null` already means "this trigger has no instant" and
so cannot be overloaded to mean "this trigger is refused".

### `reconciliation.json`

```json
{
  "id": "unique-case-id",
  "description": "...",
  "jobType": "powerOff",
  "targetInstantUtc": "2026-07-30T09:55:00Z",
  "now": "2026-07-30T10:00:00Z",
  "expectedOutcome": "proceedToGracePeriod"
}
```

`expectedOutcome` is one of `stillPending`, `completed`, `proceedToGracePeriod`,
`overdue`.

**`proceedToGracePeriod` never means "power off now."** It means the job enters the
mandatory, cancellable 60-second countdown. No vector outcome authorizes immediate
execution.

### `validation.json`

```json
{
  "id": "unique-case-id",
  "description": "...",
  "jobType": "powerOff",
  "trigger": { "kind": "duration", "minutes": 1441 },
  "expectedValid": false,
  "expectedErrorContains": "24 hours"
}
```

`expectedErrorContains` is a substring match, so the two implementations may word
messages differently while still agreeing on the substantive reason. It is omitted
when `expectedValid` is `true`.

## Trigger wire format

| `kind` | Fields |
| --- | --- |
| `indefinite` | none |
| `duration` | `minutes` (integer) |
| `absoluteTime` | `hour` (0-23), `minute` (0-59), optional `date` (`YYYY-MM-DD`) |

### The two `absoluteTime` semantics

`date` is optional, and its presence changes what the trigger means:

- **Omitted** — a time of day, resolved to its next occurrence. If that time has already
  passed today it rolls to tomorrow. This is the original and only prior behaviour, so an
  undated trigger serialises byte-identically to before and every pre-existing case is
  unchanged.
- **Present** — a one-off instant on exactly that local date. It does **not** roll forward. A
  dated instant that is not strictly in the future is refused, because silently moving an
  irreversible power-off to a day the user never chose is worse than refusing.

The pair `absolute-past-rolls-to-tomorrow` and `absolute-dated-does-not-roll-forward` exists
to pin that difference: identical `now`, zone, and time-of-day, differing only by the date,
with opposite outcomes.

### Where the past-date rule lives, and why

Validation asks only questions that need no context; resolution asks the ones that need a
clock and a zone. That split is not new — duration bounds are already validated while the DST
anomalies are resolved — and the date follows it:

| Question | Needs | Home |
| --- | --- | --- |
| Is the month 1-12? Is `2026-02-30` a real day? | nothing | the parse boundary |
| Are the hour and minute in range? | nothing | `validate` |
| Has `2026-07-01T20:00` already passed? | `now`, timezone | `resolve` |

So `validate` keeps its signature — it receives no `now` in either language, and
`validation.json` cases carry no clock — and the past-date rejection appears in
`resolution.json` instead.

**Malformed dates are deliberately not vector cases.** A value like `2026-02-30` or a month of
13 cannot be represented in either implementation's parsed trigger form (Rust `NaiveDate`,
Dart `CalendarDate`), so such a case would make the vector *file* unparseable rather than
exercise a rule. Well-formedness is enforced where the trigger is parsed, and is tested per
language.

## Verified DST reference values

The DST cases use values confirmed by running `chrono-tz` rather than derived by
hand, since getting these wrong silently is the exact failure these vectors exist to
prevent:

| Local wall time | Zone | Result |
| --- | --- | --- |
| 2026-03-08 02:30 | America/New_York | does not exist (spring-forward gap); clock jumps to 03:00 EDT = `07:00Z` |
| 2026-11-01 01:30 | America/New_York | occurs twice (fall-back); earlier = `05:30Z`, later = `06:30Z` |

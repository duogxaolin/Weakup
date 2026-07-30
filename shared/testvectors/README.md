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
| `absoluteTime` | `hour` (0-23), `minute` (0-59) |

## Verified DST reference values

The DST cases use values confirmed by running `chrono-tz` rather than derived by
hand, since getting these wrong silently is the exact failure these vectors exist to
prevent:

| Local wall time | Zone | Result |
| --- | --- | --- |
| 2026-03-08 02:30 | America/New_York | does not exist (spring-forward gap); clock jumps to 03:00 EDT = `07:00Z` |
| 2026-11-01 01:30 | America/New_York | occurs twice (fall-back); earlier = `05:30Z`, later = `06:30Z` |

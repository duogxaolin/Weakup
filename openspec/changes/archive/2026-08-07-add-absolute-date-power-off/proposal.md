# Add calendar-date selection to the Power Off schedule

## Why

Power Off currently offers two triggers: a duration, and a time of day. The time-of-day
trigger is a *recurring-alarm* semantic — it resolves to the next occurrence of that wall
clock time, rolling to tomorrow if today's has already passed. There is no way to express
"shut down at 23:00 on 10 August".

A user asked for the missing case: pick the day, month, and year alongside the time.

## What changes

An optional `date` on the existing `absoluteTime` trigger, in both implementations.

```json
{ "kind": "absoluteTime", "hour": 22, "minute": 30 }                        // next 22:30, may be tomorrow
{ "kind": "absoluteTime", "hour": 22, "minute": 30, "date": "2026-08-10" }  // exactly that date
```

The dated form is a **one-off instant**, not an alarm. It therefore differs from the undated
form in one important way: it does **not** roll forward. A dated target that has already
passed is rejected with a message, because silently moving an irreversible power-off to a day
the user never chose is worse than refusing.

Deliberately narrow:

- The date control appears on **Power Off only** in the desktop UI. The domain accepts a
  dated keep-awake trigger — nothing about the rules forbids it — but no UI offers one.
- The mobile app gains the domain rule and the database column, so the shared contract is
  honoured on both sides, but **no Flutter date picker** in this change.
- **No new Tauri command.** The date rides the existing `resolve_trigger` and job-creation
  commands, so the IPC surface is unchanged.
- The mandatory cancellable 60-second grace period and every other power-off safety
  invariant are untouched.

## Impact

- Specs: `power-job-scheduling` — the absolute-time resolution requirement, which currently
  states unconditionally that a passed time rolls to the following day.
- Wire format: additive and backward compatible. An undated trigger serialises byte-identically
  to today, so every existing shared vector case passes unmodified.
- Storage: a nullable `trigger_date` column in both databases (Rust `SCHEMA_VERSION` 2 → 3,
  Drift `schemaVersion` 1 → 2). Existing rows read back as undated.
- Scheduler: `resume_job` and `apply_timezone_change` re-resolve from the current clock, and
  resolution is now fallible for a dated job whose date has passed. Both paths need an
  explicit decision rather than an inherited one.

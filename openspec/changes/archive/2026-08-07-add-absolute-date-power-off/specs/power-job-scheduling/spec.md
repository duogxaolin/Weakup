## MODIFIED Requirements

### Requirement: Absolute-time trigger resolves to a timezone-aware future instant
The system SHALL resolve an `absoluteTime` trigger to a concrete instant in the device's
current IANA timezone. The trigger MAY carry an optional calendar `date`, and the two forms
have deliberately different semantics:

- **Without a date** (a time of day), the system SHALL resolve to that time-of-day on the
  current local date, and if that instant has already passed the system SHALL resolve it to
  the same time-of-day on the following day.
- **With a date** (a one-off instant), the system SHALL resolve to that time-of-day on that
  exact local date and SHALL NOT roll forward. If the resulting instant is not strictly in
  the future, the system SHALL reject resolution with a validation error stating that the
  date and time have already passed.

Both forms SHALL resolve the two DST anomalies identically, since a dated trigger is the same
wall-clock-to-instant mapping applied to a date the user supplied rather than today's.

Calendar well-formedness of the `date` — that the month is 1-12 and the day exists in that
month of that year — SHALL be enforced when the trigger is parsed, in both implementations,
so that a malformed date cannot reach the resolver at all.

#### Scenario: Future time today resolves to today
- **WHEN** the current local time is 14:00 and the user schedules `absoluteTime` 20:00
- **THEN** the resolved target instant SHALL be 20:00 today in the device's timezone

#### Scenario: Past time today resolves to tomorrow
- **WHEN** the current local time is 21:00 and the user schedules `absoluteTime` 20:00 with no date
- **THEN** the resolved target instant SHALL be 20:00 tomorrow in the device's timezone

#### Scenario: Dated trigger resolves to that exact date
- **WHEN** the current local date is 30 July and the user schedules `absoluteTime` 20:00 with date 10 August
- **THEN** the resolved target instant SHALL be 20:00 on 10 August in the device's timezone

#### Scenario: Dated trigger does not roll forward
- **WHEN** the current local time is 21:00 and the user schedules `absoluteTime` 20:00 with today's date
- **THEN** the system SHALL reject the trigger rather than resolving it to 20:00 tomorrow

#### Scenario: Dated trigger equal to now is rejected
- **WHEN** the requested dated instant is exactly the current instant
- **THEN** the system SHALL reject it, since a target must be strictly in the future

#### Scenario: Malformed date is rejected at the boundary
- **WHEN** a trigger arrives carrying a date such as `2026-02-30` or a month of 13
- **THEN** the system SHALL reject it as unparseable rather than resolving it to a nearby real date

#### Scenario: Spring-forward gap resolves to the jump instant
- **WHEN** the requested time-of-day falls inside a DST spring-forward gap that does not exist on the target date
- **THEN** the resolved target instant SHALL be the instant the clock jumps forward to

#### Scenario: Fall-back overlap resolves to the earlier occurrence
- **WHEN** the requested time-of-day occurs twice on the target date due to a DST fall-back
- **THEN** the resolved target instant SHALL be the first (earlier) occurrence

### Requirement: System clock or timezone change invalidates and re-resolves absolute-time jobs
The system SHALL detect a system clock or timezone change while a job is pending. For jobs
using an `absoluteTime` trigger, the system SHALL re-resolve `targetInstantUtc` against the
new timezone and SHALL notify the user if the effective target moved.

Where re-resolution fails — which a dated trigger makes possible, since a timezone change can
move a dated target into the past — the system SHALL leave that job's stored
`targetInstantUtc` unchanged and SHALL continue processing the remaining jobs. The passed
target is then classified by the existing reconciliation rules on the next tick, rather than
by a second overdue concept.

#### Scenario: Timezone change shifts an absolute-time job
- **WHEN** the device's timezone changes while an `absoluteTime` `powerOff` job is pending
- **THEN** the system SHALL recompute `targetInstantUtc` in the new timezone and notify the user of the updated time

#### Scenario: Duration-based job is unaffected by timezone change
- **WHEN** the device's timezone changes while a `duration` job is pending
- **THEN** the system SHALL leave the job's `targetInstantUtc` unchanged, since it was anchored at creation time independent of timezone

#### Scenario: Timezone change moving a dated target into the past does not abort the sweep
- **WHEN** a timezone change moves a dated `absoluteTime` job's target into the past, and other timezone-sensitive jobs are also pending
- **THEN** the system SHALL leave that job's `targetInstantUtc` unchanged and SHALL still re-resolve the remaining jobs

## ADDED Requirements

### Requirement: Resuming a paused job whose dated target has passed fails without rescheduling
Resuming a paused job re-resolves its trigger against the current clock. Where that
resolution fails because the job carries a dated `absoluteTime` trigger whose instant has
passed, the system SHALL leave the job `Paused` and SHALL report the reason, rather than
rescheduling it to a later instant the user did not choose.

#### Scenario: Resume of a passed dated power-off is refused
- **WHEN** the user resumes a paused `powerOff` job dated for an instant that has already passed
- **THEN** the system SHALL refuse the resume, state that the date and time have passed, and leave the job `Paused`

#### Scenario: Resume of an undated absolute-time job still rolls forward
- **WHEN** the user resumes a paused `powerOff` job whose undated `absoluteTime` has passed for today
- **THEN** the system SHALL resume it against the next occurrence, unchanged from existing behaviour

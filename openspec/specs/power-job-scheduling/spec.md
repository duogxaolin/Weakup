# Power Job Scheduling Specification

## Purpose

Defines the job model and the timing rules that decide when a job fires. A job is exactly one of
`keepAwake` or `powerOff` and carries exactly one trigger — `indefinite`, `duration`, or
`absoluteTime` — with validation for each, including the timezone-aware resolution of a
time-of-day to a concrete instant and the DST gap and overlap cases that resolution has to answer.

The central rule is that a job's target is persisted as an absolute UTC instant rather than a
decrementing countdown, which is what makes the remaining behaviors decidable: recomputing time
left after a restart, reconciling jobs whose target passed while the app was closed, re-registering
across a device reboot where the OS permits it, and re-resolving absolute-time jobs when the clock
or timezone changes. This spec covers when a job fires and what state it lands in, not how the
action itself is carried out or displayed.
## Requirements
### Requirement: Job types and trigger kinds
The system SHALL model a job as one of exactly two types — `keepAwake` or `powerOff` — and each job SHALL have exactly one trigger of kind `indefinite`, `duration`, or `absoluteTime`.

#### Scenario: Keep-awake job accepts all three trigger kinds
- **WHEN** a user creates a `keepAwake` job
- **THEN** the system SHALL allow `indefinite`, `duration`, or `absoluteTime` as the trigger kind

#### Scenario: Power-off job rejects indefinite trigger
- **WHEN** a user attempts to create a `powerOff` job with trigger kind `indefinite`
- **THEN** the system SHALL reject creation with a validation error stating that power-off requires a duration or a specific time

### Requirement: Duration trigger validation
The system SHALL reject a `duration` trigger whose value is zero, negative, or exceeds 24 hours (1440 minutes).

#### Scenario: Zero duration rejected
- **WHEN** a user enters a duration of 0 minutes
- **THEN** the system SHALL reject creation with a validation error stating the duration must be positive

#### Scenario: Negative duration rejected
- **WHEN** a duration value less than zero is submitted
- **THEN** the system SHALL reject creation with a validation error

#### Scenario: Duration above maximum rejected
- **WHEN** a user enters a duration greater than 1440 minutes
- **THEN** the system SHALL reject creation with a validation error stating the maximum accepted duration is 24 hours

#### Scenario: Duration at maximum accepted
- **WHEN** a user enters a duration of exactly 1440 minutes
- **THEN** the system SHALL accept the job

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

### Requirement: Target instant is persisted, not a remaining-seconds countdown
The system SHALL persist each job's resolved target as an absolute UTC instant (`targetInstantUtc`) together with the original trigger definition. The system SHALL NOT persist a decrementing remaining-time value as the source of truth.

#### Scenario: Countdown recomputed after restart
- **WHEN** the app is closed and reopened while a job is pending
- **THEN** the displayed remaining time SHALL be computed as `targetInstantUtc − now` at display time, not read from a stored countdown value

### Requirement: At most one active job per type
The system SHALL allow at most one active (non-completed, non-cancelled) job of each type (`keepAwake`, `powerOff`) at a time. Creating a second active job of a type that already has an active job SHALL require explicit user confirmation to replace the existing job, and the replacement SHALL be performed atomically.

#### Scenario: Second job of same type prompts for replacement
- **WHEN** a `powerOff` job is already active and the user attempts to create another `powerOff` job
- **THEN** the system SHALL prompt the user to confirm replacing the existing job before creating the new one

#### Scenario: Confirmed replacement is atomic
- **WHEN** the user confirms replacement
- **THEN** the system SHALL cancel the existing job and activate the new job within a single transaction such that no intermediate state has zero or two active jobs of that type

#### Scenario: Jobs of different types coexist
- **WHEN** an active `keepAwake` job exists and the user creates a `powerOff` job
- **THEN** the system SHALL activate the `powerOff` job without affecting the `keepAwake` job

### Requirement: Overdue job reconciliation on app resume
On every app resume (including cold start), the system SHALL compare each pending job's `targetInstantUtc` to the current time and reconcile as follows: an overdue `keepAwake` job SHALL be marked completed with the wakelock released; an overdue `powerOff` job whose target has passed by no more than 15 minutes SHALL proceed through the normal grace-period countdown; an overdue `powerOff` job whose target has passed by more than 15 minutes SHALL be marked overdue, SHALL NOT trigger a power-off, and SHALL notify the user.

#### Scenario: Overdue keep-awake job completes silently on resume
- **WHEN** the app resumes and a `keepAwake` job's target instant is in the past
- **THEN** the system SHALL mark the job completed and release the wakelock without prompting the user

#### Scenario: Recently-overdue power-off job still executes with grace period
- **WHEN** the app resumes and a `powerOff` job's target instant is 5 minutes in the past
- **THEN** the system SHALL begin the grace-period countdown before executing power-off

#### Scenario: Long-overdue power-off job does not execute
- **WHEN** the app resumes and a `powerOff` job's target instant is 30 minutes in the past
- **THEN** the system SHALL NOT execute power-off, SHALL mark the job overdue, and SHALL notify the user that the scheduled power-off was skipped

### Requirement: Jobs survive app restart and, where the OS permits, device reboot
The system SHALL persist all jobs to local storage such that they are recoverable after an app restart. On platforms where the OS permits background re-registration after device reboot (Windows, macOS, Linux, Android), the system SHALL re-register pending jobs after reboot. On iOS, where reboot re-registration is not possible, the system SHALL state this limitation to the user.

A job's origin — whether it was scheduled at this machine or created by an authorized remote command — SHALL be persisted with the job and SHALL be recovered unchanged. The countdown a power-off job receives is determined by its origin, so an origin that does not survive a restart silently converts a remote-initiated shutdown into one with the shorter local countdown. A job recovered from storage SHALL therefore receive exactly the countdown its original origin earns, not the countdown of a default.

Rows written before the origin was persisted SHALL be recovered as locally scheduled, which is what every such row meant when it was written.

#### Scenario: Pending job recovered after app restart
- **WHEN** the app is force-quit while a job is pending and then relaunched
- **THEN** the job SHALL appear in its prior state with its original `targetInstantUtc` intact

#### Scenario: Android job re-registers after device reboot
- **WHEN** an Android device reboots while a job is pending
- **THEN** the system SHALL re-register the job's scheduling via the boot-completed receiver without requiring the user to reopen the app first

#### Scenario: iOS states reboot limitation
- **WHEN** an iOS user views a pending job
- **THEN** the system SHALL state that the job will not survive a device reboot without reopening the app

#### Scenario: A remote-origin job keeps its origin across a restart
- **WHEN** a job created by an authorized remote command is persisted and then recovered after an app restart
- **THEN** it SHALL be recovered as remote-origin and SHALL receive the remote countdown, not the local one

#### Scenario: A job written before origin was persisted reads as local
- **WHEN** a job row written by an earlier version of the app, which stored no origin, is recovered
- **THEN** it SHALL be recovered as locally scheduled

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

### Requirement: Scheduling behavior is identical across implementations
Where the same scheduling rule is implemented in more than one language, the implementations SHALL agree on observable behavior, and that agreement SHALL be enforced by the shared test vectors rather than by inspection. This applies to target-instant resolution, the overdue-tolerance boundary, and trigger validation limits.

#### Scenario: Both implementations resolve a given trigger identically
- **WHEN** the same `(now, timezone, trigger)` input is resolved by the Rust desktop implementation and the Dart mobile implementation
- **THEN** both SHALL produce the same `target_instant_utc`

#### Scenario: Both implementations agree on the overdue tolerance
- **WHEN** the same overdue `powerOff` job state is reconciled by both implementations
- **THEN** both SHALL produce the same outcome, including at the 15-minute boundary

#### Scenario: Both implementations apply the same validation limits
- **WHEN** a `duration` trigger of 0, a negative value, 1440 minutes, or 1441 minutes is validated by either implementation
- **THEN** both SHALL accept and reject the same values with equivalent errors

### Requirement: Desktop scheduler survives process suspension without drifting
On desktop, the system SHALL recompute remaining time from the persisted absolute target instant after the host machine sleeps, hibernates, or suspends, and SHALL NOT rely on a timer having continued to run during suspension.

#### Scenario: Machine sleeps and wakes before the target instant
- **WHEN** the host machine sleeps for 30 minutes while a job's target instant is 2 hours away
- **THEN** on wake the system SHALL recompute the remaining time from `target_instant_utc − now` and the job SHALL still fire at its original target instant

#### Scenario: Machine sleeps through the target instant of a power-off job
- **WHEN** the host machine sleeps past a `powerOff` job's target instant and wakes more than 15 minutes after it
- **THEN** the system SHALL mark the job overdue, SHALL NOT power off the machine, and SHALL notify the user that the scheduled power-off was skipped

#### Scenario: Machine sleeps through the target instant of a keep-awake job
- **WHEN** the host machine sleeps past a `keepAwake` job's target instant
- **THEN** on wake the system SHALL mark the job completed and release the screen-awake assertion


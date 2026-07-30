## ADDED Requirements

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
The system SHALL resolve an `absoluteTime` trigger (a time-of-day) to a concrete `TZDateTime` in the device's current IANA timezone. If the given time-of-day has already passed for the current date, the system SHALL resolve it to the same time-of-day on the following day.

#### Scenario: Future time today resolves to today
- **WHEN** the current local time is 14:00 and the user schedules `absoluteTime` 20:00
- **THEN** the resolved target instant SHALL be 20:00 today in the device's timezone

#### Scenario: Past time today resolves to tomorrow
- **WHEN** the current local time is 21:00 and the user schedules `absoluteTime` 20:00
- **THEN** the resolved target instant SHALL be 20:00 tomorrow in the device's timezone

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

#### Scenario: Pending job recovered after app restart
- **WHEN** the app is force-quit while a job is pending and then relaunched
- **THEN** the job SHALL appear in its prior state with its original `targetInstantUtc` intact

#### Scenario: Android job re-registers after device reboot
- **WHEN** an Android device reboots while a job is pending
- **THEN** the system SHALL re-register the job's scheduling via the boot-completed receiver without requiring the user to reopen the app first

#### Scenario: iOS states reboot limitation
- **WHEN** an iOS user views a pending job
- **THEN** the system SHALL state that the job will not survive a device reboot without reopening the app

### Requirement: System clock or timezone change invalidates and re-resolves absolute-time jobs
The system SHALL detect a system clock or timezone change while a job is pending. For jobs using an `absoluteTime` trigger, the system SHALL re-resolve `targetInstantUtc` against the new timezone and SHALL notify the user if the effective target moved.

#### Scenario: Timezone change shifts an absolute-time job
- **WHEN** the device's timezone changes while an `absoluteTime` `powerOff` job is pending
- **THEN** the system SHALL recompute `targetInstantUtc` in the new timezone and notify the user of the updated time

#### Scenario: Duration-based job is unaffected by timezone change
- **WHEN** the device's timezone changes while a `duration` job is pending
- **THEN** the system SHALL leave the job's `targetInstantUtc` unchanged, since it was anchored at creation time independent of timezone

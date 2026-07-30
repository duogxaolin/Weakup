## ADDED Requirements

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

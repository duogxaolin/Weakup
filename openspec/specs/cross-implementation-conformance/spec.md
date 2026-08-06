# cross-implementation-conformance Specification

## Purpose
TBD - created by archiving change split-desktop-to-tauri. Update Purpose after archive.
## Requirements
### Requirement: Shared test vectors are the cross-language behavioral contract
The system SHALL maintain a set of language-agnostic JSON test vectors under `shared/testvectors/` that define expected scheduling behavior independent of implementation language. Both the Rust desktop implementation and the Dart mobile implementation SHALL execute these vectors as part of their own test suites.

#### Scenario: Both implementations execute the same vector files
- **WHEN** the desktop test suite and the mobile test suite are run
- **THEN** both SHALL parse and execute every vector file in `shared/testvectors/`, and neither SHALL maintain a private copy of the vector data

#### Scenario: Divergence in one implementation fails that implementation's suite
- **WHEN** a scheduling rule is changed in one implementation such that it no longer matches a shared vector
- **THEN** that implementation's test suite SHALL fail, identifying the specific vector case that diverged

#### Scenario: A new rule requires a vector before it is considered covered
- **WHEN** a behavioral rule that both implementations must share is added or changed
- **THEN** a corresponding vector case SHALL be added to `shared/testvectors/`, and the rule SHALL NOT be considered cross-implementation-verified until both suites execute it

### Requirement: Vector coverage of target-instant resolution
The shared test vectors SHALL cover absolute-time and duration trigger resolution, including the DST edge cases, expressed as an input `(now, timezone, trigger)` tuple and an expected `target_instant_utc` value.

#### Scenario: Future time today
- **WHEN** a vector specifies a local time of 14:00 with an `absoluteTime` trigger of 20:00
- **THEN** both implementations SHALL resolve the target instant to 20:00 the same day in the specified timezone

#### Scenario: Past time rolls to tomorrow
- **WHEN** a vector specifies a local time of 21:00 with an `absoluteTime` trigger of 20:00
- **THEN** both implementations SHALL resolve the target instant to 20:00 the following day

#### Scenario: Spring-forward gap
- **WHEN** a vector specifies an `absoluteTime` falling inside a DST spring-forward gap
- **THEN** both implementations SHALL resolve to the instant the clock jumps forward to

#### Scenario: Fall-back overlap
- **WHEN** a vector specifies an `absoluteTime` that occurs twice due to a DST fall-back
- **THEN** both implementations SHALL resolve to the earlier of the two occurrences

#### Scenario: Duration trigger anchored at creation
- **WHEN** a vector specifies a `duration` trigger
- **THEN** both implementations SHALL resolve the target instant as `now + duration`, independent of timezone

### Requirement: Vector coverage of overdue reconciliation
The shared test vectors SHALL cover overdue-job reconciliation outcomes, including the 15-minute power-off tolerance boundary, expressed as an input `(job type, target_instant_utc, now)` tuple and an expected reconciliation outcome.

#### Scenario: Overdue keep-awake completes
- **WHEN** a vector specifies a `keepAwake` job whose target instant has passed
- **THEN** both implementations SHALL produce the `completed` outcome

#### Scenario: Power-off within tolerance proceeds to grace period
- **WHEN** a vector specifies a `powerOff` job overdue by 5 minutes
- **THEN** both implementations SHALL produce the outcome that proceeds to the grace-period countdown

#### Scenario: Power-off beyond tolerance is refused
- **WHEN** a vector specifies a `powerOff` job overdue by 30 minutes
- **THEN** both implementations SHALL produce the `overdue` outcome that does NOT execute power-off

#### Scenario: Tolerance boundary is specified exactly
- **WHEN** a vector specifies a `powerOff` job overdue by exactly 15 minutes
- **THEN** both implementations SHALL agree on the outcome at the boundary, and the vector SHALL make the inclusive-or-exclusive choice explicit


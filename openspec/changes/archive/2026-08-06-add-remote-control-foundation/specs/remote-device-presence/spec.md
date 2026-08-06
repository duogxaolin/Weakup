## Purpose

Defines how a device's liveness is derived from the last time it reported in, so that a user
looking at one of their devices from another can tell the difference between "reachable right
now", "was here recently but may be asleep", and "gone".

The rule is implemented and vector-verified in both Rust and Dart, but nothing yet reports a
`lastSeen`, so until a transport supplies one it decides no live data — the states below describe
what the rule returns, not a liveness any surface can currently show.

## ADDED Requirements

### Requirement: Presence is a three-state value, never a boolean
The system SHALL derive a device's presence from its last report time and the current instant,
and SHALL express the result as exactly one of three states: `online`, `stale`, or `offline`.
The system SHALL NOT collapse presence into a two-state reachable/unreachable value in its
model or in any user-facing surface.

The three states are distinguished because a mobile device that has been suspended by its
operating system is neither reachable nor gone, and reporting it as either one misleads the
user: shown as online, a command sent to it appears to be ignored; shown as offline, the user
concludes the device has stopped working.

#### Scenario: A device that reported within the online window is online
- **WHEN** presence is evaluated for a device whose last report is more recent than the online threshold
- **THEN** the system SHALL report `online`

#### Scenario: A device silent past the online threshold but within the offline threshold is stale
- **WHEN** presence is evaluated for a device whose last report is older than the online threshold but not older than the offline threshold
- **THEN** the system SHALL report `stale`, and SHALL NOT report either `online` or `offline`

#### Scenario: A device silent past the offline threshold is offline
- **WHEN** presence is evaluated for a device whose last report is older than the offline threshold
- **THEN** the system SHALL report `offline`

#### Scenario: A device that has never reported is offline
- **WHEN** presence is evaluated for a device with no recorded last report
- **THEN** the system SHALL report `offline` rather than treating the absent value as the current instant

### Requirement: Presence thresholds are fixed and identical across implementations
The system SHALL use one fixed online threshold and one fixed offline threshold, with the
online threshold strictly shorter than the offline threshold. Both values SHALL be identical in
every implementation, SHALL NOT be user-configurable, and SHALL be verified by shared
cross-implementation test vectors that supply the current instant explicitly rather than
reading a clock.

#### Scenario: Both implementations agree on every boundary
- **WHEN** the shared presence vectors are executed by the desktop and mobile implementations
- **THEN** both SHALL produce the same presence state for every case, including the cases that sit exactly on each threshold

#### Scenario: A case exactly on the online threshold is decided, not left ambiguous
- **WHEN** presence is evaluated for a device whose silence equals the online threshold exactly
- **THEN** the system SHALL return the same state in both implementations, fixed by a shared vector case rather than by each implementation's own comparison operator

### Requirement: Presence staleness is reported with its age, not just its state
When the system presents a device that is not `online`, it SHALL make the age of the last
report available alongside the state, so a user can distinguish a device silent for one minute
from one silent for a day. Presence evaluation SHALL therefore expose the elapsed time it
based its decision on, not only the resulting state.

#### Scenario: A stale device carries how long it has been silent
- **WHEN** presence evaluates to `stale` or `offline`
- **THEN** the result SHALL include the elapsed time since the last report

#### Scenario: An online device needs no age
- **WHEN** presence evaluates to `online`
- **THEN** the elapsed time SHALL still be available, so a caller does not have to branch on the state to obtain it

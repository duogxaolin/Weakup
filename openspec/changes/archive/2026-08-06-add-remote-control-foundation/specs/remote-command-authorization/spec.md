## Purpose

Decides whether a command arriving from another device is allowed to act on this device, and
guarantees that an allowed power-off request still passes through the same cancellable countdown
a locally scheduled one does.

Enforcement state, stated here because it is easy to misread from the requirements alone: the
authorization rule is implemented in both Rust and Dart and verified by shared cross-implementation
vectors, but no transport yet invokes it — nothing in this system accepts a remote command. The
scenarios below that describe what happens to an arriving command therefore constrain the change
that adds the transport, rather than describing behavior available today. The one exception is the
prohibition on reaching the power-off executor: that is enforced now, by a source-text test
(`desktop/src-tauri/src/domain/remote_command_tests.rs`) that fails if such a reference is
introduced, and it is the half that must not regress when the transport lands.

## ADDED Requirements

### Requirement: A remote request creates a job and never executes an action directly
An authorized remote command SHALL be carried out by creating or modifying a job on the target
device, which the target's existing local scheduler then runs by its existing rules. No remote
command SHALL invoke a power-off, acquire a keep-awake assertion, or otherwise take a platform
action directly.

This is what keeps a remote request from bypassing the mandatory countdown: the countdown is
reached because the job path is the only path, not because the remote path chooses to be polite.
The system SHALL enforce this structurally — there SHALL be no code path from a remote command
to the power-off executor or to the countdown gate — and SHALL verify the absence of such a path
by test rather than by review.

#### Scenario: An authorized remote power-off produces a job, not a shutdown
- **WHEN** an authorized remote power-off command is accepted by the target device
- **THEN** the system SHALL create a `powerOff` job on the target and SHALL NOT invoke the platform shutdown command as part of handling the command

#### Scenario: No remote type can reach the power-off executor
- **WHEN** the source of the remote command handling is inspected by the test suite
- **THEN** it SHALL contain no reference that would invoke the power-off executor or the countdown gate directly, and the test SHALL fail if such a reference is introduced

#### Scenario: An authorized remote cancel goes through the same job path
- **WHEN** an authorized remote command cancels a scheduled power-off on the target
- **THEN** the system SHALL cancel the job through the target's existing job-cancellation path

### Requirement: Authorization is a pure decision with an enumerated reason for every refusal
The system SHALL decide whether to accept a remote command from the command, the target's
platform capabilities, whether the requesting device is paired with the target, and whether the
target's owner has enabled remote control. The decision SHALL be either accepted or refused with
exactly one reason drawn from a closed set, so that every refusal can be explained to the user
in specific terms rather than as a generic failure.

The reasons SHALL distinguish at minimum: the requesting device is not paired with the target;
remote control is disabled on the target; the target's platform cannot perform the requested
action; and the command is not one the target accepts remotely.

#### Scenario: An unpaired requester is refused with the pairing reason
- **WHEN** a remote command arrives from a device that is not paired with the target
- **THEN** the system SHALL refuse it and SHALL report that the requesting device is not paired, distinct from every other refusal reason

#### Scenario: A refusal because the platform cannot act is distinguished from a permission refusal
- **WHEN** a remote power-off command targets a device whose platform capabilities report that power-off is not supported
- **THEN** the system SHALL refuse it with the platform-capability reason and SHALL NOT report it as a pairing or permission problem

#### Scenario: Both implementations decide every case identically
- **WHEN** the shared remote-authorization vectors are executed by the desktop and mobile implementations
- **THEN** both SHALL return the same decision and the same refusal reason for every case

### Requirement: Remote control is disabled by default and can only be enabled at the target
The system SHALL default remote control to disabled on every device. Enabling it SHALL be
possible only from the target device itself; the system SHALL refuse any remote command that
would enable remote control, or otherwise change the target's remote-control setting, on a
device where it is currently disabled.

Without this, a single compromised account would be enough to turn on remote control everywhere
and then power off every device on it. Requiring physical presence to grant the permission keeps
account access alone from being sufficient.

#### Scenario: Remote control counts as on only when it is affirmatively enabled
- **WHEN** a remote command is evaluated against a target whose remote-control setting is anything other than affirmatively enabled
- **THEN** the system SHALL treat remote control as disabled and SHALL refuse the command with the disabled reason

#### Scenario: A remote command cannot enable remote control
- **WHEN** a remote command that would enable remote control arrives at a device where remote control is disabled
- **THEN** the system SHALL refuse it, and SHALL report that remote control must be enabled on the target device itself

### Requirement: Pairing is per-device and revocable, not implied by account access
The system SHALL treat a device as an authorized requester only if that specific device has been
paired with the target. Being signed in to the same account SHALL NOT by itself authorize a
device to command another. Revoking a pairing SHALL cause subsequent commands from that device to
be refused with the pairing reason.

#### Scenario: Same account is not sufficient
- **WHEN** a remote command arrives from a device signed in to the same account but not paired with the target
- **THEN** the system SHALL refuse it with the pairing reason

#### Scenario: A revoked pairing stops being authorized
- **WHEN** a pairing is revoked and a command subsequently arrives from that device
- **THEN** the system SHALL refuse it with the pairing reason

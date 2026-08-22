# remote-command-authorization Specification

## Purpose
Decides whether a command arriving from another device is allowed to act on this device, and
guarantees that an allowed power-off request still passes through the same cancellable countdown
a locally scheduled one does.

Enforcement state. The authorization rule is implemented in both Rust and Dart and verified by shared
cross-implementation vectors. The countdown half is now verified end to end rather than only asserted:
`desktop/src-tauri/tests/end_to_end_pairing_to_job.rs` pairs two devices through the real pairing
code path, routes a signed power-off through the transport fake, and confirms the target creates a
`JobOrigin::Remote` job whose countdown is read from that origin — not from a constant the test chose.
The origin survives the round trip through storage, so a job that outlived a restart would still get
the longer wait. That assertion was checked adversarially: substituting the local constant for the
origin-derived length makes the test fail with `left: 60, right: 300`, so it is testing the mapping
rather than agreeing with itself.

The prohibition on reaching the power-off executor still holds and is still enforced by source-text
tests rather than by review. No new Tauri command powers a machine off; a remote request creates a job
through the existing scheduler path, and `PowerOffGate` holds the only executor and takes no duration
argument, so there is no parameter through which a remote caller could ask for a shorter wait. The
remote countdown being strictly longer than the local one is a compile-time assertion.

The remote-control setting is implemented on both platforms: disabled by default, persisted, and
changeable only at the machine. Disabling does not revoke pairings — verified by disabling, confirming
the refusal, re-enabling, and confirming the same peer's command is obeyed again with no re-pairing.
A refusal for a disabled target reports the deliberately uninformative `NotPermitted` to the sender,
so a remote caller cannot probe the target's settings, while the specific `RemoteControlDisabled`
reason remains available at the layer that shows the owner what happened. Both are asserted.

What has changed since the previous statement of this capability is that a transport now exists to
invoke this rule. What has **not** changed is that no arriving command has ever been judged outside a
test. Two gaps in particular:

- **The desktop installs no transport.** `setup::initialise_pairing` constructs the remote surface
  with `None` in the transport slot, so at runtime nothing delivers a command to be evaluated. The
  arriving-command path is exercised only by the end-to-end suite, against the in-memory fake.
- **The mobile app is not a command target.** It can send, and its remote-control setting is read and
  persisted, but there is no arriving-command loop in `mobile/lib/` — nothing calls
  `receiveEnvelopes` outside tests. Its remote-control setting is therefore **enforced nowhere**: on
  that platform the setting is currently a stored preference rather than a gate.

One input this rule takes is not hypothetical. Whether the requesting device is paired is answered by
a durable pairing store rather than by whatever a caller passes: see the `remote-device-pairing`
capability. A revoked pairing is absent from that store rather than present and refused later, so a
revoked device fails authenticity before this rule is reached at all — verified end to end, including
the ordering, since a revoked peer's command is refused with `AuthenticityUnverified` rather than a
pairing reason.
## Requirements
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

Authorization SHALL be evaluated only after the command's authenticity has been established. The
input stating that the requesting device is paired with the target SHALL be derived from a pairing
the target itself holds and has verified the command against — never from an account session, an
owner record, or an assertion made by a relay. Authorization decides *permission*; it SHALL NOT be
the place where *identity* is assumed, because a rule that trusts a caller's claim of who it is
grants permission to whoever makes the claim.

#### Scenario: An unpaired requester is refused with the pairing reason
- **WHEN** a remote command arrives from a device that is not paired with the target
- **THEN** the system SHALL refuse it and SHALL report that the requesting device is not paired, distinct from every other refusal reason

#### Scenario: A refusal because the platform cannot act is distinguished from a permission refusal
- **WHEN** a remote power-off command targets a device whose platform capabilities report that power-off is not supported
- **THEN** the system SHALL refuse it with the platform-capability reason and SHALL NOT report it as a pairing or permission problem

#### Scenario: Both implementations decide every case identically
- **WHEN** the shared remote-authorization vectors are executed by the desktop and mobile implementations
- **THEN** both SHALL return the same decision and the same refusal reason for every case

#### Scenario: Permission is never evaluated for an unauthenticated command
- **WHEN** a command arrives whose authenticity could not be established
- **THEN** the system SHALL refuse it on authenticity grounds and SHALL NOT report any permission-related reason, so that permission state cannot be probed by unauthenticated callers

### Requirement: Remote control is disabled by default and can only be enabled at the target
The system SHALL default remote control to disabled on every device. Enabling it SHALL be
possible only from the target device itself; the system SHALL refuse any remote command that
would enable remote control, or otherwise change the target's remote-control setting, on a
device where it is currently disabled.

Without this, a single compromised account would be enough to turn on remote control everywhere
and then power off every device on it. Requiring physical presence to grant the permission keeps
account access alone from being sufficient.

The setting SHALL be presented to the owner at the target device, SHALL persist across restarts,
and SHALL default to disabled on a device that has never been configured. Turning it off SHALL take
effect for every command evaluated afterwards without requiring a restart or a network round trip.

Disabling remote control SHALL NOT revoke any pairing. The two are separate decisions: one says
"not right now", the other says "not this device, ever again". Collapsing them would make a user who
wanted a quiet evening re-pair every device the next morning.

#### Scenario: Remote control counts as on only when it is affirmatively enabled
- **WHEN** a remote command is evaluated against a target whose remote-control setting is anything other than affirmatively enabled
- **THEN** the system SHALL treat remote control as disabled and SHALL refuse the command with the disabled reason

#### Scenario: A remote command cannot enable remote control
- **WHEN** a remote command that would enable remote control arrives at a device where remote control is disabled
- **THEN** the system SHALL refuse it, and SHALL report that remote control must be enabled on the target device itself

#### Scenario: The setting survives a restart
- **WHEN** remote control is enabled at a target and the application is restarted
- **THEN** it SHALL remain enabled, and a device that has never been configured SHALL start disabled

#### Scenario: Turning it off takes effect immediately and offline
- **WHEN** the owner disables remote control at the target while no network is reachable
- **THEN** every command evaluated afterwards SHALL be refused with the disabled reason

#### Scenario: Disabling is not revoking
- **WHEN** remote control is disabled and later re-enabled at a target
- **THEN** the pairings that target held SHALL still be in effect, and no re-pairing SHALL be required

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

### Requirement: Every remote command decision is recorded
The system SHALL record every decision it reaches on an arriving remote command, whether accepted
or refused. Each record SHALL identify the device that sent the command, the command itself, the
instant of the decision, the decision reached, and — when refused — the single reason for refusal.

A power-off cannot be undone. Without a record, a user whose machine shut down has no way to
determine afterwards which device caused it, and a refused attempt leaves no trace that anything was
attempted at all. The record is what makes an irreversible action attributable, and it SHALL be
written for refusals as well as acceptances, because a series of refusals is the visible signature
of an attack in progress.

The record SHALL be readable at the target device without reference to any external service, so that
it remains available when no relay or network is reachable.

#### Scenario: An accepted power-off command is attributable
- **WHEN** a remote power-off command is accepted
- **THEN** the system SHALL record the sending device, the command, and the instant of the decision, such that the resulting shutdown can afterwards be attributed to that device

#### Scenario: A refusal is recorded with its reason
- **WHEN** a remote command is refused for any reason
- **THEN** the system SHALL record the refusal together with the single reason, so that repeated refused attempts are visible

#### Scenario: The record is readable without a network
- **WHEN** the owner inspects the record at the target device while no relay or network is reachable
- **THEN** the records SHALL be available


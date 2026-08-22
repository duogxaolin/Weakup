## MODIFIED Requirements

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

## ADDED Requirements

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

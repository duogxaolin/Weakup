# remote-device-pairing Specification

## Purpose
Defines what makes a pairing between two devices valid — how a pairing is granted, how long a grant
stays usable, and why pairing is the only act that confers authority to command another device.

Enforcement state, stated here because this capability is only partly built and the requirements
below do not distinguish the halves. What is implemented is grant *validity* alone: expiry against
each delivery method's lifetime, single use, and recognition of a grant the target actually issued.
That decision exists in both Rust (`desktop/src-tauri/src/domain/pairing_grant.rs`) and Dart
(`mobile/lib/domain/pairing_grant.dart`) and is verified by shared cross-implementation vectors,
though no caller yet presents a grant to it.

What is specified here but not implemented: revocation of a pairing at the target, out-of-band
delivery of a grant to the owner's established address, and the prohibition on a caller-supplied
delivery address. There is no pairing store, no pairing UI, and no mail delivery in this system.
Those requirements constrain the changes that add pairing storage, a pairing UI, and mail delivery
respectively, rather than describing behavior available today.

They are stated here deliberately rather than deferred. Each needs no new decision logic — only the
machinery to carry it out — and settling them now keeps the future implementer from reinventing
rules that have already been reasoned through. The honest reading of this capability is that the
decision layer is built and the machinery around it is not; it is not that these requirements are
optional.
## Requirements
### Requirement: Pairing is the only act that confers authority to command a device
The system SHALL treat a device as authorized to command a target only if that specific device has
been paired with that specific target. No other fact SHALL be sufficient — in particular, being
signed in to the same account, being registered to the same owner, or being listed as belonging to
the same person SHALL NOT by itself authorize a device to command another.

Account access and device authority are deliberately separated so that compromise of an account
does not confer the ability to power off the owner's machines. An attacker who obtains an account
session SHALL still be unable to command any device that was not already paired.

#### Scenario: An account session does not confer authority
- **WHEN** a device is signed in to the account that owns a target, but has not been paired with that target
- **THEN** the system SHALL refuse its commands for the pairing reason, exactly as though it were a stranger

#### Scenario: Pairing is specific to a pair of devices
- **WHEN** a device is paired with one target and sends a command to a different target of the same owner
- **THEN** the system SHALL refuse it, because authority is granted per pair rather than per account

### Requirement: A pairing grant is single-use
A grant offered to establish a pairing SHALL be usable at most once. Once a grant has been redeemed,
the system SHALL refuse any further attempt to redeem it, whether or not it has expired.

A grant that can be redeemed twice is a grant that can be redeemed by someone who observed it being
used. Single use is what makes an observed grant worthless after the fact.

#### Scenario: A grant cannot be redeemed twice
- **WHEN** a grant is redeemed successfully and the same grant is presented again
- **THEN** the system SHALL refuse the second attempt and SHALL report that the grant has already been used

#### Scenario: Redemption is refused distinctly from expiry
- **WHEN** an already-redeemed grant is presented before it would have expired
- **THEN** the system SHALL report that it was already used rather than reporting expiry, because the two indicate different situations to the user

### Requirement: A pairing grant expires, and its lifetime depends on how it was delivered
Every grant SHALL carry the instant it was issued and SHALL become invalid after a fixed lifetime.
The lifetime SHALL depend on how the grant was delivered to the person redeeming it:

- A grant presented **at the target device itself** SHALL have the longer lifetime. Possession of
  the machine is direct evidence of the authority being granted.
- A grant delivered **out of band to the account owner** SHALL have the shorter lifetime, because
  possession of a mailbox is weaker evidence than possession of the machine, and a shorter window
  reduces the period in which an intercepted grant is useful.

Both lifetimes SHALL be identical in every implementation, and the shorter SHALL be strictly
shorter than the longer.

#### Scenario: A grant within its lifetime is valid
- **WHEN** an unredeemed grant is presented before its lifetime has elapsed
- **THEN** the system SHALL accept it

#### Scenario: An expired grant is refused
- **WHEN** an unredeemed grant is presented after its lifetime has elapsed
- **THEN** the system SHALL refuse it and SHALL report expiry as the reason

#### Scenario: The out-of-band grant expires sooner
- **WHEN** a grant delivered out of band and a grant presented at the machine are both presented at an age between the two lifetimes
- **THEN** the system SHALL refuse the out-of-band grant and SHALL accept the one presented at the machine

#### Scenario: The expiry boundary is fixed, not implementation-defined
- **WHEN** a grant is presented at an age exactly equal to its lifetime
- **THEN** every implementation SHALL reach the same decision, and that decision SHALL be pinned by a shared vector case

### Requirement: An out-of-band grant is delivered only to the account owner
A grant delivered out of band SHALL be sent only to an address the account owner has already proven
control of. The system SHALL NOT accept a delivery address supplied as part of the request to pair,
because that would let a requester direct the grant to themselves.

#### Scenario: The delivery address is not caller-supplied
- **WHEN** a request to pair supplies its own delivery address
- **THEN** the system SHALL ignore the supplied address and SHALL use only the owner's established address

### Requirement: A pairing is revocable at the target, and revocation takes effect for later commands
The owner SHALL be able to revoke a pairing at the target device. After revocation, the system SHALL
refuse commands from the revoked device for the pairing reason.

Revocation SHALL be possible at the target without reference to any external service, so that a
device can be de-authorized even when no network or relay is reachable.

#### Scenario: A revoked device loses authority
- **WHEN** a pairing is revoked and a command subsequently arrives from that device
- **THEN** the system SHALL refuse it for the pairing reason

#### Scenario: Revocation does not require an intermediary
- **WHEN** a pairing is revoked at the target while no relay or network is reachable
- **THEN** the revocation SHALL take effect for subsequent commands regardless

### Requirement: Every grant decision names exactly one reason from a closed set
The decision on a presented grant SHALL be either validity or refusal with exactly one reason drawn
from a closed set. The reasons SHALL distinguish at minimum: the grant has expired; the grant has
already been used; and the grant does not match any grant the target issued.

Where more than one reason holds, the order of checking SHALL be fixed and identical in every
implementation.

#### Scenario: Both implementations decide every case identically
- **WHEN** the shared pairing-grant vectors are executed by the desktop and mobile implementations
- **THEN** both SHALL return the same decision and the same refusal reason for every case

#### Scenario: An unrecognised grant is distinguished from an expired one
- **WHEN** a grant is presented that the target never issued
- **THEN** the system SHALL report that it does not match, rather than reporting expiry


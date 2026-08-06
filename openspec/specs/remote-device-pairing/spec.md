# remote-device-pairing Specification

## Purpose
Defines what makes a pairing between two devices valid — how a pairing is granted, how long a grant
stays usable, and why pairing is the only act that confers authority to command another device.

Enforcement state. Grant *validity* is implemented and unchanged: expiry against each delivery
method's lifetime, single use, and recognition of a grant the target actually issued. That decision
exists in both Rust (`desktop/src-tauri/src/domain/pairing_grant.rs`) and Dart
(`mobile/lib/domain/pairing_grant.dart`) and is verified by shared cross-implementation vectors. A
durable pairing store exists on both platforms, recording each paired peer's verifying key, with
revocation that takes effect immediately for subsequent commands and survives a restart, and
re-pairing of a previously revoked device.

Now also implemented: the pairing *exchange* and the interface for it. A six-character code is drawn
from an alphabet excluding `0`/`O` and `1`/`I`/`l` (design D6, enforced at compile time so restoring
the confusable glyphs is a build error), presented at the target and typed at the requester. Both
halves exist in Rust (`desktop/src-tauri/src/application/pairing_flow.rs`) and Dart
(`mobile/lib/application/pairing_flow.dart`), reusing `evaluate_grant` rather than reimplementing
expiry or single use. Design D7's both-or-neither ordering is implemented and tested: the requester
records the issuer only on receipt of the issuer's confirmation, and withdraws the issuer's record if
its own write then fails. An end-to-end suite
(`desktop/src-tauri/tests/end_to_end_pairing_to_job.rs`) pairs two devices through that real code
path — not through hand-written pairing rows — and follows a signed command from the paired peer
through to a remote-origin job with the longer countdown, asserting on *both* stores at each step.
Surfaces exist on both platforms: a pairing screen, a paired-devices list, and revocation from the
device in front of the user.

**No pairing has ever occurred between two real devices.** Every test pairs two in-process stores
inside one binary. The exchange between separate machines — where the two halves are separated by a
network, the confirmation is a real round trip, and the compensating withdrawal can itself fail — has
never run. That last case is the one the D7 ordering exists for, and in the tests its `undo` closure
is a direct function call rather than the round trip it would really be.

**No human has driven either interface.** The desktop's `wiring.test.js` proves the controls exist,
carry labels, and are wired to the right commands; it does not prove that clicking one does the right
thing. Nobody has read a code off one screen and typed it into another.

Still not implemented: out-of-band delivery of a grant to the owner's established address, and the
prohibition on a caller-supplied delivery address. Those requirements constrain a future change
rather than describing behavior available today. The gating fact is worth keeping here so it is not
rediscovered: out-of-band delivery needs Cloud Functions, which the project's free Firebase tier does
not include. The at-machine path is unaffected and is the one that is built.

Presence in the paired-devices list is derived by the presence rule from each peer's last reported
instant, never taken as a flag from the relay. But the desktop installs no transport
(`setup::initialise_pairing` passes `None`), so **every peer currently reads Offline** and no real
reported instant has ever been observed. The derivation is verified; the reporting is not.
## Requirements
### Requirement: Pairing is the only act that confers authority to command a device
The system SHALL treat a device as authorized to command a target only if that specific device has
been paired with that specific target. No other fact SHALL be sufficient — in particular, being
signed in to the same account, being registered to the same owner, or being listed as belonging to
the same person SHALL NOT by itself authorize a device to command another.

Account access and device authority are deliberately separated so that compromise of an account
does not confer the ability to power off the owner's machines. An attacker who obtains an account
session SHALL still be unable to command any device that was not already paired.

A pairing SHALL be recorded durably at the target and SHALL record the peer's verifying key
alongside its identifier. The recorded key is what subsequent commands from that peer are checked
against; a pairing that stored only an identifier would authorize whoever presented that name rather
than the device the user actually paired with.

A pairing SHALL survive a restart of the application. Pairings that were forgotten on restart would
silently de-authorize every peer, which a user would experience as remote control failing for no
stated reason.

#### Scenario: An account session does not confer authority
- **WHEN** a device is signed in to the account that owns a target, but has not been paired with that target
- **THEN** the system SHALL refuse its commands for the pairing reason, exactly as though it were a stranger

#### Scenario: Pairing is specific to a pair of devices
- **WHEN** a device is paired with one target and sends a command to a different target of the same owner
- **THEN** the system SHALL refuse it, because authority is granted per pair rather than per account

#### Scenario: A pairing records the peer's key, not only its name
- **WHEN** a pairing is established
- **THEN** the target SHALL record the peer's verifying key, and SHALL check subsequent commands from that peer against the recorded key

#### Scenario: A command bearing the right name but the wrong key is refused
- **WHEN** a command names a paired peer but is signed with a key other than the one recorded for that pairing
- **THEN** the target SHALL refuse it on authenticity grounds

#### Scenario: Pairings survive a restart
- **WHEN** the application is restarted after a pairing is established
- **THEN** the pairing SHALL still be in effect and commands from that peer SHALL still be accepted

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

Revocation SHALL take effect immediately for every command evaluated after it, and SHALL persist
across restarts. A revocation that a restart undid would be worse than none, because the owner would
believe a lost device had been de-authorized when it had not.

The system SHALL permit a revoked device to be paired again. Revoking a pairing withdraws the
authority previously granted; it does not blacklist the device, and a user who revokes a phone after
mislaying it must be able to pair it again when it turns up.

#### Scenario: A revoked device loses authority
- **WHEN** a pairing is revoked and a command subsequently arrives from that device
- **THEN** the system SHALL refuse it for the pairing reason

#### Scenario: Revocation does not require an intermediary
- **WHEN** a pairing is revoked at the target while no relay or network is reachable
- **THEN** the revocation SHALL take effect for subsequent commands regardless

#### Scenario: Revocation survives a restart
- **WHEN** the application is restarted after a pairing is revoked
- **THEN** commands from the revoked device SHALL still be refused

#### Scenario: A revoked device can be paired again
- **WHEN** a device whose pairing was revoked is paired again
- **THEN** the new pairing SHALL take effect and the earlier revocation SHALL NOT continue to refuse its commands

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

### Requirement: Establishing a pairing requires both devices to be in use at the same time
The system SHALL establish a pairing only through an exchange in which the target device presents a
grant and the requesting device presents it back within the grant's lifetime. Both devices SHALL be
running and in the user's hands for the exchange to complete.

This is the moment the whole design rests on. Every later guarantee — that a relay cannot forge a
command, that an account session confers no authority, that a stolen phone can be de-authorized —
holds because authority was granted once, deliberately, by someone holding both devices. A pairing
that could be established remotely and unattended would return the system to trusting whoever
controls the account.

The exchange SHALL result in each device recording the other's identifier and verifying key. A
pairing SHALL NOT be recorded on one side only: a device that believes it is paired with a peer that
does not reciprocate would send commands that are always refused, with no indication why.

#### Scenario: A grant presented within its lifetime establishes the pairing
- **WHEN** a target presents a grant and the requesting device presents it back before it expires
- **THEN** both devices SHALL record the other's identifier and verifying key, and commands between them SHALL thereafter pass the pairing check

#### Scenario: An expired or already-used grant establishes nothing
- **WHEN** a grant is presented back after expiry, or after it has already been redeemed
- **THEN** no pairing SHALL be recorded, and the requesting device's commands SHALL continue to be refused for the pairing reason

#### Scenario: A pairing is recorded on both sides or neither
- **WHEN** a pairing exchange fails partway through
- **THEN** the system SHALL NOT leave one device believing it is paired while the other does not

### Requirement: The owner can see and revoke the pairings a device holds
The system SHALL present, at each device, the peers that device is paired with, and SHALL allow the
owner to revoke any of them from that device.

A pairing that cannot be seen cannot be audited, and one that cannot be revoked from the device in
front of you cannot be withdrawn when a peer is lost. Revocation is already required to work without
a network; the surface that triggers it SHALL be available on the same terms.

Each listed peer SHALL be shown with its presence derived from the peer's last reported instant by
the presence rule — never as a bare online/offline claim supplied by a relay.

#### Scenario: Paired peers are listed at the device
- **WHEN** the owner views the pairings at a device
- **THEN** each paired peer SHALL be listed with its identifier and its presence as derived from its last reported instant

#### Scenario: Revocation is available offline
- **WHEN** the owner revokes a pairing while no network is reachable
- **THEN** the revocation SHALL take effect for every command evaluated afterwards

#### Scenario: A revoked peer stops being listed as authorized
- **WHEN** a pairing is revoked
- **THEN** the peer SHALL no longer appear as an authorized peer, and its commands SHALL be refused for the pairing reason


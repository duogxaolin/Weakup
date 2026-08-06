## ADDED Requirements

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

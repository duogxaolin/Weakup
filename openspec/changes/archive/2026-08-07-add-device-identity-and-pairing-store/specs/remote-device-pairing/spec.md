## MODIFIED Requirements

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

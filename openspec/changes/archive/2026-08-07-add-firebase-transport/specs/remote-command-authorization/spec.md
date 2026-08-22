## MODIFIED Requirements

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

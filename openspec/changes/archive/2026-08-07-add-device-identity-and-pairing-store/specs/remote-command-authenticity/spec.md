## MODIFIED Requirements

### Requirement: A command is obeyed only when its authenticity is established at the target
The target device SHALL determine, before evaluating whether a command is permitted, that the
command was produced by a device paired with the target. That determination SHALL be made by
verifying a cryptographic signature over the command against a verifying key the target holds for
the claimed sender. It SHALL NOT depend on any assertion made by an intermediary that relays the
command, and it SHALL NOT be expressible as a value a caller can supply.

The verifying key used SHALL be the one recorded when the sender was paired with this target, read
from the target's own pairing store. It SHALL NOT be supplied by the caller alongside the command,
and a key belonging to a pairing that has been revoked SHALL NOT be used.

A caller that chooses which key a command is checked against decides whose signature counts, which
is the same authority as asserting authenticity outright. Reading the key from the store the user
populated by pairing is what makes the check answer to the user's decisions rather than to the
caller's.

A relay SHALL be able to prevent a command from arriving. It SHALL NOT be able to cause a command
to be obeyed that the sending device did not produce. Any design in which an intermediary's
cooperation is sufficient to have a command obeyed SHALL be treated as failing this requirement,
regardless of how trustworthy that intermediary is believed to be.

The target SHALL refuse a command for which it holds no verifying key under the claimed sender's
identity. An unknown sender SHALL be an authenticity failure rather than a lookup that falls back
to any more permissive answer.

#### Scenario: A command whose authenticity cannot be established is refused
- **WHEN** a command arrives whose origin the target cannot verify — whether because its signature does not verify against the key held for the claimed sender, or because the target holds no key for that sender at all
- **THEN** the target SHALL refuse it, SHALL report that its authenticity could not be established, and SHALL NOT evaluate whether the command would otherwise be permitted

#### Scenario: A command whose signature does not verify is refused
- **WHEN** a command arrives bearing a signature that does not verify against the key the target holds for the claimed sender
- **THEN** the target SHALL refuse it, SHALL report that its authenticity could not be established, and SHALL NOT evaluate whether the command would otherwise be permitted

#### Scenario: A command from an unknown sender is refused
- **WHEN** a command arrives naming a sender for which the target holds no verifying key
- **THEN** the target SHALL refuse it on authenticity grounds

#### Scenario: A command signed by the wrong key is refused
- **WHEN** a command is signed with a key other than the one the target holds for the claimed sender, and is otherwise fresh, unreplayed, and permitted
- **THEN** the target SHALL refuse it on authenticity grounds

#### Scenario: A correctly signed command passes authenticity
- **WHEN** a command arrives bearing a signature that verifies against the key the target holds for the claimed sender
- **THEN** the target SHALL treat its authenticity as established and SHALL proceed to the remaining checks

#### Scenario: An intermediary's assertion is not sufficient
- **WHEN** a command arrives accompanied by an intermediary's assertion that it is genuine, but its signature does not verify
- **THEN** the target SHALL refuse it exactly as though the assertion were absent

#### Scenario: Authenticity is settled before permission
- **WHEN** a command arrives that both fails authenticity and would also be refused for a permission reason
- **THEN** the target SHALL report the authenticity reason, so that a caller cannot learn the target's permission state by sending unauthenticated commands

#### Scenario: Altering a signed command invalidates it
- **WHEN** any part of a command's signed content is altered after signing — the command, the sender, the creation instant, or the value distinguishing it from other commands
- **THEN** the signature SHALL no longer verify and the target SHALL refuse the command on authenticity grounds

#### Scenario: The key comes from the pairing store, not from the caller
- **WHEN** a command is evaluated
- **THEN** the verifying key used SHALL be read from the target's pairing store for the claimed sender, and no caller-supplied key SHALL be consulted

#### Scenario: A revoked pairing's key is not used
- **WHEN** a command arrives from a device whose pairing has been revoked, bearing a signature that would verify against the key recorded before revocation
- **THEN** the target SHALL refuse it rather than verifying against the revoked pairing's key

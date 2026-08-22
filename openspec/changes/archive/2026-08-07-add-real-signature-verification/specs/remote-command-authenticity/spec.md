## MODIFIED Requirements

### Requirement: A command is obeyed only when its authenticity is established at the target
The target device SHALL determine, before evaluating whether a command is permitted, that the
command was produced by a device paired with the target. That determination SHALL be made by
verifying a cryptographic signature over the command against a verifying key the target holds for
the claimed sender. It SHALL NOT depend on any assertion made by an intermediary that relays the
command, and it SHALL NOT be expressible as a value a caller can supply.

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

## ADDED Requirements

### Requirement: The signed payload has one canonical encoding, identical in every implementation
The bytes a device signs SHALL be produced by a single canonical encoding of the command's signed
content, and that encoding SHALL be identical in every implementation. The encoding SHALL be fixed
by shared vectors rather than left to each implementation's serializer.

Two implementations that encode the same command differently produce different signatures over what
is logically the same message. Each would then reject the other's genuine commands while both pass
their own tests — a failure that is invisible until two devices of different platforms are paired,
which is the normal case rather than an edge one.

The encoding SHALL cover every field whose alteration must invalidate the signature: at minimum the
sending device, the command, the creation instant, and the value distinguishing the command from
others. A field outside the encoding is a field an intermediary can change without detection.

#### Scenario: Both implementations produce identical signing bytes
- **WHEN** the shared signing-payload vectors are encoded by the desktop and mobile implementations
- **THEN** both SHALL produce byte-identical output for every case

#### Scenario: A signature made by one implementation verifies in the other
- **WHEN** a command is signed by one implementation and verified by the other against the same key
- **THEN** verification SHALL succeed

#### Scenario: Every signed field is covered
- **WHEN** any single field of the signed content is changed and the payload re-encoded
- **THEN** the encoded bytes SHALL differ from the original

# remote-command-authenticity Specification

## Purpose
Decides whether an arriving remote command genuinely came from a device paired with this one, is
recent enough to act on, and has not been seen before. Authorization answers whether a device is
*permitted*; this answers whether the message is *real*, and it runs first.

Enforcement state, stated here because it is easy to misread from the requirements alone: the
acceptance rule is implemented in both Rust and Dart, verified by shared cross-implementation
vectors, and **verifies signatures for real**. The target checks an Ed25519 signature over a
canonically encoded payload against the verifying key it holds for the claimed sender. A caller
cannot assert authenticity; it can only present a signature and have it checked. A command whose
signature does not verify — or whose claimed sender the target holds no key for — is refused before
anything else is evaluated.

What is still absent is a transport: no relay yet delivers a command, so the scenarios describing
what happens to an *arriving* command are exercised end to end against an in-memory fake rather
than over a network. The fake is deliberately hostile — it can drop, delay, duplicate, and reorder
— and the rules are asserted to survive all four. What it cannot do is forge, and that is now a
property of arithmetic rather than of policy.

The keys these rules check against are no longer supplied by the caller. A verifying key comes from
the target's own pairing store, looked up by the claimed sender's identity, so a revoked pairing's
key is *absent* rather than present-and-rejected. On the sending side the device's private signing
key is generated once and held in the platform's secure store, and no code path can read it back —
signing is performed by the component that holds the key.

Limits that remain, and the changes they belong to: the account layer is still a fake — no OAuth, no
real identity behind `AuthProvider`. There is still no transport. And the platform secure stores
themselves are UNVERIFIED outside macOS; see the `device-identity` capability, whose Purpose states
the scope of that gap.

The structural prohibition on reaching the power-off executor is enforced by a source-text test
(`desktop/src-tauri/src/domain/command_acceptance_tests.rs`) that fails the build if command
acceptance ever references the power-off executor or the countdown gate. It is the half that must
not regress when the transport lands.
## Requirements
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

### Requirement: A command carries the instant it was created and is refused when stale
Every command SHALL carry the instant at which the sending device created it. The target SHALL
refuse a command whose stated creation instant is older than a fixed window, and the window SHALL
be identical in every implementation.

Without this, a command captured once remains obeyable forever. A power-off cannot be undone, so a
command that arrives long after it was issued SHALL be treated as an attack or an error rather than
as a delayed instruction, even when it is otherwise genuine and permitted.

#### Scenario: A command within the freshness window is accepted
- **WHEN** an otherwise valid command arrives whose creation instant is within the freshness window
- **THEN** the target SHALL accept it

#### Scenario: A stale command is refused
- **WHEN** an otherwise valid command arrives whose creation instant is older than the freshness window
- **THEN** the target SHALL refuse it and SHALL report staleness as the reason

#### Scenario: The freshness boundary is fixed, not implementation-defined
- **WHEN** a command arrives whose age is exactly the freshness window
- **THEN** every implementation SHALL reach the same decision, and that decision SHALL be pinned by a shared vector case rather than left to each implementation's comparison

### Requirement: A command dated in the future beyond a small tolerance is refused
The target SHALL refuse a command whose stated creation instant is further in the future than a
small fixed tolerance. The tolerance SHALL exist so that ordinary clock skew between two honest
devices does not refuse valid commands, and SHALL be small enough that it cannot be used to extend
the period during which a captured command stays obeyable.

#### Scenario: Mild clock skew is tolerated
- **WHEN** a command arrives whose creation instant is slightly in the future, within the tolerance
- **THEN** the target SHALL accept it rather than reporting a local failure

#### Scenario: A far-future command is refused
- **WHEN** a command arrives whose creation instant is further in the future than the tolerance
- **THEN** the target SHALL refuse it and SHALL report the reason distinctly from staleness, because the two indicate different faults

### Requirement: A command is obeyed at most once
Every command SHALL carry a value that distinguishes it from every other command from the same
device. The target SHALL refuse a command bearing a value it has already acted on, and SHALL
continue to refuse it for at least as long as the freshness window would otherwise allow it to be
accepted.

Freshness alone does not prevent replay: a command captured and resent immediately is still fresh.
Both rules are required, and neither substitutes for the other.

#### Scenario: A replayed command is refused
- **WHEN** a command arrives that is fresh, authentic, and permitted, but bears a value the target has already acted on
- **THEN** the target SHALL refuse it and SHALL report replay as the reason

#### Scenario: A first-time command is accepted and then no longer accepted
- **WHEN** a command is accepted, and an identical command bearing the same distinguishing value arrives afterwards
- **THEN** the target SHALL accept the first and refuse the second

#### Scenario: Distinct commands from the same device are each accepted
- **WHEN** two commands from the same paired device arrive bearing different distinguishing values, both fresh and permitted
- **THEN** the target SHALL accept both

### Requirement: Every refusal names exactly one reason from a closed set
The decision SHALL be either acceptance or refusal with exactly one reason drawn from a closed set,
so that a refusal can be explained in specific terms rather than as a generic failure. The reasons
SHALL distinguish at minimum: authenticity could not be established; the command is stale; the
command is dated too far in the future; the command has already been acted on; and the command is
not permitted.

Where more than one reason holds, the order in which reasons are checked SHALL be fixed and
identical in every implementation, so that two implementations never disagree about which reason a
given command earns.

#### Scenario: Both implementations decide every case identically
- **WHEN** the shared command-acceptance vectors are executed by the desktop and mobile implementations
- **THEN** both SHALL return the same decision and the same refusal reason for every case

#### Scenario: Precedence is pinned where several reasons hold at once
- **WHEN** a command arrives that is simultaneously unauthentic, stale, and replayed
- **THEN** both implementations SHALL report the same single reason, and that reason SHALL be fixed by a shared vector case

### Requirement: Acceptance of a power-off command still produces a job and never an execution
An accepted power-off command SHALL be carried out by creating a job on the target, which the
target's existing local scheduler then runs by its existing rules, including the mandatory
cancellable countdown for a remote-origin job.

Establishing that a command is genuine SHALL NOT shorten, skip, or bypass that countdown. A command
that is authentic, fresh, unreplayed, and permitted is the *normal* case, and the countdown exists
for exactly that case: the person at the machine did not ask for this and may not be watching when
it begins.

#### Scenario: A fully valid power-off command still yields a countdown
- **WHEN** a power-off command is accepted on every criterion
- **THEN** the target SHALL create a remote-origin `powerOff` job and the person at the machine SHALL retain the full remote countdown in which to cancel it

#### Scenario: No acceptance path reaches the power-off executor
- **WHEN** the source of the command-acceptance handling is inspected by the test suite
- **THEN** it SHALL contain no reference that would invoke the power-off executor or the countdown gate directly, and the test SHALL fail if such a reference is introduced

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


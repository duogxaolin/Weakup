# remote-command-authenticity Specification

## Purpose
Decides whether an arriving remote command genuinely came from a device paired with this one, is
recent enough to act on, and has not been seen before. Authorization answers whether a device is
*permitted*; this answers whether the message is *real*, and it runs first.

Enforcement state, stated here because it is easy to misread from the requirements alone: the
acceptance rule is implemented in both Rust and Dart and verified by shared cross-implementation
vectors, but no transport yet invokes it — nothing in this system accepts a remote command. The
scenarios below that describe what happens to an arriving command therefore constrain the change
that adds the transport, rather than describing behavior available today.

One limit is sharper than that and must not be read past. These rules do not perform cryptography
and do not verify signatures. The command envelope carries `signature_verified` as a boolean the
*caller supplies*; the rules decide what to do about a command that failed verification, not whether
it failed. Nothing in this change prevents a caller from passing `true` unconditionally, which would
defeat the entire model, and no test here can catch that because the caller does not exist yet.
Consequently the change that introduces the transport SHALL verify signatures against a key held
only by the paired devices, and SHALL carry an end-to-end test that a command bearing an invalid
signature is rejected. That is a blocker for the transport change, not a follow-up: until it exists,
the authenticity guarantee described below is specified but not enforced.

The one part enforced today is the structural prohibition on reaching the power-off executor. A
source-text test (`desktop/src-tauri/src/domain/command_acceptance_tests.rs`) fails the build if
command acceptance ever references the power-off executor or the countdown gate, and it is the half
that must not regress when the transport lands.
## Requirements
### Requirement: A command is obeyed only when its authenticity is established at the target
The target device SHALL determine, before evaluating whether a command is permitted, that the
command was produced by a device paired with the target. That determination SHALL depend only on
information the target itself holds or can verify, and SHALL NOT depend on any assertion made by
an intermediary that relays the command.

A relay SHALL be able to prevent a command from arriving. It SHALL NOT be able to cause a command
to be obeyed that the sending device did not produce. Any design in which an intermediary's
cooperation is sufficient to have a command obeyed SHALL be treated as failing this requirement,
regardless of how trustworthy that intermediary is believed to be.

#### Scenario: A command whose authenticity cannot be established is refused
- **WHEN** a command arrives whose origin cannot be verified against a pairing the target holds
- **THEN** the target SHALL refuse it, SHALL report that its authenticity could not be established, and SHALL NOT evaluate whether the command would otherwise be permitted

#### Scenario: An intermediary's assertion is not sufficient
- **WHEN** a command arrives accompanied by an intermediary's assertion that it is genuine, but the target cannot itself verify it against a pairing
- **THEN** the target SHALL refuse it exactly as though the assertion were absent

#### Scenario: Authenticity is settled before permission
- **WHEN** a command arrives that both fails authenticity and would also be refused for a permission reason
- **THEN** the target SHALL report the authenticity reason, so that a caller cannot learn the target's permission state by sending unauthenticated commands

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


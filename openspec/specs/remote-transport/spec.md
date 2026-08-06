# remote-transport Specification

## Purpose
Defines what carries commands between two paired devices — and, more importantly, what that carrier
is required to be incapable of. A transport moves opaque bytes and reports liveness; it is never the
reason a command is obeyed.

Enforcement state. A network implementation now exists in both languages —
`desktop/src-tauri/src/platform/firebase_transport.rs` over the Firestore REST API, and
`mobile/lib/platform/firebase_transport.dart` over `cloud_firestore` — replacing the previous state
in which only an in-memory fake existed. The structural guarantee is unchanged and is what the
requirements below actually rest on: the interface still has no method, parameter, or field by which
a transport could assert that a command is genuine, and a source-text test fails the build if one is
introduced in either the trait or the Firestore implementation. Signed content is carried as opaque
bytes; the transport does not parse, inspect, or validate it.

**No real Firestore round trip has ever occurred.** This is the central gap and it is not a small
one. Nothing in this repository has authenticated to Firestore or read a real document. The wire
types are hand-written per design D8 and exercised only against a stubbed HTTP layer, which proves
the parsing and the refusals — a fabricated command, an altered command, and malformed JSON are each
handled, the first two refused on authenticity grounds through `evaluate_command` and the third as a
named error rather than a panic — but a stub cannot prove the request shape matches what Google
actually serves. A Firestore API change, or a request this code builds incorrectly, would surface at
runtime rather than in any test here.

The desktop is additionally **not connected to its own transport**: `setup::initialise_pairing`
constructs the remote surface with `None` in the transport slot. Every peer therefore reads as
Offline in the desktop UI, and presence has never seen a real reported instant. What is verified is
the derivation, not the reporting: the 60-second interval is a named constant in each
implementation, tied by comment to the presence rule's 90-second online threshold, and the
online-or-not judgement is computed from the reported instant by that rule rather than taken as a
flag from the relay. That a device's last reported instant stops advancing when it stops running is
a property of nothing having run.

FCM is wired on mobile as a wake-up signal only, per design D4 — the push carries no command
content. **Delivery to a sleeping desktop is untestable here** and has not been observed. What *is*
tested is D4's degradation claim, which is the half that can be: with push disabled entirely, the
listener still delivers, so a push that never arrives costs latency rather than a missed command.

The relay's access rules are deployed (`firestore.rules`, released to `cloud.firestore`) and scope
both collections by owner. That scoping has never been the reason a command was refused or obeyed in
any test, which is the point — a target receiving a command from a relay whose rules failed entirely
still refuses it unless the signature verifies, and that is verified against the hostile fake.
## Requirements
### Requirement: A transport moves opaque bytes and makes no decision about them
A transport SHALL deliver a signed command from one device to another without interpreting its
contents, and SHALL NOT be able to influence whether the target obeys it. The interface a transport
implements SHALL offer no means of asserting that a command is authentic, permitted, fresh, or
otherwise acceptable.

This is what makes a hosted relay an acceptable place to run this service rather than a
concentration of risk. The guarantee is not that the operator is trustworthy; it is that the
operator's cooperation is not sufficient. A transport that could vouch for a command would make
compromise of the relay equivalent to control of every user's machine.

#### Scenario: The transport interface offers no authenticity assertion
- **WHEN** the transport interface is inspected
- **THEN** it SHALL expose no parameter, field, or method by which an implementation could report that a command is genuine, and a test SHALL fail if such a member is introduced

#### Scenario: A hostile transport cannot cause a command to be obeyed
- **WHEN** a transport delivers a command that the named sender did not produce
- **THEN** the target SHALL refuse it on authenticity grounds, and the outcome SHALL NOT depend on anything the transport did or claimed

### Requirement: A transport may fail, and the rules SHALL survive every way it can
A transport SHALL be assumed capable of dropping, delaying, duplicating, and reordering commands.
The system SHALL behave correctly under all four, and SHALL be tested against a transport that does
each of them deliberately rather than only against one that behaves.

These are not hypothetical faults. A dropped command is an ordinary mobile network; a delayed one is
a backgrounded app; a duplicate is a retry after an ambiguous timeout; a reorder is two commands
racing through different paths. A design tested only against a cooperative transport is a design
whose first real failure happens in production.

#### Scenario: A duplicated command is obeyed once
- **WHEN** a transport delivers the same signed command twice
- **THEN** the target SHALL obey it at most once, refusing the second delivery as already acted on

#### Scenario: A delayed command past the freshness window is refused
- **WHEN** a transport holds a command until it is older than the freshness window and then delivers it
- **THEN** the target SHALL refuse it as stale rather than obeying it late

#### Scenario: Reordered commands are each judged on their own merits
- **WHEN** a transport delivers two distinct valid commands in the opposite order from which they were sent
- **THEN** the target SHALL judge each independently and SHALL NOT refuse one merely for arriving out of order

#### Scenario: A dropped command has no effect
- **WHEN** a transport drops a command
- **THEN** the target SHALL be in the same state as if the command had never been sent

### Requirement: Liveness reporting is data, not a decision
A transport SHALL allow a device to report that it is present and to learn when the account's other
devices last reported. It SHALL NOT decide whether a device is online — that determination is made
from the reported instant by the presence rule, at whichever device is asking.

A transport that reported liveness as a verdict rather than as a timestamp would put a
network-dependent judgement in a place the user reads as fact, and two devices asking the same
transport could receive different answers about the same peer.

#### Scenario: The transport reports an instant, not a state
- **WHEN** a device asks the transport about the account's other devices
- **THEN** the transport SHALL report when each last reported presence, and any online-or-not judgement SHALL be derived from that instant by the presence rule

### Requirement: Account identity is obtained separately from command transport
The means of establishing which account a device belongs to SHALL be separate from the means of
carrying commands. Neither SHALL be able to substitute for the other.

Account identity determines which devices a user can *see*; pairing determines which they can
*command*. Keeping them separate in the code keeps them separate in fact — a single interface
handling both invites an implementation that treats a valid account session as sufficient authority,
which the authorization rules explicitly forbid.

#### Scenario: Account identity does not confer command authority
- **WHEN** a device holds a valid account identity for the account owning a target, but is not paired with it
- **THEN** its commands SHALL be refused for the pairing reason

#### Scenario: The two concerns are separately replaceable
- **WHEN** the account identity implementation is replaced
- **THEN** the transport implementation SHALL require no change, and the reverse SHALL also hold


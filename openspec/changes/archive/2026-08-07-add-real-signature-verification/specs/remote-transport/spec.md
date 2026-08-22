## Purpose

Defines what carries commands between two paired devices — and, more importantly, what that carrier
is required to be incapable of. A transport moves opaque bytes and reports liveness; it is never the
reason a command is obeyed.

Enforcement state: the interface and an in-memory fake exist in both Rust and Dart, and the fake is
what every end-to-end test runs against. No network implementation exists — there is no Firebase, no
HTTP, no WebSocket, no push. The requirements below therefore constrain the change that adds a real
transport as much as they describe today's fake. What is real today is the shape: the interface has
no method by which a transport could assert that a command is genuine, so an implementation cannot
acquire that power without changing the interface itself, which is a visible act rather than an
oversight.

## ADDED Requirements

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

## MODIFIED Requirements

### Requirement: A transport moves opaque bytes and makes no decision about them
A transport SHALL deliver a signed command from one device to another without interpreting its
contents, and SHALL NOT be able to influence whether the target obeys it. The interface a transport
implements SHALL offer no means of asserting that a command is authentic, permitted, fresh, or
otherwise acceptable.

This is what makes a hosted relay an acceptable place to run this service rather than a
concentration of risk. The guarantee is not that the operator is trustworthy; it is that the
operator's cooperation is not sufficient. A transport that could vouch for a command would make
compromise of the relay equivalent to control of every user's machine.

A hosted implementation SHALL store the signed command as opaque content and SHALL NOT require the
relay to read, parse, or validate it in order to deliver it. The relay's access rules SHALL scope
data to its owner, and that scoping SHALL NOT be the reason a command is refused or obeyed — a
target that receives a command from a relay whose rules failed entirely SHALL still refuse it
unless its signature verifies.

#### Scenario: The transport interface offers no authenticity assertion
- **WHEN** the transport interface is inspected
- **THEN** it SHALL expose no parameter, field, or method by which an implementation could report that a command is genuine, and a test SHALL fail if such a member is introduced

#### Scenario: A hostile transport cannot cause a command to be obeyed
- **WHEN** a transport delivers a command that the named sender did not produce
- **THEN** the target SHALL refuse it on authenticity grounds, and the outcome SHALL NOT depend on anything the transport did or claimed

#### Scenario: A relay that returns arbitrary content changes no outcome
- **WHEN** a relay returns a command whose signed content it altered, or one it fabricated entirely
- **THEN** the target SHALL refuse it on authenticity grounds

### Requirement: Liveness reporting is data, not a decision
A transport SHALL allow a device to report that it is present and to learn when the account's other
devices last reported. It SHALL NOT decide whether a device is online — that determination is made
from the reported instant by the presence rule, at whichever device is asking.

A transport that reported liveness as a verdict rather than as a timestamp would put a
network-dependent judgement in a place the user reads as fact, and two devices asking the same
transport could receive different answers about the same peer.

A device SHALL report presence on a fixed interval while it is running, and SHALL NOT report on
behalf of any device other than itself. The interval SHALL be short enough that the presence rule's
own thresholds remain meaningful, and SHALL NOT be so short that an idle account consumes a
disproportionate share of the relay's quota.

#### Scenario: The transport reports an instant, not a state
- **WHEN** a device asks the transport about the account's other devices
- **THEN** the transport SHALL report when each last reported presence, and any online-or-not judgement SHALL be derived from that instant by the presence rule

#### Scenario: A device reports only its own presence
- **WHEN** a device reports presence
- **THEN** it SHALL update only its own record, and SHALL NOT be able to make another device appear present

#### Scenario: Presence reporting stops when the device stops
- **WHEN** a device is no longer running
- **THEN** its last reported instant SHALL cease advancing, and peers SHALL observe it ageing into staleness and then absence by the presence rule's thresholds

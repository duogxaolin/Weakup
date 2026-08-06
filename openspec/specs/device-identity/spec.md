# device-identity Specification

## Purpose
How a device obtains and keeps a signing identity of its own — one keypair, generated once, whose
private half lives in the platform's secure store and can be used but never read back. This is what
turns a verified signature from "someone holds this key" into "this device sent this command".

Enforcement state, stated here because it is the first place in this system where a passing test
suite stops meaning the feature works. Every *decision* below — generate once, reuse the existing
identity, report an error rather than regenerate, sign but never read back — is implemented in both
Rust and Dart and tested against an in-memory substitute for the secure store. The platform-backed
stores behind that substitute (`keyring` on Windows, macOS, and Linux; `flutter_secure_storage` on
Android and iOS) are **UNVERIFIED on Windows, Linux, Android, and iOS**: they are written against
each library's documented behaviour and exercised only on macOS. What is untested is therefore the
storage *mechanism*, not the logic around it — a device that can read and write its secure store at
all will behave exactly as these requirements describe. Closing this gap requires running on each
target; no additional testing on a developer machine can close it.

Two prohibitions are enforced today rather than left to review, both by source-text tests in
`desktop/src-tauri/src/domain/device_identity_tests.rs`: one fails the build if a member returning
private key material is introduced on the identity component, and one fails the build if the
hand-written `Debug` implementation is replaced by a derive that would print the key.
## Requirements
### Requirement: A device has exactly one signing identity, generated once
The system SHALL generate a signing keypair the first time a device runs, and SHALL reuse that same
identity for the lifetime of the installation. It SHALL NOT generate a second identity while the
first remains usable, and SHALL NOT silently replace an existing identity.

A device whose identity changed would appear to every peer as a different device: its commands would
be refused as coming from an unknown sender, and every pairing it holds would have to be redone. So
identity generation SHALL happen once and SHALL be recoverable across restarts rather than being
regenerated whenever the app cannot immediately find it.

If an identity is expected but cannot be retrieved, the system SHALL report the failure rather than
generating a replacement. Quietly minting a new identity would convert a recoverable storage problem
into a permanent loss of every pairing.

#### Scenario: The first run generates an identity
- **WHEN** a device runs for the first time and holds no identity
- **THEN** the system SHALL generate a keypair and SHALL retain it for subsequent runs

#### Scenario: A later run reuses the same identity
- **WHEN** a device that already holds an identity runs again
- **THEN** the system SHALL use the existing identity and SHALL NOT generate a new one

#### Scenario: An unreadable identity is an error, not a reason to regenerate
- **WHEN** a device holds a record that it has an identity but the private key cannot be retrieved
- **THEN** the system SHALL report the failure and SHALL NOT generate a replacement identity

### Requirement: The private key is written to the platform's secure store, never to ordinary storage
The system SHALL store the private half of the identity using the operating system's protected
credential facility. It SHALL NOT write the private key to the application database, to a
configuration file, to a log, or to any location readable without the protection the platform
provides.

The public half and the device's own identifier are not secret and SHALL be stored in ordinary
application storage, so that a device can report who it is without unlocking anything.

#### Scenario: The private key is absent from application storage
- **WHEN** the application's database and configuration files are inspected
- **THEN** they SHALL contain no private key material

#### Scenario: The public identity is available without the secure store
- **WHEN** the device reports its own identifier and verifying key
- **THEN** it SHALL do so from ordinary storage without requiring access to the private key

### Requirement: The private key can be used for signing but never read back
The system SHALL expose no operation that returns the private key to the rest of the application.
Signing SHALL be performed by the component that holds the key, which SHALL accept a payload and
return a signature.

An interface that can return the key is one from which the key will eventually be returned — into a
log line, an error message, a crash report, or a debugging aid added under time pressure. Making the
key unreachable by construction removes that class of accident rather than relying on every future
contributor to avoid it.

#### Scenario: No operation returns private key material
- **WHEN** the identity component's interface is inspected by the test suite
- **THEN** it SHALL expose no member that returns the private key, and the test SHALL fail if such a member is introduced

#### Scenario: Signing produces a signature the peer can verify
- **WHEN** the identity component signs a canonical payload
- **THEN** the resulting signature SHALL verify against the device's published verifying key

### Requirement: A device's identifier is derived from its verifying key
The system SHALL derive a device's identifier from its verifying key rather than assigning one
independently. Two devices SHALL NOT be able to claim the same identifier while holding different
keys.

An independently assigned identifier can be claimed by any device that learns it, which would let an
attacker present a key of their own under a paired device's name. Deriving the identifier from the
key makes the claim self-certifying: to use an identifier, a device must hold the key it was derived
from.

#### Scenario: The identifier follows from the key
- **WHEN** a device's identifier is computed from its verifying key
- **THEN** the same key SHALL always produce the same identifier, and both implementations SHALL produce identical identifiers for the same key

#### Scenario: A different key yields a different identifier
- **WHEN** two distinct verifying keys are used to derive identifiers
- **THEN** the resulting identifiers SHALL differ

### Requirement: The secure store is reachable behind an interface with a testable substitute
The system SHALL access the platform's secure store through an interface, and SHALL provide a
non-persistent substitute for use in tests.

The platform implementations cannot be exercised on every target from a developer machine, so
every decision that depends on the store SHALL be testable without it. What the substitute cannot
establish is whether the real store behaves as expected on real hardware; that SHALL be stated
rather than left for a reader to assume.

#### Scenario: Identity behavior is testable without the platform store
- **WHEN** the test suite exercises identity generation, reuse, and signing
- **THEN** it SHALL do so against the substitute, without requiring a platform credential store

#### Scenario: The interface does not leak the storage mechanism
- **WHEN** application code obtains or uses the device identity
- **THEN** it SHALL do so without depending on which platform store is in use


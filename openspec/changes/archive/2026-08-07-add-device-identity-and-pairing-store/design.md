## Context

See proposal.md — Why. Signature verification is real but nothing supplies the keys:
`evaluate_command` takes a `HashMap<DeviceId, VerifyingKey>` that only tests construct. The
capability Purpose says so plainly, and this change is what makes it obsolete.

Three constraints shape the design.

**Purity is the reason the vectors work.** `evaluate_command` takes `now` and reads no clock;
`authorize` takes named booleans and performs no lookup. Key storage is the opposite of pure — it is
platform I/O with failure modes that differ per OS. So the seam between them has to be drawn
carefully, or the rules stop being vector-testable. D1 is about exactly that line.

**The safety structure stays untouched.** `PowerOffGate` holds the only `PowerOffExecutor`.
Source-text tests fail the build if a remote module references it. Nothing here goes near that path.

**This is the first change that cannot be fully verified on this machine.** Every prior change was
provable by running the suites. Keychain, Keystore, Credential Manager, and Secret Service behave
differently on real hardware, and three of the four cannot be exercised from a macOS developer
machine at all. D7 records how the design limits the damage that fact can do, and the capability
Purpose states the gap rather than leaving a reader to assume otherwise.

## Goals / Non-Goals

**Goals:**

- A device holds one long-lived identity whose private half is in the platform's secure store and
  cannot be read back by application code.
- Pairings are durable, carry the peer's key, and can be revoked with immediate effect that survives
  a restart.
- `evaluate_command` reads keys from the store rather than from an argument, closing the last route
  by which a caller influences whose signature counts.
- Every decision remains vector-testable without a platform credential store.

**Non-Goals:**

- No transport, no Firebase, no OAuth. The scaffolded Firebase config stays unread.
- No pairing UI and no mail delivery. This change gives pairing somewhere to write; the flows that
  call it come later.
- No key rotation. A device has one identity for the life of the installation, and rotating it is a
  separate problem with its own migration story — see D8.

## Decisions

### D1: The store is an interface; the rules stay pure and keep taking a key map

`evaluate_command` continues to accept `verifying_keys` as data. What changes is that production
code builds that map from the pairing store immediately before the call, rather than a caller
inventing one.

The tempting alternative — passing the store into the rule and letting it look keys up — would make
the rule impure, and every vector case would then need a store fixture. The vectors are the only
thing preventing the two implementations from drifting, so anything that makes them harder to write
is a cost paid on every future change.

The spec requirement that "no caller-supplied key SHALL be consulted" is therefore satisfied
structurally at the *composition* layer rather than inside the rule: exactly one production function
assembles the map, and it reads from the store. A source-text test asserts no other production path
constructs a `verifying_keys` map. This is weaker than making it impossible in the type system, and
the weakness is recorded rather than glossed: it is the same trade the repo already accepts for its
other source-text guards.

### D2: The identity component signs; it never returns the key

`DeviceIdentity` exposes `device_id()`, `verifying_key()`, and `sign(payload) -> Signature`. There is
no `private_key()`, no `export()`, no `Debug` that could render it, and no `Serialize`.

This mirrors `PowerOffGate`'s argument and the transport interface's: a capability that exists gets
used. A key that can be read will eventually be read into a log line, a crash report, or a debugging
aid added at 2am. Making it unreachable removes the class of accident rather than relying on
discipline.

`Debug` is implemented by hand to print the device id and the word `<sealed>` rather than being
derived, because a derived `Debug` on a struct holding key bytes is precisely the accident this
guards against.

### D3: The device id is derived from the verifying key

`DeviceId` = the first 16 bytes of SHA-256 over the 32 verifying-key bytes, hex-encoded.

An independently assigned identifier can be claimed by anyone who learns it: an attacker presents
their own key under a paired device's name, and the target — which looks keys up *by* name — would
have to already know better. Deriving the id from the key makes the claim self-certifying.

Truncated to 16 bytes because the full 32 produces a 64-character string that will end up in a UI,
and 128 bits is far beyond what a collision attack on a personal device set requires. SHA-256 rather
than the raw key so the id is a stable length regardless of any future key type.

Both implementations must agree byte for byte, so this gets vector cases rather than parallel unit
tests.

### D4: Revocation is a flag on the pairing, not a deletion

The `pairings` row carries `revoked_at`, nullable. Revoking sets it; it does not delete the row.

Deleting would lose the record that a pairing ever existed, which is exactly what someone
investigating an unexplained shutdown wants to see. It would also make "was this device ever
paired?" unanswerable, and that question matters after a device is lost.

Re-pairing a revoked device clears `revoked_at` on the same row rather than inserting a second.
Two rows for one peer would make "is this device authorized?" depend on which row is read first,
which is the kind of ambiguity that eventually resolves the wrong way. The spec requires re-pairing
to work: revocation withdraws authority, it does not blacklist.

### D5: The key map excludes revoked pairings at construction

The function that builds `verifying_keys` from the store filters out revoked rows, so a revoked
peer's key is simply absent and the command fails as an unknown sender.

The alternative — including the key and rejecting later on a pairing check — would verify the
signature of a device the user has explicitly de-authorized, then refuse it for a different reason.
That reports the wrong thing to the user and does cryptographic work on behalf of a revoked device.
Absent is the honest representation of revoked.

### D6: Both schemas gain two tables, and the identity table stores no secret

`pairings` (peer id, verifying key, paired_at, revoked_at) and `device_identity` (device id,
verifying key, created_at). Rust `SCHEMA_VERSION` 4 → 5; Dart `schemaVersion` 3 → 4.

`device_identity` holds only public material. Its purpose is to answer "who am I" without touching
the secure store, and — more importantly — to record that an identity *exists*. That record is what
makes D-below's "unreadable identity is an error" rule possible: without it, a failed secure-store
read is indistinguishable from a first run, and the system would regenerate, silently destroying
every pairing.

Two additive tables, no existing row rewritten. Both migrations are exercised by a test that opens a
pre-migration database.

### D7: The secure store sits behind a trait with an in-memory fake, and the platform gap is named

`SecretStore` with `keyring`-backed (Rust) and `flutter_secure_storage`-backed (Dart)
implementations, plus `InMemorySecretStore` for tests.

Every decision — generate-once, reuse, error-rather-than-regenerate, sign-and-never-read — is
testable against the fake. What the fake cannot establish is whether the real store works on real
hardware, and three of the four platforms cannot be exercised from this machine.

Stated plainly, because this is the first change in this sequence where "tests pass" stops meaning
"it works": **the platform-backed implementations are UNVERIFIED on Windows, Linux, Android, and
iOS.** They are written against each library's documented behaviour and exercised only on macOS.
This is recorded in the capability Purpose so the archived spec does not read as a claim of
cross-platform correctness, and closing it requires running on each target rather than more tests
here.

### D8: No key rotation, and the reason it is deferred rather than forgotten

A device has one identity for the life of the installation.

Rotation sounds like a small addition and is not: every peer holding the old key must learn the new
one, which requires either re-pairing every peer by hand or a key-distribution mechanism with its own
authenticity problem — a new key announced over the relay is exactly the forgery the whole design
exists to prevent. Solving that needs the transport, and solving it badly would undo the guarantee.

Consequence, stated rather than implied: a device whose private key is compromised cannot be
recovered by rotating. The remedy is to revoke its pairings at every peer, which is a local act the
spec already requires and this change implements. That is adequate for a personal device set and
would not be for a fleet.

## Risks / Trade-offs

- **The platform secure stores are unverified on four of five targets** → See D7. The largest gap in
  this change, named in the capability Purpose rather than left implicit. Every decision is testable
  against the fake; only the storage mechanism is not, and no amount of testing on this machine can
  close it.

- **"No caller-supplied key" is enforced at the composition layer, not the type layer** → See D1.
  Structurally weaker than making it unrepresentable, accepted because the alternative makes the
  rules impure and the vectors much harder to write. Guarded by a source-text test in the style the
  repo already uses.

- **`rand_core` is added to `ed25519-dalek`, which the previous change deliberately excluded** →
  That exclusion was correct for a change that only verified. Generation genuinely needs entropy, and
  this is the change that generates. The feature is scoped to the identity module; no other code path
  generates keys.

- **A lost secure-store entry loses every pairing** → By design, per the generate-once requirement:
  the alternative is regenerating, which loses every pairing *silently* and additionally makes the
  device look like a stranger to peers that still trust the old key. Failing loudly is the better of
  two bad outcomes. The recovery path is re-pairing, which the user can do.

- **A compromised private key cannot be rotated away** → See D8. Mitigated by revocation at each
  peer, which is local, immediate, and survives restart. Adequate for a personal device set; would
  not be for a fleet.

- **Two schema migrations in one change** → Both additive table creations with no existing row
  rewritten, each exercised against a pre-migration database. Splitting them would leave identity
  without a place to record that it exists, which is what makes the regenerate-on-failure hazard
  avoidable.

## Migration Plan

Two additive migrations:

- Rust: `SCHEMA_VERSION` 4 → 5, `CREATE TABLE IF NOT EXISTS pairings` and `device_identity`.
- Dart: `schemaVersion` 3 → 4, matching steps appended to the existing `MigrationStrategy.onUpgrade`
  — appended, not replacing it, or the `trigger_date` and `origin` migrations are lost.

No existing table is altered and no row is rewritten, so an older binary reading a migrated database
sees tables it does not query. Rollback is dropping the two tables; the secure-store entry would
remain and be reused if the change is reapplied, which is harmless because the identity is stable by
design.

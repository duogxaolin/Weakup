## Context

See proposal.md — Why. This change exists to pay one debt the previous change recorded as a blocker:
`signature_verified` is a boolean the caller supplies, so the authenticity guarantee is specified but
not enforced.

Three constraints shape the design.

**The vectors are the contract.** `shared/testvectors/README.md` states it: any rule both
implementations must share needs a vector case, or it is not cross-implementation-verified. This
change adds a new kind of vector — one that pins *bytes* rather than a decision — because a
signature is only interoperable if both sides encode the signed content identically.

**The safety structure is load-bearing and stays untouched.** `PowerOffGate` holds the only
`PowerOffExecutor` and takes no duration argument. Source-text tests fail the build if
`remote_command.rs` or `command_acceptance.rs` references it. A remote request can only create a
job; the scheduler and its countdown remain the only path to a shutdown.

**Everything decided here must be pure.** `evaluate_command` takes `now` as a parameter and reads no
clock. Verification is arithmetic over bytes, so it fits this without exception — which is precisely
why it can land before any transport exists.

## Goals / Non-Goals

**Goals:**

- `signature_verified` ceases to exist as an input. Authenticity becomes something the target
  computes, not something a caller asserts.
- The signed encoding is byte-identical across languages, pinned by vectors, so a signature made on
  a phone verifies on a desktop.
- A transport seam exists with a deliberately hostile fake behind it, so the rules are exercised
  end to end without a network.
- An invalid signature is rejected end to end, in both languages — the specific test the blocker
  named.

**Non-Goals:**

- No key generation and no secret storage. See D6 — this is the largest remaining gap and it is
  named rather than quietly deferred.
- No Firebase, HTTP, WebSocket, or push. `FakeTransport` is the only implementation.
- No OAuth. `AuthProvider` has a fake behind it.
- No decision about how a public key reaches a target. The rules take a key; pairing delivers one,
  and pairing storage does not exist yet.

## Decisions

### D1: The envelope carries a signature, not a verdict

`CommandEnvelope.signature_verified: bool` is **removed** and replaced by `signature: Vec<u8>`.

Making this a removal rather than an addition is the entire point. If both fields existed, every
call site would face a choice between doing the work and asserting the answer, and some call site
would eventually assert. Deleting the field makes the shortcut unrepresentable — the same argument
D3 of the previous change made for omitting a `relay_attested` field, and the same one
`PowerOffGate` makes by taking no duration parameter.

This is a breaking change to the envelope and to the existing `command_acceptance.json` vectors,
which assert on a field that no longer exists. Those cases are rewritten rather than extended. That
is the one place this change edits existing vectors, and it is unavoidable: a vector asserting on a
removed field cannot be preserved, only translated.

### D2: Verification happens inside `evaluate_command`, against a key in target state

`CommandTargetState` gains `verifying_keys: Map<DeviceId, VerifyingKey>`. `evaluate_command` performs
the verification itself as the first check.

The alternative — a separate `verify_signature` the caller runs first, passing the result in — is
what we have today under a different name. It leaves the composition to each call site, and a call
site that forgets produces a working system with a silent hole. Composing inside the single
vector-covered function is what makes the ordering and the completeness testable, which is the same
argument D1 of the previous change made for composing acceptance rather than exposing three checks.

An unknown sender — no key under that `DeviceId` — is `AuthenticityUnverified`, not a distinct
reason. Distinguishing "unknown device" from "bad signature" would tell an attacker which device
identifiers exist, which is exactly the probing the D5 precedence order exists to prevent.

### D3: The signed payload is a length-prefixed concatenation, not JSON

Fields are encoded in a fixed order, each prefixed by its byte length:
`sender || command || created_at_millis || nonce`.

JSON is the obvious choice and the wrong one. Key order, whitespace, unicode escaping, and integer
formatting are all unspecified enough that two languages' serializers differ — and every difference
produces a valid-looking signature that the other side rejects. Canonical JSON exists but is a
larger contract to get right in two languages than the problem needs.

Length-prefixing rather than a delimiter, because a delimiter is forgeable: with `a|b` as the
encoding, a sender named `x|y` and a command `z` produce the same bytes as a sender `x` and command
`y|z`. Length prefixes make each field's boundary explicit, so no two distinct field sets encode
identically. This is a real attack on naive concatenation, not a theoretical tidiness concern.

`created_at` is encoded as milliseconds since the Unix epoch, not as a formatted string. A string
encoding would put timezone rendering and fractional-second precision inside the signature, where
two implementations reliably disagree.

### D4: A new vector file pins bytes, not decisions

`shared/testvectors/signing_payload.json`, whose cases carry the field values and the expected
encoded bytes as lowercase hex.

Every other vector file in this repo pins a *decision*. This one pins an *encoding*, because a
decision-level test cannot catch the failure mode that matters: both implementations can agree on
"this signature is invalid" while disagreeing on what bytes a valid one covers. That disagreement
surfaces only when a phone and a desktop are paired, which is the normal case.

The file includes cases with multi-byte UTF-8 in the sender identifier and with a nonce containing
the delimiter characters a naive encoder might have chosen, so the length-prefix property from D3 is
exercised rather than assumed.

### D5: `FakeTransport` is adversarial by default, not merely simple

It can drop, delay, duplicate, and reorder on demand, and the end-to-end tests use all four.

A fake that only behaves proves the happy path and nothing else. The spec requires the rules survive
a transport that misbehaves, and the cheapest place to establish that is a fake that misbehaves on
command — long before a real network does it unpredictably. The duplicate case in particular is what
exercises replay rejection through the full path rather than by calling the nonce check directly.

### D6: Keys are supplied, not generated or stored — and this is the remaining gap

The rules take a `VerifyingKey`. Nothing here generates a keypair, writes a private key to Keychain,
Keystore, or Credential Manager, or reads one back.

This is deliberate and it is the largest thing this change does not do. Key storage has no pure
core: it is three platform APIs with different failure modes, verifiable only on a real device, and
mixing it into a change about arithmetic would mean neither part could be tested cleanly.

Trade-off, stated plainly: after this change a signature is genuinely verified, but **nothing yet
guarantees the signing key is held only by the paired devices**. A key in a plain file would verify
exactly as well as one in a Keychain. So the guarantee delivered here is "this command came from
whoever holds that key", not yet "this command came from that device". Closing the gap is a blocker
for the change that ships to a real device, not a refinement, and it is recorded in the capability's
Purpose so the archived spec says so.

What *is* closed: a relay cannot forge a command. That was the property at risk, and it no longer
depends on anyone's good behaviour.

### D7: `RemoteTransport` and `AuthProvider` are separate interfaces

Two interfaces, not one with both concerns.

Account identity decides which devices a user can *see*; pairing decides which they can *command*.
The authorization rules already state that same-account access is not sufficient. A single interface
carrying both concerns invites an implementation where a valid session is treated as authority —
the exact failure the rules forbid — and makes it awkward to replace the auth provider without
touching the transport, which is the migration path this design is meant to keep open.

### D8: The transport interface's incapability is enforced by test, in the existing style

A source-text test asserts the transport trait exposes no member that could carry an authenticity
assertion — no `verified`, `trusted`, `attested`, or `authentic` in its method or field names.

Chosen over relying on review because the failure is additive: someone adds a helpful-looking
`is_verified` field, every existing test still passes, and the guarantee is gone. Source text is
blunt, but it is the mechanism already in use here for exactly this class of rule, and consistency
matters more than elegance for a check whose whole job is to be hard to remove by accident.

The test lives in a separate file rather than an inline `mod tests`, because an inline module's own
string literals would self-match its own grep — a trap this codebase has already hit once.

## Risks / Trade-offs

- **A verified signature does not yet prove the key is device-held** → See D6. The largest remaining
  gap, named as a blocker for the change that ships to a device rather than left implicit. What is
  fixed today is relay forgery, which was the property the architecture rests on.

- **Two new cryptographic dependencies** (`ed25519-dalek` 3.0, `cryptography` 2.9) → Both are the
  standard, widely-audited choice in their ecosystem, and both are used only for verification, which
  is the smaller and safer half of the API. No custom crypto is written. The alternative — no
  verification — is strictly worse.

- **Existing `command_acceptance.json` cases must be rewritten** → Unavoidable per D1: they assert on
  a field being removed. Mitigated by the vectors being rewritten mechanically with signatures
  produced by the real implementation, and by both suites failing red before the rewrite lands.

- **A canonical encoding is a permanent commitment** → Changing it later invalidates every signature
  in flight and requires both sides to upgrade together. Accepted because the alternative is
  deferring it, and a wrong encoding baked into a wire format is worse than a fixed one chosen now.
  The vectors make an accidental change a test failure rather than a field incident.

- **`FakeTransport` may diverge from how a real relay behaves** → True, and the reason its faults are
  modelled explicitly rather than left to chance. It cannot anticipate every real failure, but a
  transport that drops, delays, duplicates, and reorders covers the four faults the design actually
  depends on surviving.

## Migration Plan

None at the storage layer — no schema change, no migration, no wire format persisted anywhere. The
envelope shape changes, but no envelope has ever been written to disk or sent over a network, so
there is nothing in existence to migrate.

Rollback is reverting the two dependencies and restoring the boolean field, which would reopen the
gap this change closes.

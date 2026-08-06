# Verify signatures for real, and put a transport seam behind the rules

## Why

The previous change specified authenticity and then declined to enforce it. Its own design says
so plainly (D2): the acceptance rule takes `signature_verified` as a **boolean the caller supplies**
and performs no cryptography. It decides what to do about a command that failed verification; it
does not decide whether it failed.

That was the right call for a change with no keys and no transport. It is the wrong state to build
a transport on. The first thing a relay does is hand a command to `evaluate_command`, and today
nothing can compute that boolean — so a transport written now would have to pass `true`
unconditionally, which defeats the entire model. The archived capability Purpose records this as a
blocker for the transport change rather than a follow-up. This change pays it.

There is a second reason to do this before any network code: verification and networking fail in
different ways, and debugging them together is much harder than debugging them apart. A signature
bug looks exactly like a transport bug from the outside. Landing verification against an in-memory
fake first means that when the real relay misbehaves later, the signature path is already known
good.

## What Changes

**Signature verification becomes real, and `signature_verified` stops being an input.**

- Ed25519 verification in both languages — `ed25519-dalek` 3.0 (Rust) and `cryptography` 2.9 (Dart).
  **BREAKING** for `CommandEnvelope`: the `signature_verified: bool` field is replaced by a
  `signature: Vec<u8>` / `List<int>` carrying the actual signature bytes. A caller can no longer
  assert authenticity; it can only present evidence and have it checked.
- A canonical byte encoding for the signed payload, fixed by shared vectors. Two implementations
  that serialize the same command differently produce different signatures and would reject each
  other's valid commands — so the encoding is part of the contract, not an implementation detail.
- `evaluate_command` gains the verifying key of the claimed sender as part of target state and
  performs the verification itself. An unknown sender fails authenticity rather than being looked
  up somewhere trusting.

**A transport seam, with a fake behind it.**

- `RemoteTransport` — an interface for registering a device, reporting liveness, listing the
  account's devices, sending a signed envelope, and receiving envelopes addressed to this device.
  It moves opaque bytes and makes no decisions.
- `AuthProvider` — an interface for obtaining the account identity a transport needs for routing.
  Deliberately separate from `RemoteTransport` so that the authentication provider can be changed
  without touching the transport, and so neither can quietly become the thing that authorises a
  command.
- `FakeTransport` — an in-memory implementation used by tests. It is also the honest expression of
  a property the design claims: a relay that can drop, delay, duplicate, and reorder commands but
  cannot forge one. The fake can do all four, and the tests assert the rules survive it.

**The end-to-end test the blocker named.**

- A test in each language that signs a command with one key, presents it with a different key, and
  asserts rejection with `AuthenticityUnverified` — travelling the full path from envelope through
  transport through acceptance, not calling the verifier directly.

Deliberately **not** in this change:

- No Firebase, no HTTP, no WebSocket, no FCM. `FakeTransport` is the only implementation.
- No key *generation* and no secret storage. Keychain, Keystore, and Credential Manager are
  platform work with no pure core, and they belong with the change that has a device to run on.
  This change verifies signatures against a key it is given; it does not create or persist one.
- No OAuth and no Google sign-in. `AuthProvider` is an interface with a fake behind it.
- No pairing UI, no pairing storage, no mail delivery — unchanged from the previous change.
- No new Tauri command and no change to `capabilities/default.json`.

## Capabilities

### New Capabilities
- `remote-transport`: What a transport is required to do and — more importantly — what it is
  required to be incapable of. Moving bytes, reporting liveness, and never being the reason a
  command is obeyed.

### Modified Capabilities
- `remote-command-authenticity`: Authenticity stops being an asserted input and becomes a verified
  fact. The requirement that a target verify against a key only the paired devices hold moves from
  a constraint on a future change to a rule this change enforces, and the capability's Purpose is
  rewritten to say what is now true.

## Impact

- **Shared contract**: new `shared/testvectors/signing_payload.json` pinning the canonical byte
  encoding — the same command must produce the same bytes in both languages, or signatures made by
  one are unverifiable by the other. Both harnesses gain a runner.
- **Rust**: new `domain/signature.rs` and `application/transport.rs`. `domain/command_envelope.rs`
  and `domain/command_acceptance.rs` both change shape. New dependency: `ed25519-dalek` 3.0.
- **Dart**: mirrors of each. New dependency: `cryptography` 2.9.
- **Storage**: unchanged. No migration, no schema bump in either database.
- **Wire format**: `CommandEnvelope` changes shape, and existing `command_acceptance.json` vectors
  must be updated to carry signatures instead of a verified flag. This is the one place this change
  edits existing vector cases rather than adding to them, because the field they assert on is gone.
- **Security posture**: this is the change that makes the authenticity guarantee real rather than
  specified. After it, a relay that hands the target a command the sender did not produce is
  refused by arithmetic rather than by policy. It still adds no network surface.

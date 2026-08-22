# Give each device a key of its own, and somewhere to keep the keys it trusts

## Why

Signature verification is real, but nothing supplies the keys. `evaluate_command` takes a
`HashMap<DeviceId, VerifyingKey>` and no code anywhere fills it — every test constructs one by hand.
The capability's own Purpose says so: keys are supplied to these rules rather than generated or
stored by them.

That leaves the guarantee one step short of what the design needs. Today a verified signature proves
*"this command came from whoever holds that key"*. It does not yet prove *"this command came from
that device"*, because nothing establishes that a signing key belongs to one device and stays there.
A private key sitting in a plain file verifies exactly as well as one in a Keychain — and a key
copied off a stolen laptop would keep working forever, with no way to notice or revoke it.

Two things are missing, and they are the same gap seen from both ends:

- **A device has no key of its own.** No keypair is generated, and no private key is written
  anywhere, securely or otherwise.
- **A device has no record of the keys it trusts.** `remote-device-pairing` specifies pairing as the
  only act that confers authority, and specifies revocation — but there is no pairing store, so
  neither can happen. Pairing today is a decision function with nothing behind it.

This change closes both. It is deliberately the last one that can be done without a device: after
it, the remaining work is a transport and a UI.

## What Changes

**Every device gets exactly one long-lived signing identity.**

- A keypair is generated once, on first run, and the private key is written to the platform's secure
  store — Keychain on macOS, Credential Manager on Windows, Secret Service on Linux, Keystore on
  Android, Keychain on iOS. The public half and the `DeviceId` derived from it are stored in the
  ordinary database, because neither is secret.
- The private key is **write-once**: after generation, the only operation the rest of the app can
  perform with it is signing. There is no read-the-private-key call, no export, and no logging path
  that could carry it. **BREAKING** in the sense that `ed25519-dalek` gains the `rand_core` feature,
  which the previous change deliberately excluded — this is the change that has a reason to
  generate.

**A device records the keys it trusts, and can stop trusting them.**

- A pairing store in both databases: which `DeviceId` this device is paired with, that peer's
  verifying key, when the pairing was established, and whether it has been revoked. **BREAKING** for
  the on-disk format: schema version bumps in both implementations.
- `evaluate_command`'s key map stops being a hand-built argument and starts coming from the store,
  so authenticity is checked against keys the user actually paired rather than whatever a caller
  passed.
- Revocation is a local act with immediate effect and no network dependency, as
  `remote-device-pairing` already requires. A revoked pairing's key is refused for every subsequent
  command.

**Signing, so a device can act as a sender rather than only as a target.**

- A `sign_command` path that produces an envelope from this device's identity, using the same
  canonical encoding the verifier already uses. Until now the codebase could only verify; the
  cross-language fixtures were produced by test helpers rather than by the app.

Deliberately **not** in this change:

- No transport. No Firebase, HTTP, WebSocket, or FCM. The scaffolded Firebase config stays unread.
- No OAuth and no account model. `AuthProvider` remains a fake.
- No pairing UI in either app, and no mail delivery. This change gives pairing somewhere to write to
  and something to write; the flows that call it come later. The out-of-band grant path additionally
  needs Cloud Functions, which the free tier does not include — recorded here so the constraint is
  not rediscovered later.
- No new Tauri command, and no change to `capabilities/default.json`.

## Capabilities

### New Capabilities
- `device-identity`: How a device obtains, keeps, and uses a signing identity of its own — the
  once-only generation, the secure-store requirement for the private half, and the rule that the
  private key can be used but never read back.

### Modified Capabilities
- `remote-device-pairing`: Pairing and revocation gain a store, so the requirements that were
  specified-but-unimplemented become real. The capability's Purpose is rewritten to say what is now
  built and what still is not.
- `remote-command-authenticity`: The verifying keys used to check a command come from the pairing
  store rather than from a caller-supplied map, closing the last route by which a caller could
  influence whose signature counts.

## Impact

- **Shared contract**: new `shared/testvectors/pairing_store.json` covering the decisions a store
  makes that both implementations must agree on — a revoked pairing is refused, an unknown peer is
  refused, re-pairing a revoked device is permitted and clears the revocation. The store's *storage*
  is platform code; its *rules* are shared.
- **Rust**: new `domain/device_identity.rs`, `platform/secret_store.rs` (trait plus a `keyring`-backed
  implementation and an in-memory fake), `data/pairing_store.rs`. New dependencies: `keyring` 4.1 and
  the `rand_core` feature on `ed25519-dalek`.
- **Dart**: mirrors of each, with `flutter_secure_storage` 11.0 behind the same trait shape.
- **Storage**: **both schemas change.** New `pairings` and `device_identity` tables. Additive; no
  existing row is rewritten.
- **Security posture**: this is the change that makes a key device-held rather than merely present.
  After it, a verified signature means the command came from a device the user paired, on hardware
  that holds the key — not merely from something that had a copy of it. It still adds no network
  surface.
- **Platform verification limits**: the secure-store implementations cannot be exercised on a
  developer machine for every target. What is verifiable here is every decision and the fake-backed
  paths; what is not is whether Keystore, Credential Manager, and Secret Service behave as expected
  on real hardware. That gap is named in the capability Purpose rather than left for a reader to
  infer.

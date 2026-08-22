# Add the authenticity rules a remote command must satisfy before it is obeyed

## Why

The previous change landed the rule that decides *whether* a paired device is allowed to send a
command. It did not, and could not, decide the prior question: **is this command genuinely from
that device, and is it the first time we have seen it?**

Authorization without authenticity is not a gate. `authorize()` today takes `is_paired: bool` as
an input and trusts whoever supplies it. If the transport hands it `true` on the strength of an
account session, then a stolen session is a shutdown; if it hands it `true` on the strength of a
forwarded message, then whoever forwards can shut the machine down. The archived spec already
records that same-account access must not be sufficient — but nothing yet defines what *is*.

Two decisions were taken with the user and are the reason this change exists rather than a
simpler one:

- **The device signs, not the server.** The relay forwards opaque signed bytes. It can drop or
  delay a command; it cannot invent one. This is what keeps a compromised relay from being a
  shutdown for every user of the service, and it is the property that makes hosting the relay on
  managed infrastructure an acceptable choice rather than a concentration of risk.
- **Pairing is an act of authorization, and the only act of authorization.** It happens once per
  device pair, either at the target machine or by a one-time code delivered to the account owner's
  own mailbox. After that the app signs every command with no further user action — the daily
  experience is a single tap.

There is also a third question that a signature alone does not answer, and it is the one most
often missed: a captured signed command stays valid forever. Replaying it shuts the machine down
at a moment nobody chose. A power-off is irreversible, so freshness is not a refinement here.

This change lands those rules — signing, freshness, replay rejection, and pairing-grant validity
— as pure decisions in both languages with shared vectors, **before** any transport exists to
bake a wrong answer into a wire format. It also pays the two debts the previous change named as
blockers, because both must be settled before the first remote command can be obeyed.

## What Changes

Three pure decision rules, implemented in Rust and Dart, driven by shared vectors:

- **Command acceptance.** `(envelope, target state, now) → Accepted | Rejected(reason)`. One
  decision that composes authenticity, freshness, and replay into a single answer with exactly
  one enumerated reason, so a refusal can be explained rather than shown as a generic failure.
  This wraps the existing `authorize()` rather than replacing it — authorization stays the
  question of *permission*, this adds the question of *authenticity*.

- **Freshness and replay.** A command carries the instant it was created and a nonce. A command
  older than a bounded window is refused, and a nonce already seen is refused. Both boundaries
  are fixed by vectors rather than by whichever comparison each implementation happens to use.
  A command dated in the *future* beyond a small tolerance is also refused — a skewed sender
  clock must not buy an attacker a longer replay window.

- **Pairing grant validity.** `(grant, now) → Valid | Invalid(reason)`. A pairing code is
  single-use and expires; a grant delivered out-of-band to the owner's mailbox expires sooner
  than one shown at the machine, because possession of the machine is stronger evidence than
  possession of a mailbox.

And the two debts the previous change named as blockers:

- **`JobOrigin` is persisted.** Today `sqlite_repository.rs` hardcodes `JobOrigin::default()` on
  every read, so a remote job that survives a restart reads back as `Local` and is given the
  **60-second** countdown instead of 300. Both databases gain the column and a migration.
  **BREAKING** for the on-disk format: schema version bumps in both implementations.

- **Every command decision is recorded.** The requirement removed from the previous change for
  lack of a device identity and a persistence story now has both, so it lands here. A power-off
  is irreversible; without a record, nobody can determine afterwards which device caused one.

Deliberately **not** in this change — each depends on a transport or a UI that does not exist,
and committing to its shape now would be speculation rather than design:

- No network code. No HTTP, no WebSocket, no Firebase, no FCM/APNs, no relay.
- No OAuth, no Google sign-in, no account model, no `AccountId`.
- No key generation, no Keychain/Keystore integration, no platform secret storage. The rules
  take a verification outcome as an input; they do not perform cryptography. See design D2.
- No choice of signature algorithm. That belongs with the code that generates keys.
- No pairing UI in either app, and no mail delivery. The rule decides whether a grant is valid;
  it does not create, display, or send one.
- No new Tauri command, and no change to `capabilities/default.json`.

The rules delivered here are complete and independently verified, and they are the part that is
identical regardless of which transport is chosen. Their limit is that nothing calls them yet —
stated in each capability's Purpose so the archived spec does not read as a description of
working behavior.

## Capabilities

### New Capabilities
- `remote-command-authenticity`: How a target device decides that an arriving command genuinely
  came from a paired device, is recent enough to obey, and has not been seen before — the
  enumerated rejection reasons, the freshness window, and the rule that a signature alone is
  never sufficient.
- `remote-device-pairing`: What makes a pairing grant valid — single use, expiry, and the two
  delivery paths with their different lifetimes — and the rule that pairing is the only act that
  confers authority.

### Modified Capabilities
- `remote-command-authorization`: Gains the requirement that authorization is evaluated only
  after authenticity has been established, so `is_paired` can no longer be satisfied by an
  account session. Also gains back the recording requirement removed from the previous change,
  now that a device identity exists to name the requester.
- `power-job-scheduling`: The job model's origin becomes durable. A remote-origin job SHALL read
  back from storage as remote, so the countdown it receives after a restart is the one its origin
  earns.

## Impact

- **Shared contract**: new `shared/testvectors/command_acceptance.json` and
  `shared/testvectors/pairing_grant.json`, plus README sections documenting both wire formats.
  Both harnesses — `desktop/src-tauri/tests/shared_vectors.rs` and
  `mobile/test/shared/shared_vectors_test.dart` — gain a runner for each, including the guard
  that fails when a case sets contradictory expectations.
- **Rust**: new `domain/command_envelope.rs`, `domain/command_acceptance.rs`,
  `domain/pairing_grant.rs`, `domain/device_id.rs`. `domain/remote_command.rs` gains the
  composition point.
- **Dart**: mirrors of each, with denial and rejection messages byte-identical to the Rust ones.
- **Storage**: **both schemas change.** Rust `SCHEMA_VERSION` 3 → 4 with
  `ALTER TABLE jobs ADD COLUMN origin TEXT`; Dart `schemaVersion` 2 → 3 with a matching
  `onUpgrade` step. A new table records command decisions. Pre-migration rows read as `local`,
  which is what they mean.
- **Wire format**: no existing format changes. The two new vector files are additive and every
  existing case is untouched.
- **Security posture**: this change adds no network surface, and deliberately so. It decides the
  rules a transport must satisfy while there is still no code depending on the answer. The rules
  it fixes are the ones that determine whether a compromised relay can shut down a machine.

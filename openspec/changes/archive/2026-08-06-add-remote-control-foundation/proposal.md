# Add the decision rules that a multi-device account will need

## Why

The mobile app cannot power off its own device — `PlatformCapabilities.resolve()` reports
`supportsPowerOff: false` on Android and iOS, and that is an OS limitation rather than missing
code. So half of what the app models is unreachable from a phone. The way to make a phone
useful here is not to fight the OS but to let it drive a machine that *can* power off: the
user's own desktop, signed in to the same account.

That is a large piece of work — an account server, a transport, device pairing, presence, and
a remote command path — and the riskiest parts of it are not the network code. They are the
*rules*: how long a silent device stays "active", and what a remote power-off request is
allowed to do. Both are pure decisions that the desktop and mobile implementations must agree
on exactly, and this repository has already been burned once by that class of problem: the
scheduling logic is written twice, in Rust and in Dart, and `shared/testvectors/` exists
because a rule corrected in one language and forgotten in the other leaves both test suites
green.

This change lands those rules first, in both languages, with shared vectors — before any
transport exists to bake a wrong answer into a wire format.

## What Changes

Two pure decision rules, implemented in Rust and Dart, driven by shared vectors:

- **Presence evaluation.** `(last_seen, now) → Online | Stale | Offline`. A device that has not
  reported recently is not simply "offline": there is a middle state, because a phone that has
  been backgrounded by iOS is neither reachable nor gone. Presenting a binary online/offline
  would be a lie the user catches immediately.

- **Remote command authorization.** `(command, target capabilities, pairing state, remote
  control setting) → Allowed | Denied(reason)`. Every reason is enumerated, so a refusal can
  be explained rather than shown as a generic failure.

And one structural safety rule, enforced by a source-level test in the same style as the
existing `no_command_can_reach_a_power_off_executor`:

- **A remote request never reaches `PowerOffGate`.** It can only create a `Job`, which the
  existing local scheduler then runs through the existing countdown. There is no remote path to
  `PowerOffExecutor`, so the mandatory cancellable countdown cannot be bypassed by a caller who
  is not at the machine.

- **A remote-initiated power-off gets a longer countdown than a local one.** The 60-second
  figure assumes the person who scheduled it is sitting at the machine and expecting it.
  Nobody is, in the remote case, so the person actually using the machine gets more time to
  refuse. `GRACE_PERIOD_SECONDS` is unchanged; the remote value is a separate, longer constant.

Deliberately **not** in this change — each depends on a server that does not exist, and
committing to its shape now would be speculation rather than design:

- No server, no HTTP, no WebSocket, no push (FCM/APNs).
- No authentication, no tokens, no password handling, no 2FA.
- No device registry tables in either database, and no schema version bump.
- No new Tauri command, no change to `capabilities/default.json`, no UI in either app.
- No account model. `AccountId` is not introduced; pairing is represented by the *state* a
  decision needs, not by a persisted record.
- No recording of remote command decisions. Attributing a power-off to the device that requested
  it needs both a device identity and somewhere to write the record, and neither exists here.
  See design.md D9 — this is a blocker for the transport change, not a later refinement.
- No persisted remote-control setting. The authorization rule takes "is remote control enabled" as
  a required input it is given, not as a stored value it reads, so remote control counts as on only
  when a caller affirmatively says so. Persisting it — and with it a real storage-level default —
  lands with the change that adds the setting's UI, which is what first gives a user a way to turn
  it on. See design.md D10.

This is a conscious staging decision, not a partial implementation: the rules delivered here
are complete and independently verified, and they are the part that is identical regardless of
which transport is chosen later. Their limit is that nothing calls them yet.

## Capabilities

### New Capabilities
- `remote-device-presence`: How a device's liveness is derived from its last report — the three
  presence states, their boundaries, and the rule that presence is never presented as a binary.
- `remote-command-authorization`: Which remote commands a target device accepts, the enumerated
  reasons for refusal, and the invariant that a remote power-off is mediated by a job and the
  existing countdown rather than by direct execution.

### Modified Capabilities
- `device-power-off`: The mandatory-countdown requirement currently fixes one duration for all
  power-offs. It gains the remote case: a longer countdown when the request did not originate
  at the machine, and an explicit statement that no remote caller can shorten or skip it.
- `platform-capability-probe`: The capability model gains whether the device can act as a
  remote-control target, so the authorization rule branches on the existing model rather than
  on a new platform check.

## Impact

- **Shared contract**: new `shared/testvectors/presence.json` and
  `shared/testvectors/remote_authorization.json`, plus README sections documenting both wire
  formats. Both harnesses — `desktop/src-tauri/tests/shared_vectors.rs` and
  `mobile/test/shared/shared_vectors_test.dart` — gain a runner for each file, including the
  guard that fails when a case sets contradictory expectations.
- **Rust**: new `domain/presence.rs` and `domain/remote_command.rs`. `platform/capabilities.rs`
  gains one field. A new source-level test asserts no remote type reaches `PowerOffGate`.
- **Dart**: new `lib/domain/presence.dart` and `lib/domain/remote_command.dart`, mirroring the
  Rust rules and their messages. `lib/platform/platform_capabilities.dart` gains one field.
- **Storage**: unchanged. No migration, no schema version bump in either database.
- **Wire format**: no existing format changes. The two new files are additive, and every
  existing vector case is untouched.
- **Security posture**: unchanged by this change, and deliberately so — it adds no network
  surface. It does record the constraints that the eventual transport must satisfy, so those
  are decided while there is still no code depending on the answer.

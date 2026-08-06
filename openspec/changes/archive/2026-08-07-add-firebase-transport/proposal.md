# Connect the two devices: a real relay, a real account, and a real pairing flow

## Why

Everything decided so far is uncalled. `evaluate_command` verifies signatures against keys from a
real pairing store, `RemoteTransport` has a fake behind it, and the Firebase project exists with
rules deployed — but no device has ever spoken to another. A phone and a desktop signed in as the
same person do not know each other exists.

This change closes that. It is the first one whose value the user can see, and the first whose
correctness this repository cannot fully establish on its own: OAuth redirects, FCM delivery, and
Keychain access on a real handset are things that either work on a device or do not, and no test
suite here decides which.

The order matters and has been deliberate. The rules landed first so that a transport could not
bake a wrong answer into a wire format; signature verification landed before the transport so the
relay could never be the reason a command is obeyed; keys and pairings landed before the flow so
that pairing had somewhere to write. What remains is genuinely the pipe.

## What Changes

**An account, for routing only.**

- `AuthProvider` gains a Google Sign-In implementation. It answers one question — which account
  this device belongs to — and it is used to find the user's own devices and address messages to
  them. It is **not** consulted about whether a command may be obeyed; pairing decides that, and
  the authorization rule already refuses to treat same-account access as sufficient.
- No password path, no email/password, no anonymous sign-in. The Firebase project is configured
  with Google as the only provider.

**A relay implementation behind the existing interface.**

- `FirebaseTransport` implementing `RemoteTransport` over Firestore: register a device, report
  presence, list the account's devices, send a signed envelope, receive envelopes addressed here.
- It moves opaque bytes. The `remote-transport` capability already requires the interface expose no
  way to assert authenticity, and this implementation adds none.
- Wake-up over FCM so a desktop that is idle learns a command arrived without polling on a tight
  loop. **BREAKING** for the desktop's network posture: the Tauri app gains outbound network
  access it did not have. See design D2 — this is the single largest change to the app's attack
  surface in the project so far, and it is confined to Rust rather than granted to the web view.

**A pairing flow the user can actually complete.**

- The desktop shows a pairing code; the phone enters it. Both devices must be running, which is
  the property the `remote-device-pairing` capability already requires and nothing has been able
  to exercise.
- On success each device records the other's `DeviceId` and verifying key in the pairing store
  that already exists.
- A paired-devices list on both platforms, showing presence derived by the existing rule from the
  `lastSeen` the transport now reports, and offering revocation — which the store already
  implements and no surface has been able to reach.

**Remote control, the thing the user asked for.**

- From the phone: see whether a paired desktop is online, and ask it to power off or to keep the
  screen awake. The request becomes a signed envelope; the desktop verifies it, creates a job, and
  runs it through the existing cancellable countdown — 300 seconds for a remote-origin power-off,
  because nobody at the machine asked for it.

Deliberately **not** in this change:

- No out-of-band pairing code by email. It needs Cloud Functions, which the project's free tier
  does not include. Recorded in `remote-device-pairing` already; the at-machine path is unaffected.
- No key rotation, unchanged from the previous change and for the same reason.
- No web or Linux desktop pairing UI beyond what the existing frontend structure supports.

## Capabilities

### New Capabilities
- `account-identity`: How a device establishes which account it belongs to, what that identity is
  used for — device discovery and message routing — and the standing rule that it never decides
  whether a command is obeyed.

### Modified Capabilities
- `remote-transport`: The interface gains a real implementation, so requirements that constrained a
  future change now describe running behavior. The capability's Purpose is rewritten accordingly.
- `remote-device-pairing`: Pairing gains a flow that establishes it and a surface that revokes it,
  so the requirements about both stop being unreachable.
- `remote-command-authorization`: Remote control gains the enable/disable setting the capability
  already requires to default off and to be changeable only at the target.

## Impact

- **Rust**: new `platform/firebase_transport.rs` and `platform/google_auth.rs`. New dependencies
  for HTTPS and JSON over the wire. The `capabilities/default.json` file is **unchanged** — the web
  view gains nothing; all network access is Rust-side. See design D2.
- **Dart**: new `platform/firebase_transport.dart` and `platform/google_auth.dart`, plus pairing
  and remote-control screens.
- **Frontend**: new pairing and paired-devices surfaces in the desktop web view, driven by new
  Tauri commands. No new power-off command — the existing job path is reused, which is what keeps
  the countdown mandatory.
- **Config**: `google-services.json` and `GoogleService-Info.plist` enter the tree. Neither is
  secret in the sense of a credential — they identify the project, and Firestore rules are what
  protect the data — but this is stated rather than assumed.
- **Storage**: unchanged. No migration and no schema bump; pairing and identity tables already
  exist.
- **Verification limits**: this change cannot be fully verified here. What can be tested is every
  decision, every serialization boundary, and the transport contract against the existing fake.
  What cannot is whether OAuth completes on a handset, whether FCM wakes a sleeping desktop, and
  whether Keychain behaves outside macOS. Those are named in each capability's Purpose rather than
  reported as working.

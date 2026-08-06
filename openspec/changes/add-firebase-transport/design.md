## Context

See proposal.md — Why. This is the change that connects two devices, and the first whose correctness
this repository cannot fully establish. Three constraints shape it.

**The desktop web view has no network access, deliberately.** `tauri.conf.json` sets
`default-src 'self'`, and `capabilities/default.json` carries no `http:`, `fs:`, `shell:`, or
`process:` permission, with a comment saying why: "nothing in the web view has any business touching
those, and the power-off path is Rust-side only." That posture was chosen when the app was offline.
It is now load-bearing in a stronger way, and D2 is about not giving it away.

**The safety structure is untouched.** `PowerOffGate` holds the only `PowerOffExecutor` and takes no
duration argument; source-text tests fail the build if a remote module references it. A remote
request creates a job and the existing scheduler runs it. Nothing here goes near that path.

**The rules are already decided and already agree across languages.** This change adds no decision
that both implementations must share — it adds two implementations of an existing interface. That is
why it introduces no new shared vectors, which would otherwise be the first thing to reach for.

## Goals / Non-Goals

**Goals:**

- A user can sign in, pair a phone with a desktop, see whether it is online, and power it off from
  across the room or across the city.
- The relay stores opaque signed bytes and can be wholly compromised without gaining the ability to
  shut down a machine.
- The desktop's new network access is confined to Rust and does not widen what the web view can do.
- Everything that can be verified without a device is verified without a device, and everything that
  cannot is named rather than implied.

**Non-Goals:**

- No out-of-band pairing code by mail — needs Cloud Functions, which the free tier excludes.
- No key rotation, unchanged from the previous change and for the same reasons.
- No offline command queueing. A command that cannot be delivered now is not held for later: it
  would arrive stale and be refused, so holding it only delays a failure the user should see at once.

## Decisions

### D1: The relay is Firestore documents, not a custom protocol

A `devices` collection carrying identifier, verifying key, owner, and last-reported instant; a
`commands` collection carrying opaque signed envelopes addressed to one device.

Chosen over a custom service because the relay's job is now genuinely trivial — it stores bytes it
cannot read and reports a timestamp it does not interpret. Every interesting property was moved out
of it by the preceding changes, so paying for a server to run it would buy nothing that Firestore's
free tier does not already provide. The rules deployed with the project already scope both
collections by owner and deny updates to a command, since a mutable envelope is one a relay could
alter after the fact.

Firestore's real-time listeners also remove the need for a polling loop on the desktop, which
matters more than it sounds: a poll interval short enough to feel immediate would consume the free
tier's daily read quota within hours.

### D2: All network access is Rust-side; the web view's posture does not change

`capabilities/default.json` is **not modified**. The web view gains no `http:` permission, and the
CSP stays `default-src 'self'`. Firestore and FCM are reached from Rust, and the frontend learns
about devices and commands through Tauri commands exactly as it learns about jobs today.

This is the decision most likely to be undone later by someone reaching for the Firebase JavaScript
SDK, so the reasoning is recorded rather than implied. The web view renders untrusted-ish content and
is the largest attack surface in the app; granting it outbound network access would let anything that
achieves script execution there reach the network directly. Keeping the network in Rust means the
frontend can only ask for the specific operations the command surface exposes.

The cost is real: the Firebase JS SDK would have been considerably less work than speaking Firestore's
REST API from Rust, and this change carries hand-written request and response types as a result. That
cost is accepted deliberately. A future contributor tempted to reverse it should read this paragraph
first.

### D3: The desktop signs in through the system browser, not an embedded web view

OAuth is completed by opening the user's default browser and receiving the redirect on a loopback
listener bound to `127.0.0.1` on an ephemeral port.

An embedded web view for sign-in is what phishing looks like: the user cannot inspect the address
bar, cannot see the certificate, and has no way to tell a real Google page from a rendered
imitation. Google's own guidance is that native apps use the system browser for exactly this reason,
and it additionally lets the user's existing session and password manager work.

The loopback listener binds to `127.0.0.1` rather than `0.0.0.0`, accepts exactly one request, and
shuts down immediately afterwards, so it is not a service that outlives the sign-in.

### D4: FCM wakes the desktop; the listener is what delivers

A command arriving produces a push, but the push carries no command content — it is a signal to look.
The Firestore listener is what actually delivers the envelope.

Putting command content in a push would place it in a channel neither device controls, and would mean
a delivered-but-unread push is a command the target believes it never received. Treating the push as
a wake-up only means the two paths cannot disagree: the listener is the single source, and the push
merely reduces how long a sleeping machine takes to consult it.

This also degrades honestly. If push fails entirely — an expired APNs certificate, a Play Services
outage, a user who disabled notifications — the system still works, just with more latency, because
the listener remains the delivery path.

### D5: Presence is reported every 60 seconds, matching the threshold already chosen

A device writes its `lastSeen` every 60 seconds while running.

The presence rule's online threshold is already 90 seconds, chosen to tolerate one missed report on a
60-second heartbeat plus jitter. Reporting on that interval is what makes the existing threshold mean
what its own documentation says. Choosing any other interval here would silently change what "online"
means without editing the rule that defines it.

Quota is the other constraint and it is not close: two devices at 60 seconds is roughly 2,880 writes
per day against a 20,000 free-tier limit. A 10-second interval would be 17,280 and would leave almost
nothing for commands.

### D6: A pairing code is short, human-transcribable, and derived from a grant that already expires

Six characters from an alphabet excluding visually confusable glyphs, presented at the target and
typed into the requesting device.

The grant's validity rules already exist and are vector-verified: single use, expiring, with distinct
lifetimes per delivery path. This change adds the *presentation*, not the rule. Six characters from a
32-glyph alphabet is about 33 bits, which is weak in isolation and adequate here because the grant
lives for five minutes, can be used once, and is only accepted by the device that issued it.

Excluding `0`/`O` and `1`/`I`/`l` is not cosmetic: a user who mistypes a code sees a failure they
cannot distinguish from an expired one, and will retry until the grant expires for real.

### D7: The pairing exchange completes on both sides or neither

Each device records the peer only after confirming the peer recorded it.

A half-recorded pairing is the worst outcome available: the requesting device believes it is
authorized, sends commands, and every one is refused for a reason the user cannot see. The spec
requires both-or-neither, and the implementation confirms before recording rather than recording
optimistically and repairing later.

### D8: Hand-written wire types, not generated clients

Firestore's REST API is spoken through hand-written request and response structs in Rust and Dart.

A generated client would drag in a large surface for the handful of operations this needs — write a
document, read a collection, listen for changes — and would put a code generator in the build for
both languages. The types here are small and the endpoints few. The trade-off is that a Firestore API
change breaks at runtime rather than at compile time, which is why every response field this code
depends on is parsed explicitly and a missing field is an error rather than a default.

## Risks / Trade-offs

- **This change cannot be fully verified in this repository** → The central limitation. OAuth
  completion, FCM delivery, and secure-store behaviour on a handset are decided by the device, not by
  a test suite. What is testable — every decision, every serialization boundary, the transport
  contract against the existing hostile fake — is tested; the rest is named in each capability's
  Purpose and in the completion report rather than reported as working.

- **The desktop gains outbound network access it did not have** → The largest change to the app's
  attack surface so far. Confined to Rust by D2, with the web view's CSP and capability set unchanged,
  so the frontend cannot reach the network even if script execution is achieved there.

- **A compromised relay can drop or delay every command** → Accepted and unavoidable for any relayed
  design. It cannot forge one, which is the property that matters for an irreversible action. A user
  whose relay is down loses remote control and keeps every local function, including the countdown
  and revocation.

- **Firestore free-tier quota is finite** → Two devices at a 60-second heartbeat use roughly 14% of
  the daily write allowance. A user with many devices, or a future shorter interval, would approach
  it. The interval is a single constant per implementation.

- **`google-services.json` and `GoogleService-Info.plist` enter the repository** → These identify the
  project rather than authenticating anyone; Firestore rules are what protect the data, and they are
  in the tree already. Stated explicitly because "API key in the repo" is alarming at a glance and
  the distinction is worth recording once.

- **Hand-written wire types break at runtime if the API changes** → See D8. Mitigated by parsing every
  depended-upon field explicitly, so a shape change surfaces as a named error rather than a silent
  default.

## Migration Plan

None at the storage layer — no schema change, no migration. The pairing and identity tables already
exist and this change is their first caller.

A user upgrading from a build without this change sees remote control disabled, no pairings, and no
account signed in, which is the correct starting state. Rolling back leaves the pairing rows in place
and unread; re-applying finds them and they remain valid, since nothing about a pairing depends on the
transport that established it.

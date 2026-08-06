# Shared test vectors

Language-agnostic scheduling scenarios executed by **both** implementations:

- Rust desktop — `desktop/src-tauri/tests/shared_vectors.rs`
- Dart mobile — `mobile/test/shared/shared_vectors_test.dart`

## Why these exist

The pure scheduling logic (~928 lines) is implemented twice, in Rust for desktop and
in Dart for mobile. Duplication was accepted deliberately (see
`openspec/changes/split-desktop-to-tauri/design.md`, D3) because FFI was
disproportionate at this size. The risk that decision creates is *silent drift*: a
rule corrected in one language and forgotten in the other, with both test suites
still green because each only checks itself.

These vectors close that gap. Both suites read these exact files, so a rule that
diverges turns one suite red and names the case.

## Rule

Any behavioral rule that both implementations must share needs a vector case here.
Until both suites execute it, the rule is not cross-implementation-verified — no
matter how well either side unit-tests it privately.

## Files

| File | Covers |
| --- | --- |
| `resolution.json` | Trigger → absolute UTC target instant, including both DST anomalies |
| `reconciliation.json` | Overdue job → outcome, including the 15-minute power-off tolerance |
| `validation.json` | Trigger validation accept/reject boundaries |
| `presence.json` | Last report time → three-state presence, including both threshold boundaries |
| `remote_authorization.json` | Remote command → accept or refuse, including the refusal-reason precedence |
| `command_acceptance.json` | Arriving command envelope → accept or refuse, including freshness, replay, and the reason precedence |
| `pairing_grant.json` | Presented pairing grant → valid or refused, including both delivery lifetimes |

## Format

All timestamps are ISO-8601 with an explicit `Z` offset. `now` is always supplied
rather than read from the clock, so cases are deterministic and DST edges are
reachable on any machine in any zone.

### `resolution.json`

```json
{
  "id": "unique-case-id",
  "description": "what this case pins down and why it matters",
  "now": "2026-07-30T07:00:00Z",
  "timezone": "Asia/Ho_Chi_Minh",
  "trigger": { "kind": "absoluteTime", "hour": 20, "minute": 0 },
  "expectedTargetInstantUtc": "2026-07-30T13:00:00Z"
}
```

`expectedTargetInstantUtc` is `null` for an indefinite trigger, which has no target
by design.

A case may instead expect a rejection, which a dated `absoluteTime` trigger makes possible:

```json
{
  "id": "absolute-dated-past-rejected",
  "description": "...",
  "now": "2026-07-30T07:00:00Z",
  "timezone": "Asia/Ho_Chi_Minh",
  "trigger": { "kind": "absoluteTime", "hour": 20, "minute": 0, "date": "2026-07-01" },
  "expectedErrorContains": "already passed"
}
```

A case sets **exactly one** of `expectedTargetInstantUtc` and `expectedErrorContains`; both
harnesses fail a case that sets both or neither. The second key is necessary rather than
convenient: `expectedTargetInstantUtc: null` already means "this trigger has no instant" and
so cannot be overloaded to mean "this trigger is refused".

### `reconciliation.json`

```json
{
  "id": "unique-case-id",
  "description": "...",
  "jobType": "powerOff",
  "targetInstantUtc": "2026-07-30T09:55:00Z",
  "now": "2026-07-30T10:00:00Z",
  "expectedOutcome": "proceedToGracePeriod"
}
```

`expectedOutcome` is one of `stillPending`, `completed`, `proceedToGracePeriod`,
`overdue`.

**`proceedToGracePeriod` never means "power off now."** It means the job enters the
mandatory, cancellable 60-second countdown. No vector outcome authorizes immediate
execution.

### `validation.json`

```json
{
  "id": "unique-case-id",
  "description": "...",
  "jobType": "powerOff",
  "trigger": { "kind": "duration", "minutes": 1441 },
  "expectedValid": false,
  "expectedErrorContains": "24 hours"
}
```

`expectedErrorContains` is a substring match, so the two implementations may word
messages differently while still agreeing on the substantive reason. It is omitted
when `expectedValid` is `true`.

### `presence.json`

```json
{
  "id": "unique-case-id",
  "description": "...",
  "lastSeen": "2026-07-30T10:00:00Z",
  "now": "2026-07-30T10:01:30Z",
  "expectedState": "online",
  "expectedElapsedSeconds": 90
}
```

`expectedState` is one of `online`, `stale`, `offline`.

Two fixed thresholds, identical in both implementations and not user-configurable — a
tunable presence window would let a device be configured to look online indefinitely:

| Threshold | Value | Meaning |
| --- | --- | --- |
| online | 90 seconds | Tolerates one missed report on a 60-second heartbeat plus jitter, without requiring a second missed one |
| offline | 15 minutes | The same magnitude as `POWER_OFF_OVERTOLERANCE_MINUTES`, rather than a third timescale |

**Boundaries are inclusive-online.** Silence of exactly 90 seconds is `online`; exactly
15 minutes is `stale`. Each boundary is pinned from both sides — `online-exactly-at-threshold`
against `stale-just-past-online-threshold`, and `stale-exactly-at-offline-threshold`
against `offline-just-past-offline-threshold` — so neither can be decided by whichever
comparison operator an implementation happens to use.

`lastSeen` is `null` for a device that has never reported, and `expectedElapsedSeconds`
is then `null` too: there is no age to report, and treating the absent value as the
current instant would show a device that has never been seen as online.

A `lastSeen` **in the future** — a peer with a skewed clock — clamps to zero elapsed and
reports `online`. It is not an error: treating it as one would make the peer's clock skew
look like a local failure. Two cases pin this, at five seconds and at a month of skew, so
an implementation that clamps only within a small tolerance does not pass.

The elapsed time is returned alongside the state rather than left to the caller to
subtract, because a second subtraction outside these vectors is where the two
implementations would drift by an off-by-one.

### `remote_authorization.json`

```json
{
  "id": "unique-case-id",
  "description": "...",
  "command": "powerOff",
  "targetCanPowerOff": true,
  "targetIsRemoteTarget": true,
  "isPaired": true,
  "remoteControlEnabled": false,
  "expectedDecision": "denied",
  "expectedReason": "remoteControlDisabled"
}
```

All five context fields are named in every case; both harnesses fail a case that omits
one, since a defaulted boolean here would silently authorize a command that should be
refused.

`expectedDecision` is `allowed` or `denied`. `expectedReason` is required when denied,
absent when allowed, and is drawn from a closed set of four:

| Reason | Holds when |
| --- | --- |
| `commandNotRemotelyAllowed` | The command is not one any target accepts remotely |
| `remoteControlDisabled` | The target's owner has not enabled remote control |
| `notPaired` | The requesting device is not paired with the target |
| `platformCannotPerform` | The target cannot act as a remote target, or cannot perform this particular command |

**Several reasons can hold at once, so the check order is part of the contract**, in
exactly the order above. It is not arbitrary — it discloses least. An unpaired requester
learns only that it is unpaired, not whether the target has remote control on or what the
target is capable of, so a caller cannot map an account's devices by reading refusal
reasons. Capability is checked last for that reason.

The `precedence-*` cases exist to pin this: each sets up two or more simultaneous reasons
and names the one that must surface. Without them both implementations would return "a"
correct denial and the vectors would agree or disagree depending on how each case happened
to be constructed.

### `command_acceptance.json`

Whether an arriving command is *real*, as distinct from whether it is *permitted*. The two
are separate questions and this one runs first.

```json
{
  "id": "unique-case-id",
  "description": "...",
  "senderDeviceId": "phone-a",
  "command": "powerOff",
  "createdAt": "2026-08-06T12:00:00Z",
  "nonce": "n-accept-power-off",
  "signatureVerified": true,
  "now": "2026-08-06T12:00:10Z",
  "seenNonces": [],
  "targetCanPowerOff": true,
  "targetIsRemoteTarget": true,
  "isPaired": true,
  "remoteControlEnabled": true,
  "expectedDecision": "accepted"
}
```

Every field is named in every case; both harnesses fail a case that omits one. A defaulted
boolean here is not a cosmetic problem — `signatureVerified` defaulted to `true` would
accept a forged command.

`signatureVerified` is a verification **outcome the caller computes**, not a signature these
rules check. Verifying one needs a key, a key store, an algorithm, and a platform: none of
those can be pure, none can be identical across Rust and Dart, and vectors carrying real key
material would test each language's crypto library rather than the shared decision. The rules
decide what to do about a command that failed verification; they do not decide whether it
failed. **The change that adds a transport must verify signatures against a key held only by
the paired devices, and must carry an end-to-end test that an invalid signature is rejected.**
Until then the authenticity guarantee is specified, not enforced.

The envelope carries no field an intermediary could populate — no `relayAttested`, no
`serverVerified`, no `trustedSource`. A relay must be able to drop or delay a command and
never to invent one, and a field a relay could set would satisfy that in prose while
contradicting it in shape. Omitting it makes the property true by construction.

`expectedDecision` is `accepted` or `rejected`. `expectedReason` is required when rejected,
absent when accepted, and is drawn from a closed set of five:

| Reason | Holds when |
| --- | --- |
| `authenticityUnverified` | The command's signature did not verify against a pairing the target holds |
| `replayedNonce` | The target has already acted on a command bearing this nonce |
| `futureDated` | The stated creation instant is further ahead than the tolerance |
| `stale` | The stated creation instant is older than the freshness window |
| `notPermitted` | The command is authentic, fresh, and new, but the authorization rule refuses it |

Two fixed time constants, identical in both implementations:

| Constant | Value | Meaning |
| --- | --- | --- |
| freshness window | 120 seconds | Survives a backgrounded phone, a relay retry, and ordinary mobile latency, without leaving a captured command usable for a meaningful period |
| future tolerance | 30 seconds | Enough for unsynchronised consumer clocks, not enough to be useful for extending the replay window |

The freshness window is deliberately **not** the 90 seconds of the presence threshold.
Presence asks "is this device still there", which tolerates a missed heartbeat; freshness
asks "was this command issued just now". One constant for both would couple two unrelated
questions.

**Boundaries are inclusive-accept.** An age of exactly 120 seconds is accepted; a future
offset of exactly 30 seconds is accepted. Each is pinned from both sides —
`fresh-exactly-at-window` against `stale-one-second-past-window`, and
`future-exactly-at-tolerance` against `future-one-second-past-tolerance` — so neither can be
decided by whichever comparison operator an implementation happens to use.

A nonce may be forgotten only once a command bearing it can no longer pass the freshness
check, so the required retention is derived rather than chosen: it must exceed the freshness
window plus the future tolerance. A compile-time assertion in each implementation pins that
relationship, because shortening retention reopens the replay window the nonce exists to
close.

**Several reasons can hold at once, so the check order is part of the contract**, in exactly
the order of the table above. It is not arbitrary. Authenticity is first because the spec
requires it: every reason after the first leaks something about the target's state, so a
caller must not be able to probe permission state, freshness windows, or which nonces a
target has seen by sending unauthenticated commands. Replay precedes the two time reasons
because a replayed command is evidence of an attack while a stale one is more often a bad
network, and the record should show an attack as an attack. `notPermitted` is last because it
leaks the most, and is reached only by a command already authentic, fresh, and new.

The `precedence-*` cases pin this, including
`precedence-all-five-reasons-hold-at-once`. Note that `futureDated` and `stale` are mutually
exclusive by construction, so `futureDated`'s place in the order is pinned against
`notPermitted` rather than against staleness.

### `pairing_grant.json`

Whether a grant offered to establish a pairing may still be redeemed. Pairing is the only act
that confers authority to command a device, so this is the decision that gates everything the
acceptance rules later assume.

```json
{
  "id": "unique-case-id",
  "description": "...",
  "delivery": "atMachine",
  "issuedAt": "2026-08-06T12:00:00Z",
  "redeemed": false,
  "recognised": true,
  "now": "2026-08-06T12:00:30Z",
  "expectedValidity": "valid"
}
```

`delivery` is `atMachine` or `outOfBand`. `expectedValidity` is `valid` or `invalid`.
`expectedReason` is required when invalid, absent when valid, and is drawn from a closed set
of three:

| Reason | Holds when |
| --- | --- |
| `alreadyUsed` | The grant has been redeemed before |
| `expired` | The grant's lifetime has elapsed |
| `noSuchGrant` | The target never issued this grant |

Two fixed lifetimes, identical in both implementations:

| Delivery | Lifetime | Meaning |
| --- | --- | --- |
| `atMachine` | 300 seconds | A code shown on screen is typed within seconds; 300 exists only to absorb a user who walks between rooms |
| `outOfBand` | 600 seconds | Must survive mail delivery latency, which nobody controls and which routinely exceeds a minute |

**Boundaries are inclusive-accept**, pinned from both sides for each delivery path.

The out-of-band grant is **longer in wall-clock terms despite resting on weaker evidence**,
and that is deliberate rather than an oversight to be corrected. The two windows are not
measuring the same thing: possession of the machine is stronger evidence than possession of a
mailbox, but a mailbox grant has to survive delivery. Setting out-of-band to 300 seconds would
fail honest users often enough that they would retry repeatedly, and a flow that habitually
fails trains people to expect failure. The weaker evidence is compensated where it actually
matters — the grant is single-use, it is delivered only to an address the owner has already
proven control of and never to one supplied in the request, and it authorises *pairing* rather
than a shutdown. An intercepted grant still cannot power anything off without the person also
completing a pairing the owner can see and revoke. If real mail latency turns out to be fast,
the right change is to shorten the out-of-band lifetime.

The `age-between-the-two-lifetimes-splits-by-delivery-*` pair is what proves the two lifetimes
are actually distinct: identical instants and identical state, differing only by delivery path,
with opposite outcomes. Without it, an implementation using one lifetime for both would pass
every other case in the file.

`alreadyUsed` takes precedence over `expired` — an already-redeemed grant reports that fact
whether or not it has also expired — and recognition is settled before either, because a
target has no reason to trust the claimed fields of a grant it never issued.

## Trigger wire format

| `kind` | Fields |
| --- | --- |
| `indefinite` | none |
| `duration` | `minutes` (integer) |
| `absoluteTime` | `hour` (0-23), `minute` (0-59), optional `date` (`YYYY-MM-DD`) |

### The two `absoluteTime` semantics

`date` is optional, and its presence changes what the trigger means:

- **Omitted** — a time of day, resolved to its next occurrence. If that time has already
  passed today it rolls to tomorrow. This is the original and only prior behaviour, so an
  undated trigger serialises byte-identically to before and every pre-existing case is
  unchanged.
- **Present** — a one-off instant on exactly that local date. It does **not** roll forward. A
  dated instant that is not strictly in the future is refused, because silently moving an
  irreversible power-off to a day the user never chose is worse than refusing.

The pair `absolute-past-rolls-to-tomorrow` and `absolute-dated-does-not-roll-forward` exists
to pin that difference: identical `now`, zone, and time-of-day, differing only by the date,
with opposite outcomes.

### Where the past-date rule lives, and why

Validation asks only questions that need no context; resolution asks the ones that need a
clock and a zone. That split is not new — duration bounds are already validated while the DST
anomalies are resolved — and the date follows it:

| Question | Needs | Home |
| --- | --- | --- |
| Is the month 1-12? Is `2026-02-30` a real day? | nothing | the parse boundary |
| Are the hour and minute in range? | nothing | `validate` |
| Has `2026-07-01T20:00` already passed? | `now`, timezone | `resolve` |

So `validate` keeps its signature — it receives no `now` in either language, and
`validation.json` cases carry no clock — and the past-date rejection appears in
`resolution.json` instead.

**Malformed dates are deliberately not vector cases.** A value like `2026-02-30` or a month of
13 cannot be represented in either implementation's parsed trigger form (Rust `NaiveDate`,
Dart `CalendarDate`), so such a case would make the vector *file* unparseable rather than
exercise a rule. Well-formedness is enforced where the trigger is parsed, and is tested per
language.

## Verified DST reference values

The DST cases use values confirmed by running `chrono-tz` rather than derived by
hand, since getting these wrong silently is the exact failure these vectors exist to
prevent:

| Local wall time | Zone | Result |
| --- | --- | --- |
| 2026-03-08 02:30 | America/New_York | does not exist (spring-forward gap); clock jumps to 03:00 EDT = `07:00Z` |
| 2026-11-01 01:30 | America/New_York | occurs twice (fall-back); earlier = `05:30Z`, later = `06:30Z` |

## Context

See proposal.md — Why. Two constraints shape this design.

The first is the one recorded in `shared/testvectors/README.md`: the scheduling logic exists twice,
in Rust and Dart, and the only thing preventing silent drift is that both suites execute the same
vector files. Any rule both implementations must share needs a vector case, or it is not
cross-implementation-verified no matter how well either side unit-tests it privately.

The second is the architecture decision taken with the user, which this design exists to honour:
**the sending device signs; the relay only forwards.** A relay that cannot forge a command is a
relay whose compromise costs the user availability rather than control of their machine. Every
decision below is downstream of that, and the one place it could be quietly undone — a rule that
accepts a relay's word for who sent something — is called out explicitly in D2 and enforced by D3.

Existing structure that is load-bearing and not to be disturbed:

- `PowerOffGate` (`desktop/src-tauri/src/application/grace_period.rs`) holds the only
  `PowerOffExecutor` in the crate and takes no duration argument. Its own doc comment: "an argument
  that can skip the wait is an argument someone will pass."
- `authorize()` in `domain/remote_command.rs` is pure, takes a context of named fields, and returns
  `Allowed` or `Denied(reason)` with a closed reason set and a fixed precedence order.
- Source-text tests (`commands/tests.rs`, `domain/remote_command_tests.rs`) enforce the "no path to
  the executor" rule on source text rather than on types, because privacy already permits the call.
- `TriggerResolver` is pure — `now` is always passed in. That is what makes vectors deterministic.

## Goals / Non-Goals

**Goals:**

- Authenticity, freshness, and replay are one composed decision with one enumerated reason, pure,
  vector-driven, and byte-identical across both languages including every boundary.
- The rules are expressible without choosing a signature algorithm, a key store, or a transport.
- `JobOrigin` becomes durable in both databases, so the countdown a job receives after a restart is
  the one its origin earns.
- Command decisions are recorded, making an irreversible action attributable.

**Non-Goals:**

- No cryptography is performed by these rules. See D2 — this is the central design decision.
- No transport, no relay, no OAuth, no key generation, no platform secret storage.
- No pairing UI and no mail delivery. The rule decides whether a grant is valid; it does not create,
  display, or send one.
- No decision about how the nonce store is pruned beyond the bound the rules require. A production
  pruning policy needs operational data this change does not have.

## Decisions

### D1: Acceptance is one composed decision, not three independent checks

`evaluate_command(envelope, target_state, now) -> CommandAcceptance` returning `Accepted` or
`Rejected(RejectionReason)`, where the reason set spans authenticity, freshness, future-dating,
replay, and permission.

The alternative — three separate functions the caller composes — was rejected because the
composition *is* the security property. A caller that checks authenticity and forgets replay has
written a working system with a silent hole, and nothing in the type system objects. Composing once,
inside the vector-covered function, means the ordering and the completeness are pinned by tests
rather than by each call site's discipline.

This wraps `authorize()` rather than replacing it. Authorization stays the question of permission and
keeps its own vectors and its own reason set; acceptance adds the prior question and delegates. The
seam is deliberate: permission is a policy that will change as features are added, authenticity is
not.

### D2: The rules take a verification *outcome*, not a signature or a key

The envelope carries `signature_verified: bool` — a value the caller computes — rather than a
signature the rule verifies.

This is the decision most likely to look like a shortcut, so the reasoning is recorded in full.

Verifying a signature requires a key, a key store, an algorithm, and a platform. None of those can be
pure, none can be identical across Rust and Dart, and all four are excluded from this change. A rule
that took raw bytes and a public key could not be vector-driven at all: the vectors would have to
carry real key material and real signatures, and the two implementations would be testing their
crypto libraries rather than agreeing on a decision.

Splitting it this way keeps every *decision* pure and shared, while leaving every *cryptographic
operation* to the change that introduces keys. The rules decide what to do about a command that
failed verification; they do not decide whether it failed.

The cost, stated plainly: nothing in this change prevents a caller from passing
`signature_verified: true` unconditionally. That would defeat the entire model, and no test here can
catch it, because the caller does not exist yet. Two mitigations, and they are not equivalent:

1. The field is named for what it asserts, not for what the caller wants — `signature_verified`, not
   `trusted` or `valid`. A caller hardcoding it reads as obviously wrong at the call site.
2. **This is a blocker for the transport change, not a follow-up.** The change that introduces the
   transport must verify signatures against a key held only by the paired devices, and must carry a
   test that a command with an invalid signature is rejected end to end. Until that exists, the
   authenticity guarantee is specified but not enforced. That is stated in the capability's Purpose
   so the archived spec cannot be read as a claim that it works today.

### D3: A relay is structurally incapable of asserting authenticity

`CommandEnvelope` carries no field an intermediary could populate to influence the decision. There is
no `relay_attested`, no `server_verified`, no `trusted_source`.

The spec requires that an intermediary's cooperation never be sufficient. A struct with a field a
relay could set would satisfy that requirement in prose while contradicting it in shape — and the
field would eventually be read, because a field that exists gets used. Omitting it makes the
requirement true by construction rather than by review, which is the same argument `PowerOffGate`
makes for taking no duration parameter.

### D4: Freshness is 120 seconds, future tolerance is 30 seconds, and both boundaries are inclusive-accept

`FRESHNESS_WINDOW_SECONDS = 120`, `FUTURE_TOLERANCE_SECONDS = 30`. An age of exactly 120s is
accepted; a future offset of exactly 30s is accepted. Stated explicitly because the spec requires the
exact-boundary cases be fixed by vectors rather than by whichever comparison operator each
implementation happens to use.

120 seconds is chosen to survive a phone that was backgrounded mid-send, a relay retry, and ordinary
mobile latency, without leaving a captured command usable for a meaningful period. It is deliberately
*not* the 90 seconds of `ONLINE_THRESHOLD_SECONDS`: presence asks "is this device still there", which
tolerates a missed heartbeat, whereas freshness asks "was this command issued just now". Reusing one
constant for both would couple two unrelated questions and make either unchangeable without the
other.

30 seconds for future tolerance is small on purpose. Every second of tolerance is a second added to
the window in which a captured command remains obeyable, so this trades against the same property
freshness protects. It is enough for unsynchronised consumer clocks and not enough to be useful.

Future-dating is refused with its **own** reason rather than folded into staleness. The two indicate
different faults — one is a sender clock ahead, the other a command that sat too long — and a user
who sees "command expired" for a clock-skew problem will spend the evening looking in the wrong
place.

### D5: Rejection precedence is fixed, authenticity first

Checked in order: `AuthenticityUnverified` → `ReplayedNonce` → `FutureDated` → `Stale` →
`NotPermitted`.

Multiple reasons can hold at once, and an unspecified order is drift waiting to happen. Authenticity
is first because the spec requires it: a caller must not be able to probe permission state, freshness
windows, or which nonces a target has seen by sending unauthenticated commands. Every reason after
the first leaks something about target state, so nothing may precede authenticity.

Replay precedes the two time reasons deliberately. A replayed command is evidence of an attack,
whereas a stale one is more often a bad network; reporting the more serious finding when both hold
means the record shows an attack as an attack. `NotPermitted` is last because it leaks the most —
whether remote control is enabled, what the platform can do — and is reached only by a command that
is already authentic, fresh, and new.

### D6: The nonce store is an input, and its required retention is derived rather than chosen

`target_state.seen_nonces` is passed in; the rule performs no lookup and no eviction.

Retention is not a free parameter. A nonce may be forgotten only once the command bearing it can no
longer pass the freshness check — otherwise forgetting reopens the replay window the nonce existed to
close. So the requirement is `retention > FRESHNESS_WINDOW_SECONDS + FUTURE_TOLERANCE_SECONDS`, and
`NONCE_RETENTION_SECONDS = 600` satisfies it with margin. A compile-time assertion pins the
relationship, in the same style as the existing assertion that the remote countdown exceeds the local
one, so a later contributor who shortens retention or lengthens freshness gets a build error rather
than a silent hole.

Note what this constant is *not*: it is not a promise about how long the audit record is kept. The
record has its own lifetime and a different purpose — attribution rather than replay defence.

### D7: A pairing grant's lifetime is a property of its delivery path

`GrantDelivery { AtMachine, OutOfBand }` with `AT_MACHINE_GRANT_LIFETIME_SECONDS = 300` and
`OUT_OF_BAND_GRANT_LIFETIME_SECONDS = 600`.

A compile-time assertion enforces that out-of-band is *not longer* than at-machine would be if the
evidence were equal — but the values as chosen make out-of-band **longer** in wall-clock terms, and
that requires explanation, because the spec says out-of-band evidence is weaker.

The resolution: the two windows are not measuring the same thing. A code shown on screen is typed
within seconds — 300s is already generous and exists only to absorb a user who walks between rooms. A
code sent to a mailbox must survive mail delivery latency, which is not under anyone's control and
routinely exceeds a minute. Setting out-of-band to 300s would fail honest users often enough that
they would retry repeatedly, and a flow that habitually fails trains people to expect failure. 600s
is the shortest value that is workable.

The weaker evidence is compensated where it actually matters — the grant is single-use, it is
delivered only to an address already proven (spec requirement, not caller-supplied), and it authorises
pairing rather than a shutdown. An intercepted grant still cannot power anything off without the
person also completing a pairing the owner can see and revoke.

Stated as a trade-off rather than hidden: if mail delivery in practice turns out to be fast, the right
change is to shorten the out-of-band lifetime, and the constant is in one place per language to make
that a two-line edit plus vector updates.

### D8: `JobOrigin` is persisted by appending a column, mirroring the `trigger_date` precedent

Rust `SCHEMA_VERSION` 3 → 4 with `ALTER TABLE jobs ADD COLUMN origin TEXT`; Dart `schemaVersion`
2 → 3 with a matching `onUpgrade` step. The column is **appended to the end of `JOB_COLUMNS`**, not
placed beside the other job fields.

`row_to_job` reads by positional index. Inserting a column mid-list renumbers every index after it,
which the compiler cannot catch because they are all integers. Appending makes the new column index
13 and leaves 0–12 untouched. This is exactly what `trigger_date` did in the previous change, and
`sqlite_repository.rs:200` carries the comment explaining why; the same comment shape is used here so
the reason travels with the code.

A `NULL` origin reads as `Local`. Every row written before this change was locally scheduled, so this
is a faithful reading of old data rather than a default standing in for missing information.

This closes the gap D7 of the previous change named: `sqlite_repository.rs:179-181` currently
hardcodes `JobOrigin::default()` on every read, so a remote job surviving a restart is handed the
60-second countdown instead of 300. That is a live shortening of a safety countdown, and it is fixed
here rather than deferred again.

### D9: The command record is a separate table, and writing it is not optional

A `command_decisions` table keyed by device, command, decision, reason, and instant — not a column on
`jobs`, and not a log file.

Not on `jobs`, because refused commands create no job and refusals are precisely what must be
recorded: a series of them is the visible signature of an attack. A record that only exists when the
command succeeded would be blind to the case it is most needed for.

Not a log file, because the spec requires the record be readable at the target without a network, and
because a log rotated by size loses the oldest entries first — which is the opposite of what
attribution needs after an unexplained shutdown.

The write is part of reaching the decision, not a side effect a caller may skip. This is the
requirement that was removed from the previous change for lack of a device identity and a persistence
story; both now exist, so it lands here rather than being deferred a second time.

### D10: `DeviceId` is an opaque identifier and carries no account meaning

A newtype over a string, validated non-empty, with no structure imposed.

The authorization rule's own spec says same-account access is not sufficient, so account identity is
not what any of these rules branch on. Giving `DeviceId` structure — embedding an account, a platform,
a public key fingerprint — would embed a guess about the account model into rules that do not need it,
which is the argument D4 of the previous change made for keeping `PlatformCapabilities` out of the
authorization context. The transport change chooses what a device identifier looks like; these rules
only need to compare two of them and record one.

## Risks / Trade-offs

- **`signature_verified` is a boolean the caller supplies, so these rules cannot enforce the property
  they are named for** → See D2. This is the single largest gap in the change and it is deliberate:
  the alternative is unvector-able crypto in two languages. Named as a blocker for the transport
  change, which must verify against a key held only by the paired devices and carry an end-to-end
  test that an invalid signature is rejected. Recorded in the capability's Purpose so the archived
  spec cannot be misread as working behavior.

- **The rules are uncalled until a transport exists** → Accepted deliberately; they are the parts
  that do not change with the transport choice. The mitigation against rot is that both vector suites
  execute them from day one, so a change that breaks them fails CI with no caller. The previous
  change established this pattern and the honesty requirement that goes with it.

- **A 120-second freshness window leaves a captured command usable for up to two minutes** → True,
  and irreducible without breaking honest use on mobile networks. The countdown is the compensating
  control: even a perfectly replayed command inside the window yields a cancellable 300-second
  countdown at the machine, so the attack costs the user an interruption rather than an unexpected
  shutdown.

- **Nonce retention grows unboundedly if never pruned** → The rules take the store as an input and do
  not prune, so an implementation that never evicts will grow. `NONCE_RETENTION_SECONDS` states the
  floor below which pruning is unsafe, with a compile-time assertion tying it to the freshness
  window; choosing a policy above that floor belongs to the change that owns the store.

- **Two schema migrations land in one change** → Both are additive column adds with a null-means-the-
  old-meaning reading, and both are exercised by a migration test that opens a pre-migration file.
  The alternative — splitting them across two changes — would leave the origin gap open for another
  cycle, and that gap actively shortens a safety countdown today.

- **The out-of-band grant lives longer in wall-clock terms than the at-machine one, despite resting on
  weaker evidence** → See D7. Compensated by single use, a delivery address that cannot be
  caller-supplied, and the fact that a grant authorises pairing rather than a shutdown. Flagged as the
  value most likely to want revising once real mail latency is known.

- **Recording every decision creates a record of when the owner is at their machine** → A privacy cost
  accepted for attribution of an irreversible action. It is local-only by requirement, never sent to a
  relay, so it is exposed only to someone who already has the device.

## Migration Plan

Two additive migrations, one per implementation, both idempotent in effect:

- Rust: `SCHEMA_VERSION` 3 → 4, `ALTER TABLE jobs ADD COLUMN origin TEXT`, plus `CREATE TABLE IF NOT
  EXISTS command_decisions`. Pre-migration rows have `NULL` origin and read as `Local`.
- Dart: `schemaVersion` 2 → 3, matching `onUpgrade` step in the existing `MigrationStrategy`.

Rollback is dropping the new table and ignoring the new column; an older binary reading a migrated
database sees columns it does not select, which SQLite tolerates. No wire format changes, so a
rollback needs no coordination with any other device.

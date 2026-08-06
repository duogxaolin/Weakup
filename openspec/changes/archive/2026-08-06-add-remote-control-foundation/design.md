## Context

See proposal.md — Why. The constraint that shapes this design is the one already recorded in
`shared/testvectors/README.md`: the scheduling logic exists twice, in Rust and Dart, and the only
thing preventing silent drift is that both suites execute the same vector files. Any rule that
both implementations must share needs a vector case, or it is not cross-implementation-verified
no matter how well either side unit-tests it privately.

Three pieces of existing structure are load-bearing here and are not to be disturbed:

- `PowerOffGate` (`desktop/src-tauri/src/application/grace_period.rs`) holds the only
  `PowerOffExecutor` in the crate and takes no duration argument. Its own doc comment states the
  reason: "an argument that can skip the wait is an argument someone will pass."
- `desktop/src-tauri/src/commands/tests.rs` reads `mod.rs` as a string and fails if it contains
  `pub fn power_off`, `pub fn shutdown_now`, or `pub fn skip_grace`. The safety rule is enforced
  on source text, not on types.
- `TriggerResolver` is pure — no clock, no persistence, no platform. `now` is always passed in.
  That is what makes the vectors deterministic and DST edges reachable on any machine.

The new rules must fit those, which is most of what follows.

## Goals / Non-Goals

**Goals:**

- The two decision rules are pure functions with `now` passed in, so both are vector-driven.
- The presence and authorization rules are byte-identically decided in Rust and Dart, boundary
  cases included, verified by shared vectors rather than by parallel unit tests.
- The "no remote path to power-off" invariant is enforced the same way the existing one is: a
  source-text test that fails when a forbidden reference appears.
- Adding this leaves every existing vector case, wire format, and database schema untouched.

**Non-Goals (design-level, beyond the proposal's scope list):**

- No decision here about HTTP vs WebSocket, or about how `last_seen` reaches the evaluating
  device. The presence rule takes an instant and does not care how it was obtained.
- No decision about how pairing is persisted or established. The authorization rule takes
  "is this device paired" as an input, not as a lookup it performs.
- No `AccountId` type. Introducing one now would embed an assumption about the account model into
  a rule that does not need it — the authorization rule's own spec says same-account is not
  sufficient, so account identity is not what it branches on.

## Decisions

### D1: Presence is three states with the elapsed time attached, not an enum alone

`PresenceEvaluation { state, elapsed }` rather than a bare `PresenceState`.

The spec requires the age be available for a non-online device, and a caller that has only the
state must recompute the elapsed time from the same two inputs — which is where the two
implementations would drift by an off-by-one. Returning both from the single place that already
computed the subtraction removes the second computation entirely.

Alternative considered: return just the state and let the UI subtract. Rejected because it puts
arithmetic that decides user-visible text outside the vector-covered function.

### D2: Thresholds are 90 seconds and 15 minutes, and boundaries are inclusive-online

`ONLINE_THRESHOLD = 90s`, `OFFLINE_THRESHOLD = 15min`. Silence of exactly 90s is `online`;
exactly 15min is `stale`. Stated explicitly because the spec requires the exact-boundary cases be
fixed by vectors rather than by whichever comparison operator each implementation happens to use.

90 seconds is chosen to tolerate one missed report on a 60-second heartbeat plus jitter, without
requiring a second missed one. 15 minutes matches the shape of the existing
`POWER_OFF_OVERTOLERANCE_MINUTES = 15` — reusing a magnitude the codebase already reasons about,
rather than introducing a third timescale. They are separate constants regardless, because they
answer different questions and coupling them would make one unchangeable without the other.

Negative elapsed time — a `last_seen` in the future, from a device with a skewed clock — is
treated as `online` with a clamped elapsed of zero. The alternative, treating it as an error,
would make a peer's clock skew look like a local failure. Two vector cases pin this.

### D3: The authorization decision is a Result-shaped enum, not a bool plus optional reason

`RemoteCommandDecision::Allowed` / `Denied(DenialReason)`, with `DenialReason` closed.

A `(bool, Option<Reason>)` pair admits the two nonsense states — allowed-with-reason and
denied-without — and something will eventually construct one. A two-variant enum where the reason
lives inside `Denied` makes both unrepresentable, which is the same argument
`CalendarDate`'s validating constructor already makes for February 30th.

`DenialReason` is `NotPaired`, `RemoteControlDisabled`, `PlatformCannotPerform`,
`CommandNotRemotelyAllowed`. Closed rather than a string so that adding a fifth reason is a
compile error at every match site in Rust, and so the vectors can name reasons exactly.

### D4: Authorization takes a context struct, not four positional arguments

`RemoteCommandContext { command, target_can_power_off, target_is_remote_target, is_paired,
remote_control_enabled }`.

Four booleans in a row is a call site nobody can read and a transposition nobody catches — and a
transposition here means authorizing a command that should be refused. Named fields make the
vector JSON a direct transcription of the struct, which also keeps the two harnesses' parsing
honest.

The context carries the *capability booleans* rather than a `PlatformCapabilities` instance. The
rule must be evaluable for a *remote* device from data that arrived over the wire, and
`PlatformCapabilities::resolve()` describes the *local* machine. Passing the whole model would
invite calling `resolve()` inside the rule and silently authorizing against the wrong device.

### D5: Denial reason precedence is fixed and ordered

Checked in order: `CommandNotRemotelyAllowed` → `RemoteControlDisabled` → `NotPaired` →
`PlatformCannotPerform`.

Multiple reasons can hold at once, and an unspecified order is drift waiting to happen — both
implementations would return "a" correct denial and the vectors would fail intermittently
depending on case construction. The order is not arbitrary: it discloses least. An unpaired
requester learns only that it is unpaired, not whether the target has remote control on or what
the target can do — so a caller cannot map an account's devices by reading refusal reasons.
Vector cases exist where two reasons hold simultaneously, pinning which one surfaces.

### D6: The remote countdown is a separate constant, and the gate still takes no argument

`REMOTE_GRACE_PERIOD_SECONDS = 300` alongside the unchanged `GRACE_PERIOD_SECONDS = 60`.
`PowerOffGate::run` gains no parameter. The gate selects the duration from the job's own origin,
which it already has access to via the job.

This is the decision most likely to be got wrong by a later contributor, so the reasoning is
recorded rather than implied: adding `run(job_id, duration)` would satisfy the spec's letter and
destroy its substance, because the countdown's safety comes from there being no way to pass a
short value. The origin is a property of the job, not of the call, so the gate reads it rather
than being told it.

Five minutes rather than two: the person at the machine did not schedule this and may not be
looking at the screen when the countdown starts. Under two minutes, a user who stepped away for
coffee has no real chance to refuse.

`GRACE_PERIOD_SECONDS` keeps its exact current value and name so every existing grace-period test
passes unmodified. A new test asserts the remote constant is strictly greater — a regression that
made them equal would otherwise be invisible.

### D7: Job origin is a domain field, but is not persisted in this change

`JobOrigin { Local, Remote }` on the job in memory. No column, no schema bump, no migration.

The proposal excludes storage changes, and nothing yet creates a remote job, so a column would be
written by nothing and read by nothing. The origin defaults to `Local`, which is what every
existing construction site means, so no call site changes behavior.

Trade-off, stated plainly: when persistence lands, a remote job that survives a restart will read
back as `Local` and get the 60-second countdown. That is the *safe* direction of the error only in
the sense that it does not skip a countdown — it does shorten one. So the storage task in the next
change is not optional, and the design task list for that change must carry it as a blocker rather
than an improvement. A test in this change asserts the default is `Local` so the gap is documented
by code and not only by this paragraph.

### D8: The structural safety test greps the new source, in the existing style

A test in the same shape as `no_command_can_reach_a_power_off_executor`: `include_str!` the remote
command module and fail if it references `power_off(`, `PowerOffGate`, `PowerOffExecutor`, or
`GraceOutcome`.

Chosen over relying on Rust's privacy rules because privacy already permits this — the gate is
`pub` within the crate, so a future remote handler in the same crate could legally call it. The
existing test exists for exactly that reason and the new rule needs the same protection. Source
text is a blunt instrument, but it is the one already in use here and consistency matters more
than elegance for a rule whose whole job is to be hard to remove accidentally.

### D9: Command recording is specified by the transport change, not by this one

No audit record, no requesting-device identity, no timestamp. The authorization rule returns a
decision and nothing observes it.

The requirement this defers is that every remote command evaluation be recorded — the requesting
device, the command, the decision, and the refusal reason when refused — so that an irreversible
action can be attributed afterwards. It cannot land here because it needs two things this change
explicitly excludes: a device identity to name the requester, and a persistence story to write the
record to. There is no `DeviceId` to record and nowhere to put it, and inventing either to satisfy
a requirement nothing yet exercises would embed a guess about the account model into a rule that
does not branch on it — the same argument D4 makes for keeping `PlatformCapabilities` out of the
authorization context.

Trade-off, stated plainly: a power-off is irreversible, and until recording exists a remote one is
unattributable. If a transport shipped against these rules as they stand, a user whose machine shut
down could not determine afterwards which device caused it — not from a log, not from the job, not
from anywhere. That is a worse failure than the one D7 describes, because a shortened countdown is
still a countdown someone can refuse, whereas an unattributable power-off is simply unexplainable
after the fact. So recording is a blocker for the change that adds the transport, not an
improvement to be scheduled behind it: the first change that lets a remote command reach this rule
must carry the recording task, and no transport should ship without it. Unlike D7, no test in this
change can document the gap, because there is no origin field to assert a default on — which is
precisely why it is written down here and named in the proposal's exclusion list rather than left
to be rediscovered.

### D10: The job-creation scenarios stay uncalled, and the remote-control setting stays unpersisted

Two spec-level gaps are left open deliberately, and both would be easy for a later author to
"fix" in a way that costs more than the gap does.

The first is that the authorization capability's job-creation scenarios — an authorized remote
power-off produces a `powerOff` job, an authorized remote cancel goes through the existing
job-cancellation path — bind a caller that does not exist. `authorize()` has exactly one caller in
the tree, the vector harness. Unlike the recording requirement D9 defers, these need no new
machinery: jobs, job cancellation, and the scheduler all already exist, so nothing has to be
invented to satisfy them. They stay because they are the positive half of the safety rule. The
prohibition — no remote command reaches the power-off executor — says only what must not happen;
without the paired prescription saying what must happen instead, a future author facing a
prohibition with no alternative has an open invitation to invent a third path. Keeping both halves
means the transport change arrives with its route already chosen. The capability's Purpose section
states plainly that these scenarios constrain that future change rather than describe current
behavior, so the distinction survives archiving into `openspec/specs/`, where proposal.md and
design.md do not follow.

The second is that there is no persisted remote-control setting. `remote_control_enabled` is a
required field on the authorization context in both languages — no `Default` impl in Rust, marked
`required` in Dart — supplied per call by whoever builds the context. The "disabled by default"
requirement is therefore a rule about what the decision does with the value, not about what storage
returns when unset, and the scenario is worded that way: remote control counts as on only when
affirmatively enabled. Persisting the setting, and with it a genuine storage-level default, lands
with the change that adds the setting's UI and storage — the same change that gives a user a way to
turn it on, which is the only thing that makes a stored value meaningful.

What a future author must not do: satisfy the default by adding a `Default` impl to
`RemoteCommandContext`. D4's entire argument is that the fields are named and required precisely so
that nothing is silently omitted at a call site where a transposition or an omission authorizes a
command that should be refused. A `Default` would let a caller construct a context without deciding
whether remote control is enabled, and the resulting decision would look deliberate while being an
accident. The default belongs in the storage layer that reads a user's setting, not in the struct
the rule branches on.

## Risks / Trade-offs

- **The rules are dead code until a transport exists** → Accepted deliberately, and the reason is
  in the proposal: these are the parts that do not change with the transport choice. The mitigation
  against them rotting is that both vector suites execute them from day one, so a change that
  breaks them fails CI even with no caller.

- **Chosen thresholds may prove wrong in the field** → They are constants in one place per
  language, pinned by vectors that name each boundary. Changing one is a two-line edit plus vector
  updates that will not pass unless both languages agree. No user-facing configuration is added,
  because a user-tunable presence window would let a device be configured to look online
  indefinitely.

- **`JobOrigin` not persisted means a restarted remote job gets the short countdown** → See D7.
  Documented by a test, and named as a blocker for the change that adds persistence.

- **Nothing records remote command decisions, so a remote power-off would be unattributable** →
  See D9. Deferred because recording needs a device identity and a persistence story, both
  excluded here. Named as a blocker for the change that adds the transport: the rules are safe to
  ship uncalled, but the first caller must arrive with recording already in place.

- **The archived spec describes command handling that has no caller, and a default with no
  storage** → See D10. The job-creation scenarios are kept rather than stripped, because a
  prohibition without its paired prescription invites a future author to invent a third path to
  power-off. Both capabilities' Purpose sections state the enforcement state explicitly, so the
  distinction survives archiving into `openspec/specs/` even though proposal.md and design.md do
  not. The remote-control setting stays unpersisted until the change that adds its UI and storage;
  the named hazard is satisfying the default with a `Default` impl on the context struct, which
  would let a caller construct a context without deciding.

- **Five minutes may be long enough for the target to be shut down by other means first** → The
  job is cancellable throughout and the scheduler already handles a job whose target passed while
  the app was down, via `reconcile`. No new overdue concept is introduced.

- **A source-text test can be defeated by aliasing the forbidden name** → True, and true of the
  existing test it copies. It stops the accident, not the determined author. Accepted for
  consistency; the type-level alternative (moving the gate into a private module) is a larger
  refactor of working safety-critical code and is not justified by this change.

## Migration Plan

None required. No schema change, no wire-format change, no existing behavior modified — the two
capability models gain a field whose value is derived from the platform already being matched on.
Rollback is deleting the new files and reverting the two capability structs.

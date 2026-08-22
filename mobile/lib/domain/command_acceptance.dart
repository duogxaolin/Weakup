/// Whether an arriving command is *real*, as distinct from whether it is *permitted*.
///
/// Authorization answers permission and keeps its own reason set. This library answers the
/// prior question — is this command genuinely from a paired device, recent enough to obey, and
/// one we have not already acted on — and it runs first.
///
/// # Why this is one decision rather than three checks a caller composes
///
/// The composition *is* the security property. A caller that checks authenticity and forgets
/// replay has written a working system with a silent hole, and nothing in the type system
/// objects. Composing once, inside the vector-covered function, means the ordering and the
/// completeness are pinned by tests rather than by each call site's discipline.
///
/// This wraps [authorizeRemoteCommand] rather than replacing it. The seam is deliberate:
/// permission is a policy that will change as features are added; authenticity is not.
///
/// # This library decides; it does not act
///
/// An accepted command is carried out by creating a *job*, which the target's existing
/// scheduler then runs by its existing rules — including the mandatory cancellable countdown a
/// remote-origin job earns. Establishing that a command is genuine must not shorten, skip, or
/// bypass that countdown: an authentic, fresh, unreplayed, permitted command is the *normal*
/// case, and the countdown exists for exactly that case.
///
/// Mirrors `desktop/src-tauri/src/domain/command_acceptance.rs`, including the rejection
/// messages verbatim. `shared/testvectors/command_acceptance.json` is what keeps the two
/// implementations from drifting.
library;

import 'command_envelope.dart';
import 'device_id.dart';
import 'remote_command.dart';
import 'signature.dart';
import 'signing_payload.dart';

/// How old a command may be and still be obeyed.
///
/// Two minutes survives a phone that was backgrounded mid-send, a relay retry, and ordinary
/// mobile latency, without leaving a captured command usable for a meaningful period.
///
/// Deliberately **not** the 90 seconds of `onlineThresholdSeconds`: presence asks "is this
/// device still there", which tolerates a missed heartbeat, whereas freshness asks "was this
/// command issued just now". One constant for both would couple two unrelated questions and
/// make either unchangeable without the other.
const int freshnessWindowSeconds = 120;

/// How far ahead of the target's clock a command may claim to have been created.
///
/// Small on purpose. Every second of tolerance is a second added to the window in which a
/// captured command remains obeyable, so this trades against the same property freshness
/// protects. Thirty seconds is enough for unsynchronised consumer clocks and not enough to be
/// useful to an attacker.
const int futureToleranceSeconds = 30;

/// The shortest time a nonce may be remembered for.
///
/// Not a free parameter. A nonce may be forgotten only once the command bearing it can no
/// longer pass the freshness check — otherwise forgetting reopens the replay window the nonce
/// existed to close. Hence [nonceRetentionOutlastsAcceptanceWindow] below, which derives the
/// floor rather than trusting this number.
///
/// Note what this is *not*: it is not a promise about how long the decision record is kept.
/// That record has its own lifetime and a different purpose — attribution rather than replay
/// defence.
const int nonceRetentionSeconds = 600;

/// Whether nonce retention outlasts the window in which a command can still be accepted.
///
/// Forgetting a nonce sooner would reopen the replay window the nonce exists to close.
///
/// **This is weaker than the Rust side's guard.** There, `const _: () = assert!(...)` makes
/// the mistake unbuildable; Dart has no compile-time assertion over constants, so this is
/// checked by the assertion in [assertNonceRetentionIsSafe] and by a unit test. A contributor
/// who shortens retention here gets a failing test rather than a failing build.
bool get nonceRetentionOutlastsAcceptanceWindow =>
    nonceRetentionSeconds > freshnessWindowSeconds + futureToleranceSeconds;

/// Fails in debug builds when retention has been shortened below the safe floor.
///
/// Called from [evaluateCommand] so the check runs wherever the rule is used, rather than only
/// where someone remembered to call it. `assert` is stripped in release builds, which is
/// precisely why the unit test exists alongside it.
void assertNonceRetentionIsSafe() {
  assert(
    nonceRetentionOutlastsAcceptanceWindow,
    'nonce retention must outlast the window in which a command can still be '
    'accepted; shortening it reopens the replay window the nonce exists to close',
  );
}

/// Why an arriving command was refused.
///
/// Closed rather than a string so the shared vectors can name reasons exactly rather than
/// matching on prose.
enum RejectionReason {
  /// The command's signature did not verify against a pairing this target holds. An
  /// intermediary's assertion that a command is genuine counts for nothing here.
  authenticityUnverified,

  /// This target has already acted on a command bearing this nonce.
  replayedNonce,

  /// The command claims to have been created further ahead than the clock tolerance allows.
  /// Distinct from staleness: a sender clock running ahead is a different fault from a command
  /// that sat too long.
  futureDated,

  /// The command was created longer ago than the freshness window allows.
  stale,

  /// The command is authentic, fresh, and new, but the authorization rule refuses it. The
  /// specific permission reason is deliberately not disclosed — see [evaluateCommand].
  notPermitted,
}

extension RejectionReasonX on RejectionReason {
  /// Prose for the person who asked.
  ///
  /// Specific per variant rather than a generic failure: a refusal the user cannot act on is
  /// indistinguishable from a bug. **These strings are byte-identical to the ones in
  /// `desktop/src-tauri/src/domain/command_acceptance.rs`** — a user who sees a refusal on
  /// their phone and again on their laptop must not be told two different things.
  String get userMessage => switch (this) {
        RejectionReason.authenticityUnverified =>
          'That command could not be confirmed as coming from a paired device. It '
              'was ignored.',
        RejectionReason.replayedNonce =>
          'That command had already been carried out once. Repeating it was '
              'refused.',
        RejectionReason.futureDated =>
          'That command is dated too far in the future. Check the clock on the '
              'device that sent it.',
        RejectionReason.stale =>
          'That command took too long to arrive and was refused. Send it again.',
        RejectionReason.notPermitted =>
          'That command is not permitted on the target device.',
      };

  /// The name both implementations serialise. A cross-language contract, not an internal
  /// detail — these strings appear in the shared vectors.
  String get wireName => switch (this) {
        RejectionReason.authenticityUnverified => 'authenticityUnverified',
        RejectionReason.replayedNonce => 'replayedNonce',
        RejectionReason.futureDated => 'futureDated',
        RejectionReason.stale => 'stale',
        RejectionReason.notPermitted => 'notPermitted',
      };
}

/// Accepted, or refused with exactly one reason.
///
/// The same shape as [RemoteCommandDecision], and for the same reason: a
/// `(bool, RejectionReason?)` pair would admit accepted-with-a-reason and refused-without-one,
/// and something would eventually construct one.
sealed class CommandAcceptance {
  const CommandAcceptance();

  bool get isAccepted => this is AcceptedCommand;

  /// The refusal reason, or null when accepted.
  RejectionReason? get rejectionReason =>
      this is RejectedCommand ? (this as RejectedCommand).reason : null;
}

/// The target will act on this command.
final class AcceptedCommand extends CommandAcceptance {
  const AcceptedCommand();

  @override
  bool operator ==(Object other) =>
      identical(this, other) || other is AcceptedCommand;

  @override
  int get hashCode => runtimeType.hashCode;

  @override
  String toString() => 'Accepted';
}

/// The target refuses, for exactly one enumerated reason.
final class RejectedCommand extends CommandAcceptance {
  const RejectedCommand(this.reason);

  final RejectionReason reason;

  /// The prose shown to the person who asked.
  String get userMessage => reason.userMessage;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      (other is RejectedCommand && other.reason == reason);

  @override
  int get hashCode => reason.hashCode;

  @override
  String toString() => 'Rejected(${reason.wireName})';
}

/// Everything about the *target* that [evaluateCommand] needs, named.
///
/// The nonce store is an input rather than something this rule looks up or prunes. Choosing a
/// pruning policy needs operational data this change does not have; [nonceRetentionSeconds]
/// states the floor below which pruning is unsafe, and the change that owns the store picks a
/// policy above it.
final class CommandTargetState {
  const CommandTargetState({
    required this.seenNonces,
    required this.verifyingKeys,
    required this.targetCanPowerOff,
    required this.targetIsRemoteTarget,
    required this.isPaired,
    required this.remoteControlEnabled,
  });

  /// Nonces this target has already acted on.
  final Set<String> seenNonces;

  /// The verifying key this target holds for each device it is paired with.
  ///
  /// A map rather than a single key because a target may be paired with several devices and
  /// must check the signature against the key belonging to the *claimed* sender specifically.
  /// Verifying against "any key the target holds" would let one paired device issue commands in
  /// another's name.
  ///
  /// A sender absent from this map is an authenticity failure, not a lookup that falls back to
  /// anything more permissive. See [evaluateCommand] for why that is not a distinct reason.
  ///
  /// How a key gets here is pairing's business. These rules take one; they do not generate,
  /// store, or fetch it.
  final Map<DeviceId, VerifyingKey> verifyingKeys;

  /// Whether the *target* can shut itself down.
  final bool targetCanPowerOff;

  /// Whether the *target* can act as a remote-control target at all.
  final bool targetIsRemoteTarget;

  /// Whether the sending device is paired with this target. Derived from a pairing the target
  /// itself holds and has verified the command against — never from an account session, an
  /// owner record, or a relay's assertion.
  final bool isPaired;

  /// Whether the target's owner has turned remote control on, at the target.
  final bool remoteControlEnabled;

  @override
  String toString() => 'CommandTargetState(seenNonces=${seenNonces.length}, '
      'canPowerOff=$targetCanPowerOff, '
      'isRemoteTarget=$targetIsRemoteTarget, '
      'paired=$isPaired, '
      'remoteControlEnabled=$remoteControlEnabled)';
}

/// Decides whether this target will act on this command.
///
/// **The order of these checks is part of the contract**, and the shared vectors pin it:
/// authenticityUnverified, then replayedNonce, then futureDated, then stale, then notPermitted.
/// Several reasons can hold at once, and an unspecified order would let both implementations
/// return "a" correct refusal while disagreeing case by case.
///
/// The order is not arbitrary. Authenticity is first because the spec requires it: every reason
/// after the first leaks something about the target's state, so a caller must not be able to
/// probe permission state, freshness windows, or which nonces this target has seen by sending
/// unauthenticated commands. Replay precedes the two time reasons deliberately — a replayed
/// command is evidence of an attack whereas a stale one is more often a bad network, and
/// reporting the more serious finding when both hold means the record shows an attack as an
/// attack. notPermitted is last because it leaks the most, and is reached only by a command
/// already authentic, fresh, and new.
///
/// Permission is delegated to [authorizeRemoteCommand] rather than reimplemented, and its
/// specific reason is deliberately collapsed into [RejectionReason.notPermitted] here. The two
/// rules answer different questions and keep separate reason sets.
///
/// # Authenticity is computed here, not supplied
///
/// This function verifies the signature itself, against a key from
/// [CommandTargetState.verifyingKeys]. It does not take a caller's word for it — there is no
/// boolean to pass. An earlier version accepted a `signatureVerified` flag, which meant a
/// transport could have satisfied the whole authenticity model by passing `true`.
///
/// Verification is composed *inside* this function rather than exposed as a separate call the
/// caller runs first, for the same reason the other four checks are: a caller that forgets one
/// produces a working system with a silent hole, and nothing in the type system objects.
///
/// # Why this returns a Future while the Rust mirror does not
///
/// The `cryptography` package's Ed25519 verification is asynchronous and has no synchronous
/// variant; `ed25519-dalek` is synchronous. The asymmetry belongs to the packages rather than to
/// the design — the same bytes are checked against the same key with the same result, and the
/// shared vectors hold both sides to it. Everything else here stays pure: [now] is still passed
/// in and no clock is read.
Future<CommandAcceptance> evaluateCommand({
  required CommandEnvelope envelope,
  required CommandTargetState targetState,
  required DateTime now,
}) async {
  assertNonceRetentionIsSafe();

  // Authenticity is computed here, not supplied. This function is the only place the check can
  // be composed with the others, which is what makes its ordering and its completeness testable
  // rather than a matter of each call site's discipline.
  //
  // Both failure modes below produce the *same* reason. Distinguishing "I hold no key for that
  // device" from "the signature did not verify" would tell an attacker which device identifiers
  // a target knows about, which is exactly the probing the precedence order exists to prevent.
  final key = targetState.verifyingKeys[envelope.sender];
  if (key == null) {
    return const RejectedCommand(RejectionReason.authenticityUnverified);
  }

  final payload = encodeSigningPayload(
    sender: envelope.sender,
    command: envelope.command.wireName,
    createdAt: envelope.createdAt,
    nonce: envelope.nonce,
  );

  final signatureIsGood = await verifySignature(
    payload: payload,
    signature: envelope.signature,
    key: key,
  );
  if (!signatureIsGood) {
    return const RejectedCommand(RejectionReason.authenticityUnverified);
  }

  if (targetState.seenNonces.contains(envelope.nonce)) {
    return const RejectedCommand(RejectionReason.replayedNonce);
  }

  // Positive when the command is older than `now`, negative when it claims the future. One
  // subtraction serves both bounds, so the two cannot drift apart.
  final ageSeconds = now.difference(envelope.createdAt).inSeconds;

  // Both boundaries are inclusive-accept: an age of exactly the window is accepted, and a
  // future offset of exactly the tolerance is accepted. A decision, not an accident of which
  // comparison operator was typed, which is why each is pinned from both sides.
  if (ageSeconds < -futureToleranceSeconds) {
    return const RejectedCommand(RejectionReason.futureDated);
  }

  if (ageSeconds > freshnessWindowSeconds) {
    return const RejectedCommand(RejectionReason.stale);
  }

  final decision = authorizeRemoteCommand(
    RemoteCommandContext(
      command: envelope.command,
      targetCanPowerOff: targetState.targetCanPowerOff,
      targetIsRemoteTarget: targetState.targetIsRemoteTarget,
      isPaired: targetState.isPaired,
      remoteControlEnabled: targetState.remoteControlEnabled,
    ),
  );

  return switch (decision) {
    AllowedDecision() => const AcceptedCommand(),
    DeniedDecision() => const RejectedCommand(RejectionReason.notPermitted),
  };
}

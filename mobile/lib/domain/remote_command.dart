/// Whether a command arriving from another device is allowed to act on this one.
///
/// Pure, and deliberately inert. This library decides; it does not act. An authorized
/// command is carried out by creating or modifying a *job*, which the target's existing
/// scheduler then runs by its existing rules — which is what keeps a remote power-off
/// inside the mandatory cancellable countdown. The countdown is reached because the job
/// path is the only path, not because the remote path chooses to be polite.
///
/// Mirrors `desktop/src-tauri/src/domain/remote_command.rs`, including the denial
/// messages verbatim. `shared/testvectors/remote_authorization.json` is what keeps the
/// two implementations from drifting.
library;

/// A command one device can ask another to carry out.
///
/// Closed rather than a string, so the exhaustive switches below stop compiling when a
/// fifth command is added — including [RemoteCommandX.isRemotelyPermitted], which is
/// where a new command would otherwise default to being accepted.
enum RemoteCommand {
  /// Schedule a power-off on the target. Reaches the machine as a job.
  powerOff,

  /// Schedule a keep-awake on the target. Also a job; the remote path never acquires the
  /// assertion itself.
  keepAwake,

  /// Cancel a scheduled job on the target, through its existing cancellation path.
  cancelJob,

  /// Turn on the target's remote-control setting. Never permitted remotely — see
  /// [RemoteCommandX.isRemotelyPermitted].
  enableRemoteControl,
}

extension RemoteCommandX on RemoteCommand {
  /// Whether this command is one a target accepts from another device at all.
  ///
  /// [RemoteCommand.enableRemoteControl] is not, and no combination of pairing or
  /// settings makes it so. Requiring physical presence to grant the permission is what
  /// keeps access to an account from being enough to turn remote control on everywhere
  /// and then power off every device on it.
  bool get isRemotelyPermitted => switch (this) {
        RemoteCommand.powerOff => true,
        RemoteCommand.keepAwake => true,
        RemoteCommand.cancelJob => true,
        RemoteCommand.enableRemoteControl => false,
      };

  /// Whether carrying this command out requires the target to be able to power off.
  ///
  /// Per command rather than a blanket gate: cancelling a job asks nothing of the
  /// shutdown path, so a target that cannot power off still accepts a cancellation.
  bool get requiresPowerOffCapability => switch (this) {
        RemoteCommand.powerOff => true,
        RemoteCommand.keepAwake => false,
        RemoteCommand.cancelJob => false,
        RemoteCommand.enableRemoteControl => false,
      };

  /// The name both implementations serialise. A cross-language contract, not an
  /// internal detail — these strings appear in the shared vectors.
  String get wireName => switch (this) {
        RemoteCommand.powerOff => 'powerOff',
        RemoteCommand.keepAwake => 'keepAwake',
        RemoteCommand.cancelJob => 'cancelJob',
        RemoteCommand.enableRemoteControl => 'enableRemoteControl',
      };
}

/// Why a remote command was refused.
///
/// Closed rather than a string so the shared vectors can name reasons exactly rather
/// than matching on prose.
enum DenialReason {
  /// The requesting device has not been paired with this one. Being signed in to the
  /// same account is not sufficient.
  notPaired,

  /// The target's owner has not turned remote control on. This is every device's
  /// starting state.
  remoteControlDisabled,

  /// The target cannot act as a remote target, or cannot perform this command.
  platformCannotPerform,

  /// No target accepts this command remotely, whatever else is true.
  commandNotRemotelyAllowed,
}

extension DenialReasonX on DenialReason {
  /// Prose for the person who asked.
  ///
  /// Specific per variant rather than a generic failure: a refusal the user cannot act
  /// on is indistinguishable from a bug. **These strings are byte-identical to the ones
  /// in `desktop/src-tauri/src/domain/remote_command.rs`** — a user who sees a refusal on
  /// their phone and again on their laptop must not be told two different things.
  String get userMessage => switch (this) {
        DenialReason.notPaired =>
          'This device is not paired with that one. Pair them from the device you '
              'want to control.',
        DenialReason.remoteControlDisabled =>
          'Remote control is turned off on the target device. It can only be '
              'turned on at that device.',
        DenialReason.platformCannotPerform =>
          'The target device cannot carry out that command. Phones and tablets '
              'cannot be controlled remotely.',
        DenialReason.commandNotRemotelyAllowed =>
          'That command cannot be sent from another device. It has to be done at '
              'the device itself.',
      };

  /// The name both implementations serialise.
  String get wireName => switch (this) {
        DenialReason.notPaired => 'notPaired',
        DenialReason.remoteControlDisabled => 'remoteControlDisabled',
        DenialReason.platformCannotPerform => 'platformCannotPerform',
        DenialReason.commandNotRemotelyAllowed => 'commandNotRemotelyAllowed',
      };
}

/// Accepted, or refused with exactly one reason.
///
/// A `(bool, DenialReason?)` pair would admit two nonsense states — allowed with a
/// reason, and denied without one — and something would eventually construct one.
/// Keeping the reason inside [DeniedDecision] makes both unrepresentable, the same
/// argument [CalendarDate]'s validating constructor makes for February 30th.
sealed class RemoteCommandDecision {
  const RemoteCommandDecision();

  bool get isAllowed => this is AllowedDecision;

  /// The refusal reason, or null when accepted.
  DenialReason? get denialReason =>
      this is DeniedDecision ? (this as DeniedDecision).reason : null;
}

/// The target accepts this command from this requester.
final class AllowedDecision extends RemoteCommandDecision {
  const AllowedDecision();

  @override
  bool operator ==(Object other) =>
      identical(this, other) || other is AllowedDecision;

  @override
  int get hashCode => runtimeType.hashCode;

  @override
  String toString() => 'Allowed';
}

/// The target refuses, for exactly one enumerated reason.
final class DeniedDecision extends RemoteCommandDecision {
  const DeniedDecision(this.reason);

  final DenialReason reason;

  /// The prose shown to the person who asked.
  String get userMessage => reason.userMessage;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      (other is DeniedDecision && other.reason == reason);

  @override
  int get hashCode => reason.hashCode;

  @override
  String toString() => 'Denied(${reason.wireName})';
}

/// Everything [authorizeRemoteCommand] needs, named.
///
/// A class rather than five positional booleans: a row of booleans is a call site nobody
/// can read and a transposition nobody catches, and a transposition here authorizes a
/// command that should be refused.
///
/// The capability *booleans* are carried rather than a `PlatformCapabilities`. This rule
/// is evaluated for a *remote* device, from data that arrived over the wire, and
/// `PlatformCapabilities.resolve()` describes the local machine. Passing the whole model
/// would invite calling `resolve()` inside the rule and silently authorizing against the
/// wrong device.
final class RemoteCommandContext {
  const RemoteCommandContext({
    required this.command,
    required this.targetCanPowerOff,
    required this.targetIsRemoteTarget,
    required this.isPaired,
    required this.remoteControlEnabled,
  });

  /// What is being asked for.
  final RemoteCommand command;

  /// Whether the *target* can shut itself down.
  final bool targetCanPowerOff;

  /// Whether the *target* can act as a remote-control target at all.
  final bool targetIsRemoteTarget;

  /// Whether the requesting device is paired with the target. Per device and revocable,
  /// not implied by both being on one account.
  final bool isPaired;

  /// Whether the target's owner has turned remote control on, at the target.
  final bool remoteControlEnabled;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      (other is RemoteCommandContext &&
          other.command == command &&
          other.targetCanPowerOff == targetCanPowerOff &&
          other.targetIsRemoteTarget == targetIsRemoteTarget &&
          other.isPaired == isPaired &&
          other.remoteControlEnabled == remoteControlEnabled);

  @override
  int get hashCode => Object.hash(
        command,
        targetCanPowerOff,
        targetIsRemoteTarget,
        isPaired,
        remoteControlEnabled,
      );

  @override
  String toString() => 'RemoteCommandContext(${command.wireName}, '
      'canPowerOff=$targetCanPowerOff, '
      'isRemoteTarget=$targetIsRemoteTarget, '
      'paired=$isPaired, '
      'remoteControlEnabled=$remoteControlEnabled)';
}

/// Decides whether the target accepts this command from this requester.
///
/// **The order of these checks is part of the contract**, and the shared vectors pin it:
/// commandNotRemotelyAllowed, then remoteControlDisabled, then notPaired, then
/// platformCannotPerform. Several reasons can hold at once, and an unspecified order
/// would let both implementations return "a" correct denial while disagreeing case by
/// case.
///
/// The order is not arbitrary: it discloses least. An unpaired requester learns only that
/// it is unpaired — not whether the target has remote control on, nor what the target is
/// capable of — so a caller cannot map an account's devices by reading refusal reasons.
/// Capability is checked last for exactly that reason.
RemoteCommandDecision authorizeRemoteCommand(RemoteCommandContext context) {
  if (!context.command.isRemotelyPermitted) {
    return const DeniedDecision(DenialReason.commandNotRemotelyAllowed);
  }

  if (!context.remoteControlEnabled) {
    return const DeniedDecision(DenialReason.remoteControlDisabled);
  }

  if (!context.isPaired) {
    return const DeniedDecision(DenialReason.notPaired);
  }

  if (!context.targetIsRemoteTarget ||
      (context.command.requiresPowerOffCapability &&
          !context.targetCanPowerOff)) {
    return const DeniedDecision(DenialReason.platformCannotPerform);
  }

  return const AllowedDecision();
}

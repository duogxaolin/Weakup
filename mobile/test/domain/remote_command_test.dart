// Unit tests for the remote command authorization rule.
//
// The shared vectors in `shared/testvectors/remote_authorization.json` prove this agrees
// with the Rust implementation, case for case. These prove the local rule: the precedence
// order, each reason on its own, and the messages.
//
// The structural half of the Rust side — the source-text test asserting no remote type
// reaches the countdown gate — has no mirror here, and needs none: this implementation
// holds no executor to reach. The mobile platform cannot power off at all.

import 'package:flutter_test/flutter_test.dart';
import 'package:weakup/domain/domain.dart';

void main() {
  /// A context that is authorized in every respect, so each test can spoil exactly one
  /// thing and attribute the refusal to it.
  RemoteCommandContext permissive(RemoteCommand command) => RemoteCommandContext(
        command: command,
        targetCanPowerOff: true,
        targetIsRemoteTarget: true,
        isPaired: true,
        remoteControlEnabled: true,
      );

  group('denial reason precedence', () {
    test('a command that is never remote is refused before anything else', () {
      // Everything else is permissive, so only the command itself can be the reason.
      expect(
        authorizeRemoteCommand(permissive(RemoteCommand.enableRemoteControl)),
        const DeniedDecision(DenialReason.commandNotRemotelyAllowed),
      );
    });

    test('the command check wins over every other reason at once', () {
      // All four reasons hold. The first in the order surfaces, so the refusal discloses
      // nothing about the target beyond the command being forbidden.
      const context = RemoteCommandContext(
        command: RemoteCommand.enableRemoteControl,
        targetCanPowerOff: false,
        targetIsRemoteTarget: false,
        isPaired: false,
        remoteControlEnabled: false,
      );

      expect(
        authorizeRemoteCommand(context),
        const DeniedDecision(DenialReason.commandNotRemotelyAllowed),
      );
    });

    test('a disabled setting is reported before a missing pairing', () {
      const context = RemoteCommandContext(
        command: RemoteCommand.powerOff,
        targetCanPowerOff: true,
        targetIsRemoteTarget: true,
        isPaired: false,
        remoteControlEnabled: false,
      );

      expect(
        authorizeRemoteCommand(context),
        const DeniedDecision(DenialReason.remoteControlDisabled),
      );
    });

    test('a missing pairing is reported before what the target can do', () {
      // Capability is checked last on purpose: an unpaired caller must not be able to
      // learn what the target is capable of by reading the refusal it gets back.
      const context = RemoteCommandContext(
        command: RemoteCommand.powerOff,
        targetCanPowerOff: false,
        targetIsRemoteTarget: false,
        isPaired: false,
        remoteControlEnabled: true,
      );

      expect(
        authorizeRemoteCommand(context),
        const DeniedDecision(DenialReason.notPaired),
      );
    });
  });

  group('each reason on its own', () {
    test('an unpaired requester is refused with the pairing reason', () {
      // Same account is not sufficient, and a revoked pairing lands here too: the rule
      // takes "is this device paired" as an input rather than performing a lookup, so
      // revocation needs no separate branch.
      const context = RemoteCommandContext(
        command: RemoteCommand.powerOff,
        targetCanPowerOff: true,
        targetIsRemoteTarget: true,
        isPaired: false,
        remoteControlEnabled: true,
      );

      expect(
        authorizeRemoteCommand(context),
        const DeniedDecision(DenialReason.notPaired),
      );
    });

    test('remote control being off is its own reason', () {
      const context = RemoteCommandContext(
        command: RemoteCommand.powerOff,
        targetCanPowerOff: true,
        targetIsRemoteTarget: true,
        isPaired: true,
        remoteControlEnabled: false,
      );

      expect(
        authorizeRemoteCommand(context),
        const DeniedDecision(DenialReason.remoteControlDisabled),
      );
    });

    test('a target that cannot power off is refused for capability not permission', () {
      // The distinction the spec insists on: "this machine cannot do that" must not be
      // reported as "you are not allowed to", or the user goes looking for a setting
      // that would not help.
      const context = RemoteCommandContext(
        command: RemoteCommand.powerOff,
        targetCanPowerOff: false,
        targetIsRemoteTarget: true,
        isPaired: true,
        remoteControlEnabled: true,
      );

      expect(
        authorizeRemoteCommand(context),
        const DeniedDecision(DenialReason.platformCannotPerform),
      );
    });

    test('a device that cannot be a remote target refuses every command', () {
      // A phone. The OS suspends the app, so a command sent to it could not be received
      // reliably — refused explicitly rather than accepted and silently dropped.
      for (final command in [
        RemoteCommand.powerOff,
        RemoteCommand.keepAwake,
        RemoteCommand.cancelJob,
      ]) {
        expect(
          authorizeRemoteCommand(
            RemoteCommandContext(
              command: command,
              targetCanPowerOff: false,
              targetIsRemoteTarget: false,
              isPaired: true,
              remoteControlEnabled: true,
            ),
          ),
          const DeniedDecision(DenialReason.platformCannotPerform),
          reason: '$command must be refused by a device that cannot be a target',
        );
      }
    });
  });

  group('what is accepted', () {
    test('a paired, enabled, capable desktop accepts every permitted command', () {
      for (final command in [
        RemoteCommand.powerOff,
        RemoteCommand.keepAwake,
        RemoteCommand.cancelJob,
      ]) {
        expect(
          authorizeRemoteCommand(permissive(command)),
          const AllowedDecision(),
          reason: '$command should be accepted',
        );
      }
    });

    test('the capability check is per command rather than a blanket gate', () {
      // Cancelling asks nothing of the shutdown path, so a target that cannot power off
      // still accepts it. A single targetCanPowerOff gate over all commands would refuse
      // this and pass every other test in this file.
      const context = RemoteCommandContext(
        command: RemoteCommand.cancelJob,
        targetCanPowerOff: false,
        targetIsRemoteTarget: true,
        isPaired: true,
        remoteControlEnabled: true,
      );

      expect(authorizeRemoteCommand(context), const AllowedDecision());
      expect(RemoteCommand.cancelJob.requiresPowerOffCapability, isFalse);
      expect(RemoteCommand.keepAwake.requiresPowerOffCapability, isFalse);
      expect(RemoteCommand.powerOff.requiresPowerOffCapability, isTrue);
    });

    test('enabling remote control is refused however permissive everything else is', () {
      // The spec's reason, restated as a test: without this, access to one account would
      // be enough to turn remote control on everywhere and then power off every device
      // on it. Physical presence is the thing account access cannot forge.
      expect(RemoteCommand.enableRemoteControl.isRemotelyPermitted, isFalse);
      expect(
        authorizeRemoteCommand(permissive(RemoteCommand.enableRemoteControl)).isAllowed,
        isFalse,
      );
    });
  });

  group('the decision type and its messages', () {
    test('a decision carries a reason exactly when it is a refusal', () {
      // The nonsense states a (bool, reason?) pair would admit, asserted absent.
      expect(const AllowedDecision().denialReason, isNull);
      expect(const AllowedDecision().isAllowed, isTrue);

      const denied = DeniedDecision(DenialReason.notPaired);
      expect(denied.denialReason, DenialReason.notPaired);
      expect(denied.isAllowed, isFalse);
    });

    test('every denial reason has a distinct message a person can act on', () {
      // A generic "refused" is indistinguishable from a bug, and two reasons sharing a
      // message would make them indistinguishable from each other.
      final messages = DenialReason.values.map((r) => r.userMessage).toList();

      for (final message in messages) {
        expect(message.trim(), isNotEmpty);
      }
      expect(messages.toSet().length, messages.length,
          reason: 'two reasons share a message');
    });

    test('the disabled message says where the setting can be turned on', () {
      // Naming the target device is the whole point: a user told only "remote control is
      // off" will look for the switch on the phone in their hand, where it is not.
      expect(
        DenialReason.remoteControlDisabled.userMessage,
        contains('turned on at that device'),
      );
    });

    test('reason and command wire names match the shared vectors', () {
      // These exact strings appear in the shared vectors, so they are a cross-language
      // contract rather than an internal detail.
      expect(DenialReason.notPaired.wireName, 'notPaired');
      expect(DenialReason.remoteControlDisabled.wireName, 'remoteControlDisabled');
      expect(DenialReason.platformCannotPerform.wireName, 'platformCannotPerform');
      expect(
        DenialReason.commandNotRemotelyAllowed.wireName,
        'commandNotRemotelyAllowed',
      );

      expect(RemoteCommand.powerOff.wireName, 'powerOff');
      expect(RemoteCommand.keepAwake.wireName, 'keepAwake');
      expect(RemoteCommand.cancelJob.wireName, 'cancelJob');
      expect(RemoteCommand.enableRemoteControl.wireName, 'enableRemoteControl');
    });
  });

  group('JobOrigin', () {
    test('a job is local unless it says otherwise', () {
      // The default is what every construction site already meant, so adding the field
      // changed no existing behaviour.
      final job = Job(
        id: 1,
        type: JobType.powerOff,
        trigger: const DurationTrigger(minutes: 30),
        status: JobStatus.active,
        createdAt: DateTime.utc(2026, 7, 30, 10),
        updatedAt: DateTime.utc(2026, 7, 30, 10),
      );

      expect(job.origin, JobOrigin.local);
    });

    test('the origin survives copyWith and can be set to remote', () {
      // Documents the D7 gap by code: the origin exists in memory and is carried
      // through the model, but is not persisted in this change. A remote job that
      // survives a restart would read back as local and get the *shorter* countdown —
      // which is why persisting it is a blocker for the change that adds a transport,
      // not a follow-up to it.
      final local = Job(
        id: 1,
        type: JobType.powerOff,
        trigger: const DurationTrigger(minutes: 30),
        status: JobStatus.active,
        createdAt: DateTime.utc(2026, 7, 30, 10),
        updatedAt: DateTime.utc(2026, 7, 30, 10),
      );

      expect(local.copyWith(origin: JobOrigin.remote).origin, JobOrigin.remote);
      expect(local.copyWith(status: JobStatus.paused).origin, JobOrigin.local);
    });

    test('the origin names match the desktop wire format', () {
      expect(JobOrigin.local.name, 'local');
      expect(JobOrigin.remote.name, 'remote');
    });
  });
}

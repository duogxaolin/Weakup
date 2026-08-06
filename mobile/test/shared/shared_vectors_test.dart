// Executes the language-agnostic vectors in `shared/testvectors/` against the
// Dart implementation.
//
// The desktop (Rust) implementation runs the very same files from
// `desktop/src-tauri/tests/shared_vectors.rs`. That is the whole point: the two
// implementations share ~900 lines of scheduling rules but no code, so a rule
// fixed in one language and forgotten in the other would leave both suites
// green. These files are the contract that makes such drift fail loudly.
//
// Adding a rule to either implementation means adding a case here.

import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:timezone/data/latest.dart' as tz_data;
import 'package:timezone/timezone.dart' as tz;
import 'package:weakup/core/core.dart';
import 'package:weakup/domain/domain.dart';

/// Resolved relative to the package root, which is the working directory for
/// `flutter test`. Kept as a single constant so a repo re-layout fails in one
/// obvious place instead of twelve.
final Directory vectorDir = Directory('../shared/testvectors');

Map<String, dynamic> loadVectorSet(String fileName) {
  final file = File('${vectorDir.path}/$fileName');
  if (!file.existsSync()) {
    fail(
      'Shared vector file not found: ${file.absolute.path}\n'
      'The Dart and Rust suites must read the same files; a missing file means '
      'this suite is silently testing nothing.',
    );
  }
  return jsonDecode(file.readAsStringSync()) as Map<String, dynamic>;
}

List<Map<String, dynamic>> casesOf(String fileName) {
  final set = loadVectorSet(fileName);
  final cases = (set['cases'] as List).cast<Map<String, dynamic>>();
  expect(cases, isNotEmpty, reason: '$fileName declares no cases');
  return cases;
}

/// Parses the `trigger` object. The `kind` names are the wire format both
/// implementations agree on.
TriggerSpec parseTrigger(Map<String, dynamic> json) {
  return switch (json['kind'] as String) {
    'indefinite' => const IndefiniteTrigger(),
    'duration' => DurationTrigger(minutes: json['minutes'] as int),
    'absoluteTime' => AbsoluteTimeTrigger(
        hour: json['hour'] as int,
        minute: json['minute'] as int,
        // Optional. Absent means a time of day resolved to its next occurrence;
        // present means a one-off instant on exactly that date.
        date: switch (json['date'] as String?) {
          null => null,
          final raw => CalendarDate.parse(raw),
        },
      ),
    final unknown => fail('Unknown trigger kind in vector: $unknown'),
  };
}

JobType parseJobType(String raw) => switch (raw) {
      'keepAwake' => JobType.keepAwake,
      'powerOff' => JobType.powerOff,
      final unknown => fail('Unknown jobType in vector: $unknown'),
    };

ReconcileOutcome parseOutcome(String raw) => switch (raw) {
      'stillPending' => ReconcileOutcome.stillPending,
      'completed' => ReconcileOutcome.completed,
      'proceedToGracePeriod' => ReconcileOutcome.proceedToGracePeriod,
      'overdue' => ReconcileOutcome.overdue,
      final unknown => fail('Unknown expectedOutcome in vector: $unknown'),
    };

/// The three presence states, by the names both implementations serialise.
PresenceState parsePresenceState(String raw) => switch (raw) {
      'online' => PresenceState.online,
      'stale' => PresenceState.stale,
      'offline' => PresenceState.offline,
      final unknown => fail('Unknown expectedState in vector: $unknown'),
    };

RemoteCommand parseRemoteCommand(String raw) => switch (raw) {
      'powerOff' => RemoteCommand.powerOff,
      'keepAwake' => RemoteCommand.keepAwake,
      'cancelJob' => RemoteCommand.cancelJob,
      'enableRemoteControl' => RemoteCommand.enableRemoteControl,
      final unknown => fail('Unknown command in vector: $unknown'),
    };

DenialReason parseDenialReason(String raw) => switch (raw) {
      'notPaired' => DenialReason.notPaired,
      'remoteControlDisabled' => DenialReason.remoteControlDisabled,
      'platformCannotPerform' => DenialReason.platformCannotPerform,
      'commandNotRemotelyAllowed' => DenialReason.commandNotRemotelyAllowed,
      final unknown => fail('Unknown expectedReason in vector: $unknown'),
    };

/// The five closed acceptance reasons, by the names both implementations serialise.
RejectionReason parseRejectionReason(String raw) => switch (raw) {
      'authenticityUnverified' => RejectionReason.authenticityUnverified,
      'replayedNonce' => RejectionReason.replayedNonce,
      'futureDated' => RejectionReason.futureDated,
      'stale' => RejectionReason.stale,
      'notPermitted' => RejectionReason.notPermitted,
      final unknown => fail('Unknown expectedReason in vector: $unknown'),
    };

GrantDelivery parseGrantDelivery(String raw) => switch (raw) {
      'atMachine' => GrantDelivery.atMachine,
      'outOfBand' => GrantDelivery.outOfBand,
      final unknown => fail('Unknown delivery in vector: $unknown'),
    };

GrantRejection parseGrantRejection(String raw) => switch (raw) {
      'expired' => GrantRejection.expired,
      'alreadyUsed' => GrantRejection.alreadyUsed,
      'noSuchGrant' => GrantRejection.noSuchGrant,
      final unknown => fail('Unknown expectedReason in vector: $unknown'),
    };

/// The substring the vectors match against. Vectors compare on substance, not
/// wording, so the two implementations may phrase a message differently.
String errorText(AppError error) =>
    error is ValidationError ? error.message : error.toString();

DateTime parseUtc(String raw) => DateTime.parse(raw).toUtc();

void main() {
  setUpAll(tz_data.initializeTimeZones);

  test('shared resolution vectors all match', () {
    final failures = <String>[];

    for (final c in casesOf('resolution.json')) {
      final id = c['id'] as String;
      final location = tz.getLocation(c['timezone'] as String);
      final now = parseUtc(c['now'] as String);
      final needle = c['expectedErrorContains'] as String?;
      // Absent means "expects a rejection"; present-but-null means "this trigger has
      // no instant by design". `containsKey` is what tells those apart — reading the
      // value alone gives null for both.
      final expectsInstant = c.containsKey('expectedTargetInstantUtc');

      if (expectsInstant == (needle != null)) {
        failures.add(
          '$id: must set exactly one of expectedTargetInstantUtc and '
          'expectedErrorContains',
        );
        continue;
      }

      final result = TriggerResolver.resolve(
        parseTrigger(c['trigger'] as Map<String, dynamic>),
        location,
        now: now,
      );

      if (needle != null) {
        if (result.isSuccess) {
          failures.add('$id: expected a rejection, got ${result.valueOrNull}  '
              '(${c['description']})');
          continue;
        }
        final text = errorText((result as Failure).error);
        if (!text.contains(needle)) {
          failures.add('$id: message "$text" does not contain "$needle"');
        }
        continue;
      }

      if (result.isFailure) {
        failures.add('$id: resolve failed: ${errorText((result as Failure).error)}  '
            '(${c['description']})');
        continue;
      }

      final expectedRaw = c['expectedTargetInstantUtc'] as String?;
      final expected = expectedRaw == null ? null : parseUtc(expectedRaw);
      final actual = result.valueOrNull?.toUtc();

      if (actual != expected) {
        failures.add('$id: expected $expected, got $actual  (${c['description']})');
      }
    }

    expect(failures, isEmpty, reason: failures.join('\n'));
  });

  test('shared reconciliation vectors all match', () {
    final failures = <String>[];

    for (final c in casesOf('reconciliation.json')) {
      final id = c['id'] as String;
      final targetRaw = c['targetInstantUtc'] as String?;

      final actual = TriggerResolver.reconcile(
        jobType: parseJobType(c['jobType'] as String),
        targetInstantUtc: targetRaw == null ? null : parseUtc(targetRaw),
        now: parseUtc(c['now'] as String),
      );
      final expected = parseOutcome(c['expectedOutcome'] as String);

      if (actual != expected) {
        failures.add('$id: expected $expected, got $actual  (${c['description']})');
      }
    }

    expect(failures, isEmpty, reason: failures.join('\n'));
  });

  test('shared validation vectors all match', () {
    final failures = <String>[];

    for (final c in casesOf('validation.json')) {
      final id = c['id'] as String;
      final expectedValid = c['expectedValid'] as bool;

      final result = TriggerResolver.validate(
        parseTrigger(c['trigger'] as Map<String, dynamic>),
        parseJobType(c['jobType'] as String),
      );

      if (result.isSuccess != expectedValid) {
        failures.add(
          '$id: expected valid=$expectedValid, got valid=${result.isSuccess}  '
          '(${c['description']})',
        );
        continue;
      }

      final needle = c['expectedErrorContains'] as String?;
      if (needle != null) {
        final text = errorText((result as Failure).error);
        if (!text.contains(needle)) {
          failures.add('$id: message "$text" does not contain "$needle"');
        }
      }
    }

    expect(failures, isEmpty, reason: failures.join('\n'));
  });

  test('no shared vector expects an immediate power-off', () {
    // The 60-second grace countdown is mandatory and non-skippable. A vector
    // asserting a direct power-off would legitimise a path around it, so the
    // strongest guard is that no such vector can exist.
    for (final c in casesOf('reconciliation.json')) {
      expect(
        c['expectedOutcome'],
        isNot('powerOffNow'),
        reason: '${c['id']} must route through proceedToGracePeriod',
      );
    }
  });

  test('shared presence vectors all match', () {
    final failures = <String>[];

    for (final c in casesOf('presence.json')) {
      final id = c['id'] as String;

      // `containsKey` rather than a null read: an absent key means the case forgot to
      // say when the device last reported, which is a broken case rather than a device
      // that has never reported. The same distinction the resolution runner draws.
      if (!c.containsKey('lastSeen')) {
        failures.add('$id: does not set lastSeen (use null for never reported)');
        continue;
      }
      if (!c.containsKey('expectedElapsedSeconds')) {
        failures.add('$id: does not set expectedElapsedSeconds');
        continue;
      }

      final lastSeenRaw = c['lastSeen'] as String?;
      final expectedElapsed = c['expectedElapsedSeconds'] as int?;

      // A case claiming an age for a device that never reported, or no age for one that
      // did, would pass against an implementation that got the other half wrong.
      if ((lastSeenRaw == null) != (expectedElapsed == null)) {
        failures.add(
          '$id: a device with no lastSeen has no elapsed time, and one with a '
          'lastSeen always has one',
        );
        continue;
      }

      final expectedState = parsePresenceState(c['expectedState'] as String);
      if (lastSeenRaw == null && expectedState != PresenceState.offline) {
        failures.add('$id: a device that never reported can only be offline');
        continue;
      }

      final actual = evaluatePresence(
        lastSeen: lastSeenRaw == null ? null : parseUtc(lastSeenRaw),
        now: parseUtc(c['now'] as String),
      );

      if (actual.state != expectedState) {
        failures.add('$id: expected $expectedState, got ${actual.state}  '
            '(${c['description']})');
        continue;
      }

      final actualElapsed = actual.elapsed?.inSeconds;
      if (actualElapsed != expectedElapsed) {
        failures.add('$id: expected elapsed $expectedElapsed, got $actualElapsed  '
            '(${c['description']})');
      }
    }

    expect(failures, isEmpty, reason: failures.join('\n'));
  });

  test('presence vectors cover both boundaries and both clock-skew cases', () {
    // The four cases a refactor is most likely to drop, because each looks redundant
    // beside its neighbour until the comparison operator changes.
    final ids = casesOf('presence.json').map((c) => c['id'] as String).toSet();
    expect(ids, contains('online-exactly-at-threshold'));
    expect(ids, contains('stale-exactly-at-offline-threshold'));
    expect(ids, contains('online-last-seen-in-the-future-clamps-to-zero'));
    expect(ids, contains('online-last-seen-far-in-the-future-clamps-to-zero'));
  });

  test('shared remote authorization vectors all match', () {
    final failures = <String>[];

    for (final c in casesOf('remote_authorization.json')) {
      final id = c['id'] as String;
      final expectedDecision = c['expectedDecision'] as String;
      final expectedReasonRaw = c['expectedReason'] as String?;

      if (expectedDecision == 'allowed' && expectedReasonRaw != null) {
        failures.add('$id: is allowed but names a refusal reason; a decision has '
            'one or the other');
        continue;
      }
      if (expectedDecision == 'denied' && expectedReasonRaw == null) {
        failures.add('$id: is denied but names no reason; every refusal is explainable');
        continue;
      }
      if (expectedDecision != 'allowed' && expectedDecision != 'denied') {
        failures.add('$id: unknown expectedDecision "$expectedDecision"');
        continue;
      }

      final actual = authorizeRemoteCommand(
        RemoteCommandContext(
          command: parseRemoteCommand(c['command'] as String),
          targetCanPowerOff: c['targetCanPowerOff'] as bool,
          targetIsRemoteTarget: c['targetIsRemoteTarget'] as bool,
          isPaired: c['isPaired'] as bool,
          remoteControlEnabled: c['remoteControlEnabled'] as bool,
        ),
      );

      if (expectedReasonRaw == null) {
        if (actual is! AllowedDecision) {
          failures.add('$id: expected allowed, got $actual  (${c['description']})');
        }
        continue;
      }

      // The reason is asserted, not merely the refusal: a denial for the wrong reason is
      // the drift these vectors exist to catch, and "it was denied" would pass against an
      // implementation with the precedence order reversed.
      final expectedReason = parseDenialReason(expectedReasonRaw);
      if (actual is! DeniedDecision) {
        failures.add('$id: expected denied($expectedReason), got $actual  '
            '(${c['description']})');
        continue;
      }
      if (actual.reason != expectedReason) {
        failures.add('$id: expected reason $expectedReason, got ${actual.reason}  '
            '(${c['description']})');
      }
    }

    expect(failures, isEmpty, reason: failures.join('\n'));
  });

  test('every remote authorization case names all five context fields', () {
    // A missing boolean would read as null and throw, or default in a laxer harness and
    // silently change what the case tests — and half of these fields, defaulted the wrong
    // way, authorize a command that should be refused.
    for (final c in casesOf('remote_authorization.json')) {
      final id = c['id'] as String;
      for (final field in [
        'command',
        'targetCanPowerOff',
        'targetIsRemoteTarget',
        'isPaired',
        'remoteControlEnabled',
      ]) {
        expect(
          c.containsKey(field),
          isTrue,
          reason: '$id omits the required context field $field',
        );
      }
    }
  });

  test('remote authorization vectors pin the denial precedence order', () {
    // Each of these sets up two or more simultaneous reasons and names the one that must
    // surface. Without them the vectors would agree with an implementation that checks in
    // any order, and the order is what keeps a refusal from disclosing the target's state.
    final cases = casesOf('remote_authorization.json');
    final ids = cases.map((c) => c['id'] as String).toSet();
    expect(ids, contains('precedence-not-remotely-allowed-beats-remote-control-disabled'));
    expect(ids, contains('precedence-not-remotely-allowed-beats-not-paired'));
    expect(ids, contains('precedence-remote-control-disabled-beats-not-paired'));
    expect(ids, contains('precedence-not-paired-beats-platform-cannot-perform'));
    expect(ids, contains('precedence-all-four-reasons-hold-at-once'));

    // Every reason must also appear on its own, or a reason could be dropped from the
    // implementation entirely and only the precedence cases would notice.
    final reasons = cases
        .map((c) => c['expectedReason'] as String?)
        .whereType<String>()
        .toSet();
    expect(reasons, contains('commandNotRemotelyAllowed'));
    expect(reasons, contains('remoteControlDisabled'));
    expect(reasons, contains('notPaired'));
    expect(reasons, contains('platformCannotPerform'));
  });

  test('shared command acceptance vectors all match', () {
    final failures = <String>[];

    for (final c in casesOf('command_acceptance.json')) {
      final id = c['id'] as String;
      final expectedDecision = c['expectedDecision'] as String;
      final expectedReasonRaw = c['expectedReason'] as String?;

      if (expectedDecision == 'accepted' && expectedReasonRaw != null) {
        failures.add('$id: is accepted but names a refusal reason; a decision has '
            'one or the other');
        continue;
      }
      if (expectedDecision == 'rejected' && expectedReasonRaw == null) {
        failures.add('$id: is rejected but names no reason; every refusal is explainable');
        continue;
      }
      if (expectedDecision != 'accepted' && expectedDecision != 'rejected') {
        failures.add('$id: unknown expectedDecision "$expectedDecision"');
        continue;
      }

      final actual = evaluateCommand(
        envelope: CommandEnvelope(
          sender: DeviceId(c['senderDeviceId'] as String),
          command: parseRemoteCommand(c['command'] as String),
          createdAt: parseUtc(c['createdAt'] as String),
          nonce: c['nonce'] as String,
          signatureVerified: c['signatureVerified'] as bool,
        ),
        targetState: CommandTargetState(
          seenNonces: (c['seenNonces'] as List).cast<String>().toSet(),
          targetCanPowerOff: c['targetCanPowerOff'] as bool,
          targetIsRemoteTarget: c['targetIsRemoteTarget'] as bool,
          isPaired: c['isPaired'] as bool,
          remoteControlEnabled: c['remoteControlEnabled'] as bool,
        ),
        now: parseUtc(c['now'] as String),
      );

      if (expectedReasonRaw == null) {
        if (actual is! AcceptedCommand) {
          failures.add('$id: expected accepted, got $actual  (${c['description']})');
        }
        continue;
      }

      // The reason is asserted, not merely the refusal: a rejection for the wrong reason is
      // the drift these vectors exist to catch, and "it was rejected" would pass against an
      // implementation with the precedence order reversed.
      final expectedReason = parseRejectionReason(expectedReasonRaw);
      if (actual is! RejectedCommand) {
        failures.add('$id: expected rejected($expectedReason), got $actual  '
            '(${c['description']})');
        continue;
      }
      if (actual.reason != expectedReason) {
        failures.add('$id: expected reason $expectedReason, got ${actual.reason}  '
            '(${c['description']})');
      }
    }

    expect(failures, isEmpty, reason: failures.join('\n'));
  });

  test('every command acceptance case names every envelope and target field', () {
    // A missing field would read as null and throw, or default in a laxer harness and
    // silently change what the case tests. `signatureVerified` defaulted to true would
    // accept a forged command, which is the whole property this file exists to pin.
    for (final c in casesOf('command_acceptance.json')) {
      final id = c['id'] as String;
      for (final field in [
        'senderDeviceId',
        'command',
        'createdAt',
        'nonce',
        'signatureVerified',
        'now',
        'seenNonces',
        'targetCanPowerOff',
        'targetIsRemoteTarget',
        'isPaired',
        'remoteControlEnabled',
      ]) {
        expect(
          c.containsKey(field),
          isTrue,
          reason: '$id omits the required field $field',
        );
      }
    }
  });

  test('command acceptance vectors pin every boundary and the precedence order', () {
    // The cases a refactor is most likely to drop, because each looks redundant beside its
    // neighbour until a comparison operator changes or a check is reordered.
    final cases = casesOf('command_acceptance.json');
    final ids = cases.map((c) => c['id'] as String).toSet();

    // Both halves of both time boundaries. Losing either half of a pair leaves the boundary
    // decided by whichever operator an implementation happens to use.
    expect(ids, contains('fresh-exactly-at-window'));
    expect(ids, contains('stale-one-second-past-window'));
    expect(ids, contains('future-exactly-at-tolerance'));
    expect(ids, contains('future-one-second-past-tolerance'));
    // Replay, and the case proving a device is not banned after its first command.
    expect(ids, contains('replay-of-seen-nonce'));
    expect(ids, contains('distinct-nonce-same-device-accepted'));
    // Every adjacent pair in the precedence order, plus the all-at-once case.
    expect(ids, contains('precedence-authenticity-beats-replay'));
    expect(ids, contains('precedence-authenticity-beats-not-permitted'));
    expect(ids, contains('precedence-replay-beats-future-dated'));
    expect(ids, contains('precedence-replay-beats-stale'));
    expect(ids, contains('precedence-future-dated-beats-not-permitted'));
    expect(ids, contains('precedence-stale-beats-not-permitted'));
    expect(ids, contains('precedence-all-five-reasons-hold-at-once'));

    // Every reason must also appear on its own, or a reason could be dropped from the
    // implementation entirely and only the precedence cases would notice.
    final reasons = cases
        .map((c) => c['expectedReason'] as String?)
        .whereType<String>()
        .toSet();
    expect(reasons, contains('authenticityUnverified'));
    expect(reasons, contains('replayedNonce'));
    expect(reasons, contains('futureDated'));
    expect(reasons, contains('stale'));
    expect(reasons, contains('notPermitted'));

    // One accepted case per remotely-permitted command, so a rule that accepted only
    // power-off could not pass.
    final accepted = cases
        .where((c) => c['expectedDecision'] == 'accepted')
        .map((c) => c['command'] as String)
        .toSet();
    expect(accepted, contains('powerOff'));
    expect(accepted, contains('keepAwake'));
    expect(accepted, contains('cancelJob'));
  });

  test('shared pairing grant vectors all match', () {
    final failures = <String>[];

    for (final c in casesOf('pairing_grant.json')) {
      final id = c['id'] as String;
      final expectedValidity = c['expectedValidity'] as String;
      final expectedReasonRaw = c['expectedReason'] as String?;

      if (expectedValidity == 'valid' && expectedReasonRaw != null) {
        failures.add('$id: is valid but names a refusal reason; a decision has '
            'one or the other');
        continue;
      }
      if (expectedValidity == 'invalid' && expectedReasonRaw == null) {
        failures.add('$id: is invalid but names no reason; every refusal is explainable');
        continue;
      }
      if (expectedValidity != 'valid' && expectedValidity != 'invalid') {
        failures.add('$id: unknown expectedValidity "$expectedValidity"');
        continue;
      }

      final actual = evaluateGrant(
        grant: PairingGrant(
          delivery: parseGrantDelivery(c['delivery'] as String),
          issuedAt: parseUtc(c['issuedAt'] as String),
          redeemed: c['redeemed'] as bool,
          recognised: c['recognised'] as bool,
        ),
        now: parseUtc(c['now'] as String),
      );

      if (expectedReasonRaw == null) {
        if (actual is! ValidGrant) {
          failures.add('$id: expected valid, got $actual  (${c['description']})');
        }
        continue;
      }

      final expectedReason = parseGrantRejection(expectedReasonRaw);
      if (actual is! InvalidGrant) {
        failures.add('$id: expected invalid($expectedReason), got $actual  '
            '(${c['description']})');
        continue;
      }
      if (actual.reason != expectedReason) {
        failures.add('$id: expected reason $expectedReason, got ${actual.reason}  '
            '(${c['description']})');
      }
    }

    expect(failures, isEmpty, reason: failures.join('\n'));
  });

  test('pairing grant vectors pin both lifetimes and the delivery split', () {
    // Both exact-lifetime cases and both halves of the splits-by-delivery pair. The pair is
    // the only thing proving the two lifetimes are actually distinct: an implementation
    // using one lifetime for both passes every other case in the file.
    final cases = casesOf('pairing_grant.json');
    final ids = cases.map((c) => c['id'] as String).toSet();
    expect(ids, contains('at-machine-exactly-at-lifetime'));
    expect(ids, contains('at-machine-one-second-past'));
    expect(ids, contains('out-of-band-exactly-at-lifetime'));
    expect(ids, contains('out-of-band-one-second-past'));
    expect(ids, contains('age-between-the-two-lifetimes-splits-by-delivery-at-machine'));
    expect(ids, contains('age-between-the-two-lifetimes-splits-by-delivery-out-of-band'));
    expect(ids, contains('already-redeemed-before-expiry'));
    expect(ids, contains('already-redeemed-after-expiry'));
    expect(ids, contains('unrecognised-grant'));

    final reasons = cases
        .map((c) => c['expectedReason'] as String?)
        .whereType<String>()
        .toSet();
    expect(reasons, contains('expired'));
    expect(reasons, contains('alreadyUsed'));
    expect(reasons, contains('noSuchGrant'));
  });

  test('every vector case has a unique id and a description', () {
    for (final file in [
      'resolution.json',
      'reconciliation.json',
      'validation.json',
      'presence.json',
      'remote_authorization.json',
      'command_acceptance.json',
      'pairing_grant.json',
    ]) {
      final seen = <String>{};
      for (final c in casesOf(file)) {
        final id = c['id'] as String;
        expect(seen.add(id), isTrue, reason: 'duplicate id "$id" in $file');
        expect(c['description'], isNotNull, reason: '$id in $file lacks a description');
      }
    }
  });

  test('vectors cover both DST anomalies and the tolerance boundary', () {
    // Guards against the vectors being quietly trimmed down to the easy cases,
    // which would leave drift-detection intact in name only.
    final resolutionIds =
        casesOf('resolution.json').map((c) => c['id'] as String).toSet();
    expect(resolutionIds, contains('absolute-spring-forward-gap'));
    expect(resolutionIds, contains('absolute-fall-back-overlap-takes-earlier'));
    // Supplying a date must not open a second DST-resolution path.
    expect(resolutionIds, contains('absolute-dated-spring-forward-gap'));
    expect(resolutionIds, contains('absolute-dated-fall-back-overlap-takes-earlier'));
    // The pair that pins the dated and undated semantics apart; losing either half
    // leaves the difference untested.
    expect(resolutionIds, contains('absolute-past-rolls-to-tomorrow'));
    expect(resolutionIds, contains('absolute-dated-does-not-roll-forward'));

    final outcomes = casesOf('reconciliation.json')
        .map((c) => c['expectedOutcome'] as String)
        .toSet();
    expect(outcomes, contains('proceedToGracePeriod'));
    expect(outcomes, contains('overdue'));
  });
}

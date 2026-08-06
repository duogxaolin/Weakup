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
import 'package:weakup/data/pairing_store.dart';
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

/// Lowercase hex, the form `signing_payload.json` states its expected bytes in.
String hexEncode(List<int> bytes) =>
    bytes.map((b) => b.toRadixString(16).padLeft(2, '0')).join();

/// Parses lowercase hex back into bytes.
List<int> hexDecode(String hex) => [
      for (var i = 0; i < hex.length; i += 2)
        int.parse(hex.substring(i, i + 2), radix: 16),
    ];

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

  test('shared command acceptance vectors all match', () async {
    final failures = <String>[];

    // The verifying key each case's target holds, declared once at the top of the file.
    // Registered under whichever sender the case names, so that holding a key and being
    // *paired* stay separate questions — `rejected-not-permitted-unpaired-sender` carries a
    // signature that verifies and is still refused, on permission grounds.
    final keySet = loadVectorSet('command_acceptance.json');
    final key = VerifyingKey.fromBytes(hexDecode(keySet['verifyingKeyHex'] as String));
    expect(key, isNotNull, reason: 'verifyingKeyHex must be a valid Ed25519 public key');

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

      final sender = DeviceId(c['senderDeviceId'] as String);
      final actual = await evaluateCommand(
        envelope: CommandEnvelope(
          sender: sender,
          command: parseRemoteCommand(c['command'] as String),
          createdAt: parseUtc(c['createdAt'] as String),
          nonce: c['nonce'] as String,
          signature: hexDecode(c['signature'] as String),
        ),
        targetState: CommandTargetState(
          seenNonces: (c['seenNonces'] as List).cast<String>().toSet(),
          verifyingKeys: {sender: key!},
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
    // silently change what the case tests. An omitted `signature` would be an empty one,
    // which fails verification — so an accepted case would flip silently.
    for (final c in casesOf('command_acceptance.json')) {
      final id = c['id'] as String;
      for (final field in [
        'senderDeviceId',
        'command',
        'createdAt',
        'nonce',
        'signature',
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

      // The field this change removed. Its presence anywhere would mean a case can once
      // again assert authenticity instead of demonstrating it.
      expect(
        c.containsKey('signatureVerified'),
        isFalse,
        reason: '$id carries signatureVerified, which no longer exists: authenticity is '
            'verified from the signature, never asserted',
      );
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

  test('shared signing payload vectors all match', () {
    // Unlike every other case type here, these pin an *encoding* rather than a decision.
    // Both implementations can agree a signature is invalid while disagreeing about what
    // bytes a valid one covers, and that only surfaces when a phone and a desktop are
    // paired.
    final failures = <String>[];

    for (final c in casesOf('signing_payload.json')) {
      final id = c['id'] as String;

      final actual = encodeSigningPayload(
        sender: DeviceId(c['sender'] as String),
        command: c['command'] as String,
        createdAt: DateTime.fromMillisecondsSinceEpoch(
          c['createdAtMillis'] as int,
          isUtc: true,
        ),
        nonce: c['nonce'] as String,
      );

      final actualHex = hexEncode(actual);
      final expectedHex = c['expectedBytesHex'] as String;
      if (actualHex != expectedHex) {
        failures.add('$id: expected $expectedHex, got $actualHex  '
            '(${c['description']})');
      }
    }

    expect(failures, isEmpty, reason: failures.join('\n'));
  });

  test('signing payload vectors pin the unambiguous boundary and the delimiter case', () {
    final cases = casesOf('signing_payload.json');
    final ids = cases.map((c) => c['id'] as String).toSet();

    // The pair that proves distinct field values cannot encode identically.
    expect(ids, contains('boundary-ambiguity-sender-ab-nonce-c'));
    expect(ids, contains('boundary-ambiguity-sender-a-nonce-bc'));
    // A nonce full of the delimiters a naive encoder would have joined fields with.
    expect(ids, contains('nonce-contains-naive-delimiters'));
    // The length prefix counts bytes, not characters.
    expect(ids, contains('sender-multibyte-utf8'));
    // The instant is unpadded decimal millis.
    expect(ids, contains('created-at-epoch-zero'));

    // The whole argument for length prefixes, as an assertion. If these two encode
    // identically the encoding is forgeable: an attacker who can choose a device id or a
    // nonce could move bytes across a field boundary and keep the signature valid.
    Map<String, dynamic> find(String id) => cases.firstWhere(
          (c) => c['id'] == id,
          orElse: () => fail('missing required case $id'),
        );
    expect(
      find('boundary-ambiguity-sender-ab-nonce-c')['expectedBytesHex'],
      isNot(equals(find('boundary-ambiguity-sender-a-nonce-bc')['expectedBytesHex'])),
      reason: "sender 'ab' + nonce 'c' must not encode to the same bytes as sender 'a' + "
          "nonce 'bc'; if they match, the encoding is forgeable and the vectors are wrong",
    );
  });

  test('a signature produced by the Rust implementation verifies here', () async {
    // The property the whole design rests on, proven by execution rather than by both sides
    // passing their own tests. These bytes were emitted by `ed25519-dalek` in the desktop
    // suite; nothing in this package produced them.
    //
    // The Rust side runs the mirror of this test against the Dart-produced entry, and the two
    // entries use different seeds, so neither can pass by verifying its own signature.
    final fixtures = loadVectorSet('cross_language_signatures.json');
    final shared = fixtures['sharedPayload'] as Map<String, dynamic>;
    final rustProduced = fixtures['rustProduced'] as Map<String, dynamic>;

    // First: both implementations must encode the payload identically. A signature over a
    // different encoding would fail below for a reason that has nothing to do with crypto,
    // so this is asserted separately to keep the two failures distinguishable.
    final payload = encodeSigningPayload(
      sender: DeviceId(shared['sender'] as String),
      command: shared['command'] as String,
      createdAt: DateTime.fromMillisecondsSinceEpoch(
        shared['createdAtMillis'] as int,
        isUtc: true,
      ),
      nonce: shared['nonce'] as String,
    );
    expect(
      hexEncode(payload),
      shared['payloadHex'],
      reason: 'the two implementations do not agree on the signed bytes',
    );

    final key = VerifyingKey.fromBytes(
      hexDecode(rustProduced['verifyingKeyHex'] as String),
    );
    expect(key, isNotNull, reason: 'the Rust verifying key must be valid here');

    expect(
      await verifySignature(
        payload: payload,
        signature: hexDecode(rustProduced['signatureHex'] as String),
        key: key!,
      ),
      isTrue,
      reason: 'a signature made by the Rust implementation must verify in Dart; if this '
          'fails, a phone and a desktop cannot command each other',
    );
  });

  test('the Dart-produced fixture still matches this implementation', () async {
    // Guards the other half of the pair: if this package's signing or encoding changes, the
    // committed fixture the Rust suite verifies goes stale, and the Rust test would fail with
    // no indication of why. Failing here names the cause.
    final fixtures = loadVectorSet('cross_language_signatures.json');
    final shared = fixtures['sharedPayload'] as Map<String, dynamic>;
    final dartProduced = fixtures['dartProduced'] as Map<String, dynamic>;

    final payload = encodeSigningPayload(
      sender: DeviceId(shared['sender'] as String),
      command: shared['command'] as String,
      createdAt: DateTime.fromMillisecondsSinceEpoch(
        shared['createdAtMillis'] as int,
        isUtc: true,
      ),
      nonce: shared['nonce'] as String,
    );

    final key = VerifyingKey.fromBytes(
      hexDecode(dartProduced['verifyingKeyHex'] as String),
    );
    expect(key, isNotNull);

    expect(
      await verifySignature(
        payload: payload,
        signature: hexDecode(dartProduced['signatureHex'] as String),
        key: key!,
      ),
      isTrue,
      reason: 'the committed Dart-produced fixture no longer verifies against this '
          'implementation; regenerate it, and expect the Rust suite to have failed too',
    );
  });

  test('shared device id derivation vectors all match', () {
    // Unlike the decision vectors, these pin a *derivation*: a verifying key in, an
    // identifier out. Two implementations can each be internally consistent while
    // disagreeing about the answer, and that only surfaces when a phone and a desktop try
    // to recognise each other — by which point each has recorded the other under an id the
    // other does not answer to.
    final failures = <String>[];

    for (final c in casesOf('device_id_derivation.json')) {
      final id = c['id'] as String;
      final keyBytes = hexDecode(c['verifyingKeyHex'] as String);
      expect(keyBytes, hasLength(32), reason: '$id: verifyingKeyHex must be 32 bytes');

      final actual = deriveDeviceId(keyBytes);
      final expected = c['expectedDeviceId'] as String;
      if (actual.value != expected) {
        failures.add('$id: expected $expected, got ${actual.value}  '
            '(${c['description']})');
      }
    }

    expect(failures, isEmpty, reason: failures.join('\n'));
  });

  test('device id derivation vectors prove the whole key is hashed', () {
    // The three one-byte-difference cases are the only thing standing between this
    // derivation and one that hashes a prefix, or skips the hash and truncates the key
    // directly. Each differs from the all-zeroes key in exactly one byte — at the start,
    // past the 16-byte truncation point, and at the very end.
    final cases = casesOf('device_id_derivation.json');
    final ids = cases.map((c) => c['id'] as String).toSet();

    for (final required in [
      'all-zeroes-key',
      'all-ff-key',
      'known-good-key',
      'one-byte-difference-first-byte',
      'one-byte-difference-middle-byte',
      'one-byte-difference-last-byte',
    ]) {
      expect(ids, contains(required), reason: 'the $required case must not be removed');
    }

    Map<String, dynamic> find(String id) => cases.firstWhere(
          (c) => c['id'] == id,
          orElse: () => fail('missing required case $id'),
        );

    // The argument itself, as an assertion. If any two of these expected ids match, the
    // derivation does not depend on the whole key and the vectors are wrong — a device
    // could then present a different key under a paired device's identifier, which is
    // exactly what deriving the id from the key exists to prevent.
    final seen = <String, String>{};
    for (final id in [
      'all-zeroes-key',
      'one-byte-difference-first-byte',
      'one-byte-difference-middle-byte',
      'one-byte-difference-last-byte',
    ]) {
      final derived = find(id)['expectedDeviceId'] as String;
      final other = seen[derived];
      expect(
        other,
        isNull,
        reason: '$id and $other derive the same device id ($derived); these keys differ '
            'by one byte, so a derivation mapping them together ignores part of the key',
      );
      seen[derived] = id;
    }

    // Every id is the full 32 hex characters of a 16-byte value. A shorter one would mean
    // a leading zero byte was dropped somewhere, colliding two devices that differ.
    for (final c in cases) {
      expect(
        (c['expectedDeviceId'] as String).length,
        32,
        reason: '${c['id']}: a device id is 16 bytes as 32 hex characters',
      );
    }
  });

  test('shared pairing store vectors all match', () {
    // The store's storage is platform code and differs per OS; these are its shared rules.
    final set = loadVectorSet('pairing_store.json');
    final keys = (set['keys'] as Map).cast<String, dynamic>();
    final cases = (set['cases'] as List).cast<Map<String, dynamic>>();
    expect(cases, isNotEmpty, reason: 'pairing_store.json declares no cases');

    VerifyingKey keyOf(String name) {
      final hex = keys[name] as String?;
      if (hex == null) {
        fail('no key named $name in the file-level keys object');
      }
      final key = VerifyingKey.fromBytes(hexDecode(hex));
      if (key == null) {
        fail('key $name is not a valid Ed25519 public key');
      }
      return key;
    }

    final failures = <String>[];

    for (final c in cases) {
      final id = c['id'] as String;

      // Build the store's rows exactly as the case states them, then ask the production
      // function which keys a target would actually check signatures against.
      final pairings = [
        for (final entry in (c['pairings'] as List).cast<Map<String, dynamic>>())
          PairingRecord(
            peer: DeviceId(entry['peerDeviceId'] as String),
            verifyingKey: keyOf(entry['verifyingKeyRef'] as String),
            revoked: entry['revoked'] as bool,
          ),
      ];

      final sender = DeviceId(c['sender'] as String);
      final actual = verifyingKeysFromPairings(pairings)[sender];
      final expectedRef = c['expectedKeyRef'] as String?;

      if (expectedRef == null) {
        // Absent, not present-and-rejected-later. A key that appeared here would be
        // verified against before anything refused it, which is the behaviour D5 rules out.
        if (actual != null) {
          failures.add('$id: expected no key but one was returned  (${c['description']})');
        }
      } else {
        final expected = keyOf(expectedRef);
        if (actual == null) {
          failures.add('$id: expected key $expectedRef but the sender is absent from the '
              'map  (${c['description']})');
        } else if (actual != expected) {
          failures.add('$id: the wrong key was returned  (${c['description']})');
        }
      }
    }

    expect(failures, isEmpty, reason: failures.join('\n'));
  });

  test('pairing store vectors pin revocation, re-pairing, and the per-pairing split', () {
    final set = loadVectorSet('pairing_store.json');
    final cases = (set['cases'] as List).cast<Map<String, dynamic>>();
    final ids = cases.map((c) => c['id'] as String).toSet();

    for (final required in [
      // The ordinary case, and the two ways a key can be absent.
      'paired-peer-yields-its-key',
      'unpaired-peer-is-absent',
      // D5: revoked is absent, not present-and-rejected-later.
      'revoked-peer-is-absent-not-rejected-later',
      // Revocation withdraws authority; it does not blacklist.
      're-paired-formerly-revoked-peer-yields-its-key-again',
      // The pair proving revocation is per-pairing rather than a global switch.
      'revocation-is-per-pairing-not-global-revoked-one',
      'revocation-is-per-pairing-not-global-active-one',
      // One peer's key must not stand in for another's.
      'a-peer-is-checked-against-its-own-key-not-another-peers',
      // The first-run state.
      'an-empty-store-yields-absent',
    ]) {
      expect(ids, contains(required), reason: 'the $required case must not be removed');
    }

    Map<String, dynamic> find(String id) => cases.firstWhere(
          (c) => c['id'] == id,
          orElse: () => fail('missing required case $id'),
        );

    // The per-pairing split, as an assertion rather than as prose. The two cases must hold
    // identical pairings and differ only in which peer is asked about — otherwise they do
    // not isolate the property, and an implementation treating revocation as a global
    // switch could pass both.
    final revokedSide = find('revocation-is-per-pairing-not-global-revoked-one');
    final activeSide = find('revocation-is-per-pairing-not-global-active-one');
    expect(
      (revokedSide['pairings'] as List).length,
      (activeSide['pairings'] as List).length,
      reason: 'the per-pairing split cases must hold the same pairings',
    );
    expect(
      revokedSide['sender'],
      isNot(equals(activeSide['sender'])),
      reason: 'the per-pairing split cases must ask about different peers',
    );
    expect(
      revokedSide['expectedKeyRef'] == null && activeSide['expectedKeyRef'] != null,
      isTrue,
      reason: 'the per-pairing split must have opposite outcomes: the revoked peer absent, '
          'the active one present. Without that, revoking one pairing could disable the '
          'whole store and both cases would still pass',
    );

    // At least one case must expect absent for a *revoked* peer specifically, or the D5
    // rule is untested however many other cases the file grows.
    expect(
      cases.any((c) {
        if (c['expectedKeyRef'] != null) return false;
        return (c['pairings'] as List).cast<Map<String, dynamic>>().any(
            (e) => e['peerDeviceId'] == c['sender'] && e['revoked'] == true);
      }),
      isTrue,
      reason: 'no case asks about a peer whose pairing is revoked; the rule that revoked '
          'means absent would be unverified',
    );
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
      'signing_payload.json',
      'device_id_derivation.json',
      'pairing_store.json',
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

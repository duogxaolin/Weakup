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
      final expectedRaw = c['expectedTargetInstantUtc'] as String?;
      final expected = expectedRaw == null ? null : parseUtc(expectedRaw);

      final actual =
          TriggerResolver.resolve(parseTrigger(c['trigger'] as Map<String, dynamic>),
                  location, now: now)
              ?.toUtc();

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

  test('every vector case has a unique id and a description', () {
    for (final file in ['resolution.json', 'reconciliation.json', 'validation.json']) {
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

    final outcomes = casesOf('reconciliation.json')
        .map((c) => c['expectedOutcome'] as String)
        .toSet();
    expect(outcomes, contains('proceedToGracePeriod'));
    expect(outcomes, contains('overdue'));
  });
}

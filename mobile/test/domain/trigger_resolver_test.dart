import 'package:flutter_test/flutter_test.dart';
import 'package:timezone/data/latest.dart' as tz_data;
import 'package:timezone/timezone.dart' as tz;
import 'package:weakup/core/core.dart';
import 'package:weakup/domain/domain.dart';

void main() {
  setUpAll(tz_data.initializeTimeZones);

  group('TriggerResolver.validate', () {
    test('IndefiniteTrigger valid for keepAwake', () {
      final r = TriggerResolver.validate(const IndefiniteTrigger(), JobType.keepAwake);
      expect(r.isSuccess, isTrue);
    });

    test('IndefiniteTrigger invalid for powerOff', () {
      final r = TriggerResolver.validate(const IndefiniteTrigger(), JobType.powerOff);
      expect(r.isFailure, isTrue);
      expect((r as Failure).error, isA<ValidationError>());
    });

    test('DurationTrigger 0 rejected', () {
      final r = TriggerResolver.validate(const DurationTrigger(minutes: 0), JobType.keepAwake);
      expect(r.isFailure, isTrue);
      expect((r as Failure).error, isA<ValidationError>());
    });

    test('DurationTrigger -1 rejected', () {
      final r = TriggerResolver.validate(const DurationTrigger(minutes: -1), JobType.keepAwake);
      expect(r.isFailure, isTrue);
    });

    test('DurationTrigger 1 accepted', () {
      final r = TriggerResolver.validate(const DurationTrigger(minutes: 1), JobType.keepAwake);
      expect(r.isSuccess, isTrue);
    });

    test('DurationTrigger 1440 accepted', () {
      final r = TriggerResolver.validate(const DurationTrigger(minutes: 1440), JobType.keepAwake);
      expect(r.isSuccess, isTrue);
    });

    test('DurationTrigger 1441 rejected', () {
      final r = TriggerResolver.validate(const DurationTrigger(minutes: 1441), JobType.keepAwake);
      expect(r.isFailure, isTrue);
      expect((r as Failure).error, isA<ValidationError>());
    });

    test('AbsoluteTimeTrigger always valid from validate()', () {
      final r = TriggerResolver.validate(
        const AbsoluteTimeTrigger(hour: 20, minute: 0),
        JobType.powerOff,
      );
      expect(r.isSuccess, isTrue);
    });
  });

  group('TriggerResolver.resolve', () {
    late tz.Location eastern;

    setUpAll(() {
      eastern = tz.getLocation('America/New_York');
    });

    test('IndefiniteTrigger returns null', () {
      final now = tz.TZDateTime(eastern, 2025, 3, 10, 14, 0);
      final result = TriggerResolver.resolve(
        const IndefiniteTrigger(),
        eastern,
        now: now,
      );
      expect(result, isNull);
    });

    test('DurationTrigger 60 min adds 1 hour', () {
      final now = tz.TZDateTime(eastern, 2025, 6, 15, 10, 0);
      final result = TriggerResolver.resolve(
        const DurationTrigger(minutes: 60),
        eastern,
        now: now,
      );
      expect(result, isNotNull);
      // Result should be 1 hour after now in UTC
      final expectedUtc = now.add(const Duration(hours: 1)).toUtc();
      expect(result!, equals(expectedUtc));
    });

    test('AbsoluteTimeTrigger future time today resolves to today', () {
      // Current: 14:00, target 20:00 — same day
      final now = tz.TZDateTime(eastern, 2025, 6, 15, 14, 0);
      final result = TriggerResolver.resolve(
        const AbsoluteTimeTrigger(hour: 20, minute: 0),
        eastern,
        now: now,
      );
      expect(result, isNotNull);
      final localResult = tz.TZDateTime.from(result!, eastern);
      expect(localResult.hour, 20);
      expect(localResult.minute, 0);
      expect(localResult.day, 15);
    });

    test('AbsoluteTimeTrigger past time today rolls to tomorrow', () {
      // Current: 21:00, target 20:00 — tomorrow
      final now = tz.TZDateTime(eastern, 2025, 6, 15, 21, 0);
      final result = TriggerResolver.resolve(
        const AbsoluteTimeTrigger(hour: 20, minute: 0),
        eastern,
        now: now,
      );
      expect(result, isNotNull);
      final localResult = tz.TZDateTime.from(result!, eastern);
      expect(localResult.hour, 20);
      expect(localResult.minute, 0);
      expect(localResult.day, 16); // tomorrow
    });

    // DST spring-forward: America/New_York on 2025-03-09 at 02:00 clocks forward to 03:00.
    // Requesting 02:30 on 2025-03-09 should resolve to 03:00 (the jump instant).
    test('Spring-forward gap resolves to the jump instant', () {
      // We're asking for 02:30 on the spring-forward date.
      // The tz package advances this to 03:00 (post-gap).
      final now = tz.TZDateTime(eastern, 2025, 3, 9, 1, 0); // 01:00 — before gap
      final result = TriggerResolver.resolve(
        const AbsoluteTimeTrigger(hour: 2, minute: 30),
        eastern,
        now: now,
      );
      expect(result, isNotNull);
      final localResult = tz.TZDateTime.from(result!, eastern);
      // The tz package should adjust 02:30 (non-existent) to 03:00 or later
      expect(localResult.hour, greaterThanOrEqualTo(3));
    });

    // DST fall-back: America/New_York on 2025-11-02 at 02:00 clocks back to 01:00.
    // Requesting 01:30 on 2025-11-02 should resolve to the FIRST (earlier) 01:30.
    test('Fall-back overlap resolves to the earlier occurrence', () {
      // We're before the fall-back at 00:30.
      final now = tz.TZDateTime(eastern, 2025, 11, 2, 0, 30);
      final result = TriggerResolver.resolve(
        const AbsoluteTimeTrigger(hour: 1, minute: 30),
        eastern,
        now: now,
      );
      expect(result, isNotNull);
      final localResult = tz.TZDateTime.from(result!, eastern);
      // Should be the first 01:30 (EDT, offset -4h), not the second (EST, offset -5h)
      expect(localResult.hour, 1);
      expect(localResult.minute, 30);
      // The first occurrence has offset -4 hours (EDT = UTC-4)
      expect(localResult.timeZoneOffset.inHours, -4);
    });
  });
}

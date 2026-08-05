import 'package:flutter_test/flutter_test.dart';
import 'package:timezone/data/latest.dart' as tz_data;
import 'package:timezone/timezone.dart' as tz;
import 'package:weakup/core/core.dart';
import 'package:weakup/domain/domain.dart';

/// Resolves and unwraps, failing the test if resolution was refused.
///
/// Most cases here assert on the resolved instant, and only the dated ones care that
/// resolution can be refused at all. Those call [TriggerResolver.resolve] directly.
DateTime? resolved(
  TriggerSpec trigger,
  tz.Location location, {
  DateTime? now,
}) {
  final result = TriggerResolver.resolve(trigger, location, now: now);
  if (result.isFailure) {
    fail('resolve refused $trigger: ${(result as Failure).error}');
  }
  return result.valueOrNull;
}

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
      final result = resolved(
        const IndefiniteTrigger(),
        eastern,
        now: now,
      );
      expect(result, isNull);
    });

    test('DurationTrigger 60 min adds 1 hour', () {
      final now = tz.TZDateTime(eastern, 2025, 6, 15, 10, 0);
      final result = resolved(
        const DurationTrigger(minutes: 60),
        eastern,
        now: now,
      );
      expect(result, isNotNull);
      // Result should be 1 hour after now in UTC.
      final expectedUtc = now.add(const Duration(hours: 1)).toUtc();
      expect(result!.isAtSameMomentAs(expectedUtc), isTrue,
          reason: 'expected $expectedUtc, got $result');
    });

    test('resolve returns a plain UTC DateTime, not a TZDateTime', () {
      // TZDateTime.operator== requires the other side to be a TZDateTime with an
      // equal location, so leaking one here would compare unequal to the same
      // instant loaded back from the database — silently, with no type error.
      final now = tz.TZDateTime(eastern, 2025, 6, 15, 10, 0);
      for (final trigger in const [
        DurationTrigger(minutes: 60),
        AbsoluteTimeTrigger(hour: 23, minute: 30),
      ]) {
        final result = resolved(trigger, eastern, now: now);
        expect(result, isNotNull);
        expect(result, isNot(isA<tz.TZDateTime>()), reason: 'for $trigger');
        expect(result!.isUtc, isTrue, reason: 'for $trigger');
        // Round-trips through the equality operator the app actually relies on.
        expect(
          DateTime.fromMicrosecondsSinceEpoch(result.microsecondsSinceEpoch,
              isUtc: true),
          equals(result),
          reason: 'for $trigger',
        );
      }
    });

    test('AbsoluteTimeTrigger future time today resolves to today', () {
      // Current: 14:00, target 20:00 — same day
      final now = tz.TZDateTime(eastern, 2025, 6, 15, 14, 0);
      final result = resolved(
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
      final result = resolved(
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
    // Requesting 02:30 on 2025-03-09 must resolve to 03:00 — the jump instant.
    test('Spring-forward gap resolves to the jump instant', () {
      final now = tz.TZDateTime(eastern, 2025, 3, 9, 1, 0); // 01:00 — before gap
      final result = resolved(
        const AbsoluteTimeTrigger(hour: 2, minute: 30),
        eastern,
        now: now,
      );
      expect(result, isNotNull);
      final localResult = tz.TZDateTime.from(result!, eastern);
      // Exactly 03:00, not merely "03:00 or later". The looser assertion this
      // replaces passed while the resolver returned 03:30, because TZDateTime
      // preserves the requested offset-from-midnight across a gap instead of
      // clamping to the transition. For a shutdown, half an hour late is wrong.
      expect(localResult.hour, 3);
      expect(localResult.minute, 0);
    });

    // DST fall-back: America/New_York on 2025-11-02 at 02:00 clocks back to 01:00.
    // Requesting 01:30 on 2025-11-02 should resolve to the FIRST (earlier) 01:30.
    test('Fall-back overlap resolves to the earlier occurrence', () {
      // We're before the fall-back at 00:30.
      final now = tz.TZDateTime(eastern, 2025, 11, 2, 0, 30);
      final result = resolved(
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

  group('TriggerResolver.resolve with a date', () {
    late tz.Location eastern;
    late tz.Location saigon;

    setUpAll(() {
      eastern = tz.getLocation('America/New_York');
      saigon = tz.getLocation('Asia/Ho_Chi_Minh');
    });

    test('a dated wall time resolves to that exact date', () {
      // Eleven days out. The undated form would have resolved to today or tomorrow.
      final now = tz.TZDateTime(saigon, 2026, 7, 30, 14, 0);
      final result = resolved(
        AbsoluteTimeTrigger(
          hour: 20,
          minute: 0,
          date: CalendarDate(year: 2026, month: 8, day: 10),
        ),
        saigon,
        now: now,
      );

      expect(result, isNotNull);
      final local = tz.TZDateTime.from(result!, saigon);
      expect(local.year, 2026);
      expect(local.month, 8);
      expect(local.day, 10);
      expect(local.hour, 20);
    });

    test('a dated wall time in the past is rejected, not rolled forward', () {
      // The counterpart of 'past time today rolls to tomorrow': identical now, zone,
      // and time-of-day, differing only by carrying today's date. Moving an
      // irreversible power-off to a day the user never chose is worse than refusing.
      final now = tz.TZDateTime(saigon, 2026, 7, 30, 21, 0);
      final result = TriggerResolver.resolve(
        AbsoluteTimeTrigger(
          hour: 20,
          minute: 0,
          date: CalendarDate(year: 2026, month: 7, day: 30),
        ),
        saigon,
        now: now,
      );

      expect(result.isFailure, isTrue);
      final error = (result as Failure).error;
      expect(error, isA<ValidationError>());
      // Byte-identical to the desktop implementation's message, as every other
      // message in these two files is.
      expect((error as ValidationError).message,
          'That date and time have already passed.');
    });

    test('a dated wall time equal to now is rejected', () {
      // Strictly in the future, as for the undated form.
      final now = tz.TZDateTime(saigon, 2026, 7, 30, 20, 0);
      final result = TriggerResolver.resolve(
        AbsoluteTimeTrigger(
          hour: 20,
          minute: 0,
          date: CalendarDate(year: 2026, month: 7, day: 30),
        ),
        saigon,
        now: now,
      );
      expect(result.isFailure, isTrue);
    });

    test('a dated wall time one minute after now is accepted', () {
      // Pins the boundary from the other side, so the comparison cannot drift.
      final now = tz.TZDateTime(saigon, 2026, 7, 30, 19, 59);
      final result = resolved(
        AbsoluteTimeTrigger(
          hour: 20,
          minute: 0,
          date: CalendarDate(year: 2026, month: 7, day: 30),
        ),
        saigon,
        now: now,
      );
      expect(result, isNotNull);
    });

    test('a dated wall time resolves the spring-forward gap identically', () {
      // Supplying a date must not open a second DST path. `now` is a week earlier, so
      // the date is what selects the day rather than the clock.
      final now = tz.TZDateTime(eastern, 2026, 3, 1, 1, 0);
      final result = resolved(
        AbsoluteTimeTrigger(
          hour: 2,
          minute: 30,
          date: CalendarDate(year: 2026, month: 3, day: 8),
        ),
        eastern,
        now: now,
      );

      final local = tz.TZDateTime.from(result!, eastern);
      expect(local.hour, 3);
      expect(local.minute, 0);
    });

    test('a dated wall time takes the earlier of an ambiguous pair', () {
      final now = tz.TZDateTime(eastern, 2026, 10, 25, 0, 30);
      final result = resolved(
        AbsoluteTimeTrigger(
          hour: 1,
          minute: 30,
          date: CalendarDate(year: 2026, month: 11, day: 1),
        ),
        eastern,
        now: now,
      );

      final local = tz.TZDateTime.from(result!, eastern);
      expect(local.hour, 1);
      expect(local.minute, 30);
      expect(local.timeZoneOffset.inHours, -4, reason: 'the earlier occurrence is EDT');
    });

    test('validation does not ask whether a dated instant has passed', () {
      // It has no clock and no location to answer with. A date long past is still
      // structurally valid; resolve is what refuses it.
      final result = TriggerResolver.validate(
        AbsoluteTimeTrigger(
          hour: 20,
          minute: 0,
          date: CalendarDate(year: 2020, month: 1, day: 1),
        ),
        JobType.powerOff,
      );
      expect(result.isSuccess, isTrue);
    });

    test('a date does not weaken the time-of-day bounds', () {
      final date = CalendarDate(year: 2026, month: 8, day: 10);
      expect(
        TriggerResolver.validate(
          AbsoluteTimeTrigger(hour: 24, minute: 0, date: date),
          JobType.powerOff,
        ).isFailure,
        isTrue,
      );
      expect(
        TriggerResolver.validate(
          AbsoluteTimeTrigger(hour: 12, minute: 60, date: date),
          JobType.powerOff,
        ).isFailure,
        isTrue,
      );
    });

    test('a dated and an undated trigger are not equal', () {
      // They mean different things, so they must not compare equal — otherwise a
      // stored dated job would look unchanged after the date was dropped.
      expect(
        AbsoluteTimeTrigger(
          hour: 20,
          minute: 0,
          date: CalendarDate(year: 2026, month: 8, day: 10),
        ),
        isNot(const AbsoluteTimeTrigger(hour: 20, minute: 0)),
      );
    });
  });
}

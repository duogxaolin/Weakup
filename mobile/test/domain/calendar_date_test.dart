// `CalendarDate` is small enough that its tests are mostly about what it *refuses*.
//
// Its whole job is to make a malformed date unrepresentable, which is what lets the
// resolver, the mapper, and the shared vectors all stop worrying about February 30th.
// If the validating factory ever loosens, these are what catch it.

import 'package:flutter_test/flutter_test.dart';
import 'package:weakup/domain/domain.dart';

void main() {
  group('CalendarDate construction', () {
    test('accepts a real date', () {
      final date = CalendarDate(year: 2026, month: 8, day: 10);
      expect(date.year, 2026);
      expect(date.month, 8);
      expect(date.day, 10);
    });

    test('rejects a month outside 1-12', () {
      expect(() => CalendarDate(year: 2026, month: 13, day: 1),
          throwsFormatException);
      expect(() => CalendarDate(year: 2026, month: 0, day: 1),
          throwsFormatException);
    });

    test('rejects a day that does not exist in that month', () {
      // The case that motivates the type: `DateTime.utc(2026, 2, 30)` silently
      // normalises to 2 March, which for a scheduled shutdown is the wrong day.
      expect(() => CalendarDate(year: 2026, month: 2, day: 30),
          throwsFormatException);
      expect(() => CalendarDate(year: 2026, month: 4, day: 31),
          throwsFormatException);
      expect(() => CalendarDate(year: 2026, month: 8, day: 0),
          throwsFormatException);
    });

    test('knows leap years', () {
      expect(CalendarDate(year: 2028, month: 2, day: 29).day, 29);
      expect(() => CalendarDate(year: 2026, month: 2, day: 29),
          throwsFormatException);
      // 1900 is not a leap year; 2000 is. The century rule, not just divisibility by 4.
      expect(() => CalendarDate(year: 1900, month: 2, day: 29),
          throwsFormatException);
      expect(CalendarDate(year: 2000, month: 2, day: 29).day, 29);
    });
  });

  group('CalendarDate parsing and formatting', () {
    test('round-trips YYYY-MM-DD', () {
      const raw = '2026-08-10';
      expect(CalendarDate.parse(raw).format(), raw);
    });

    test('zero-pads on the way out', () {
      expect(CalendarDate(year: 2026, month: 1, day: 5).format(), '2026-01-05');
    });

    test('rejects anything that is not exactly YYYY-MM-DD', () {
      // Strict on purpose. `DateTime.parse` would accept the ISO instant forms and
      // quietly discard the parts a wall-clock date must not carry.
      for (final bad in [
        '2026-8-10',
        '10/08/2026',
        '2026-08-10T22:30:00Z',
        '2026-08',
        '',
        'tomorrow',
      ]) {
        expect(() => CalendarDate.parse(bad), throwsFormatException,
            reason: 'should not parse "$bad"');
      }
    });

    test('rejects a well-shaped string naming an impossible date', () {
      // The regex matches; the factory is what refuses it.
      expect(() => CalendarDate.parse('2026-02-30'), throwsFormatException);
      expect(() => CalendarDate.parse('2026-13-01'), throwsFormatException);
    });

    test('tryParse returns null rather than throwing', () {
      expect(CalendarDate.tryParse(null), isNull);
      expect(CalendarDate.tryParse('nonsense'), isNull);
      expect(CalendarDate.tryParse('2026-08-10'),
          CalendarDate(year: 2026, month: 8, day: 10));
    });
  });

  group('CalendarDate equality', () {
    test('two dates naming the same day are equal and hash alike', () {
      // Value semantics matter here: the trigger's `==` delegates to this, and a
      // reference comparison would make a reloaded job look different from the one
      // that was saved.
      final a = CalendarDate(year: 2026, month: 8, day: 10);
      final b = CalendarDate.parse('2026-08-10');
      expect(a, b);
      expect(a.hashCode, b.hashCode);
    });

    test('different days are not equal', () {
      expect(CalendarDate(year: 2026, month: 8, day: 10),
          isNot(CalendarDate(year: 2026, month: 8, day: 11)));
    });
  });
}

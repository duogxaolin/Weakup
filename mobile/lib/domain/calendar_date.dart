/// A wall-clock calendar date: a year, a month, and a day, with no time and no zone.
///
/// Deliberately **not** a `DateTime`. The caution is the same one the `_toPlainUtc`
/// comment in `trigger_resolver.dart` records: a `DateTime` carries zone semantics, and
/// two `DateTime`s naming the same calendar day can compare unequal, or shift a day, the
/// moment one of them is UTC and the other is not. "10 August" is not an instant — it
/// becomes one only when a time and a location are applied to it, which is
/// `TriggerResolver`'s job and not this type's.
///
/// Mirrors Rust's `chrono::NaiveDate` in the desktop implementation. The wire format both
/// sides agree on is `YYYY-MM-DD`, documented in `shared/testvectors/README.md`.
final class CalendarDate {
  /// Throws [FormatException] if [year], [month], and [day] are not a real date.
  ///
  /// Validating in the constructor rather than leaving it to the resolver is what makes
  /// a malformed date unrepresentable — the mirror of `NaiveDate::from_ymd_opt`
  /// returning `None`. February 30th cannot be constructed, so no downstream code has to
  /// consider it, and the shared vectors need no malformed-date case.
  factory CalendarDate({required int year, required int month, required int day}) {
    if (month < 1 || month > 12) {
      throw FormatException('Month must be between 1 and 12, got $month');
    }
    if (day < 1 || day > _daysInMonth(year, month)) {
      throw FormatException('Day $day does not exist in $year-$month');
    }
    return CalendarDate._(year, month, day);
  }

  const CalendarDate._(this.year, this.month, this.day);

  /// Parses `YYYY-MM-DD`, throwing [FormatException] on anything else.
  ///
  /// Strict on purpose: `DateTime.parse` would accept `2026-08-10T22:30:00Z` and quietly
  /// discard the parts a calendar date must not carry.
  factory CalendarDate.parse(String raw) {
    final match = RegExp(r'^(\d{4})-(\d{2})-(\d{2})$').firstMatch(raw);
    if (match == null) {
      throw FormatException('Expected a date as YYYY-MM-DD, got "$raw"');
    }
    return CalendarDate(
      year: int.parse(match.group(1)!),
      month: int.parse(match.group(2)!),
      day: int.parse(match.group(3)!),
    );
  }

  /// Returns null instead of throwing, for parsing values of unknown provenance
  /// (a database row, an IPC payload).
  static CalendarDate? tryParse(String? raw) {
    if (raw == null) return null;
    try {
      return CalendarDate.parse(raw);
    } on FormatException {
      return null;
    }
  }

  final int year;
  final int month;
  final int day;

  /// `YYYY-MM-DD`, the format Rust's `NaiveDate` serialises to and the format
  /// [CalendarDate.parse] reads.
  String format() => '${year.toString().padLeft(4, '0')}-'
      '${month.toString().padLeft(2, '0')}-'
      '${day.toString().padLeft(2, '0')}';

  /// Day 0 of the following month is the last day of this one, which handles leap
  /// years without restating the rule.
  static int _daysInMonth(int year, int month) =>
      DateTime.utc(year, month + 1, 0).day;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      (other is CalendarDate &&
          other.year == year &&
          other.month == month &&
          other.day == day);

  @override
  int get hashCode => Object.hash(year, month, day);

  @override
  String toString() => format();
}

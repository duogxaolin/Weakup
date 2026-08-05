import 'package:drift/drift.dart';

/// Drift table for [Job] entities.
/// The generated data class is named [JobRow] to avoid collision with
/// the domain [Job] entity.
@DataClassName('JobRow')
class Jobs extends Table {
  IntColumn get id => integer().autoIncrement()();
  TextColumn get type => text()(); // 'keepAwake' | 'powerOff'
  TextColumn get triggerKind => text()(); // 'indefinite' | 'duration' | 'absoluteTime'
  IntColumn get triggerMinutes => integer().nullable()(); // DurationTrigger
  IntColumn get triggerHour => integer().nullable()(); // AbsoluteTimeTrigger
  IntColumn get triggerMinute => integer().nullable()(); // AbsoluteTimeTrigger
  /// The exact local date an AbsoluteTimeTrigger fires on, as `YYYY-MM-DD`.
  ///
  /// Nullable, and null means "a time of day" — which is what every row written
  /// before this column existed meant, so the migration needs no backfill.
  ///
  /// Text rather than a `DateTimeColumn`: a wall-clock date is not an instant, and
  /// storing it as one would attach a zone the value must not carry.
  TextColumn get triggerDate => text().nullable()();
  TextColumn get status => text()();
  DateTimeColumn get targetInstantUtc => dateTime().nullable()();
  DateTimeColumn get createdAt => dateTime()();
  DateTimeColumn get updatedAt => dateTime()();
  TextColumn get failureMessage => text().nullable()();
}

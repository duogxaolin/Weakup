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
  TextColumn get status => text()();
  DateTimeColumn get targetInstantUtc => dateTime().nullable()();
  DateTimeColumn get createdAt => dateTime()();
  DateTimeColumn get updatedAt => dateTime()();
  TextColumn get failureMessage => text().nullable()();
}

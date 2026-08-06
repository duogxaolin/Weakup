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

  /// Whether this job was scheduled at this device or created by an authorized remote
  /// command, as `local` or `remote`.
  ///
  /// Nullable, and null means `local` — which is what every row written before this column
  /// existed meant, so the migration needs no backfill.
  ///
  /// Not cosmetic: the countdown a power-off job receives is chosen from its origin, so an
  /// origin that did not survive a restart handed a remote-initiated shutdown the shorter
  /// local countdown.
  TextColumn get origin => text().nullable()();
}

/// Every decision this device reached about an arriving remote command.
///
/// A separate table rather than a column on [Jobs], because a refused command creates no job
/// and refusals are precisely what must be recorded: a series of them is the visible signature
/// of an attack. A record that existed only when the command succeeded would be blind to the
/// case it is most needed for.
///
/// Local by requirement — never sent to a relay. It must be readable at the device when no
/// network is reachable, which is exactly when someone is most likely to be asking what shut
/// their machine down.
@DataClassName('CommandDecisionRow')
class CommandDecisions extends Table {
  IntColumn get id => integer().autoIncrement()();

  /// The device that sent the command.
  TextColumn get senderDeviceId => text()();

  /// What it asked for, as the wire name both implementations serialise.
  TextColumn get command => text()();

  /// `accepted` or `rejected`. Separate from the reason so a reader can count refusals
  /// without interpreting them.
  TextColumn get decision => text()();

  /// The single refusal reason, or null when accepted.
  TextColumn get rejectionReason => text().nullable()();

  /// When the decision was reached.
  DateTimeColumn get decidedAt => dateTime()();
}

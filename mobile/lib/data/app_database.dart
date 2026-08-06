import 'package:drift/drift.dart';
import 'package:drift_flutter/drift_flutter.dart';

import 'tables.dart';

part 'app_database.g.dart';

@DriftDatabase(tables: [Jobs, CommandDecisions])
class AppDatabase extends _$AppDatabase {
  AppDatabase([QueryExecutor? executor]) : super(executor ?? _openConnection());

  @override
  int get schemaVersion => 3;

  /// There was no migration strategy before version 2, because there had never been a
  /// second version. Without one, Drift leaves an existing database at its old shape
  /// and every query naming `trigger_date` fails at runtime on an upgraded install —
  /// while a fresh install works, which is the shape of bug that reaches users.
  ///
  /// Each step is a separate `if` on the *from* version rather than an else-if chain, so an
  /// install upgrading from 1 straight to 3 runs both steps in order. Adding a version means
  /// adding a step here, never editing an existing one — a v2 install that has already run
  /// the first step must still run only the second.
  @override
  MigrationStrategy get migration => MigrationStrategy(
        onUpgrade: (m, from, to) async {
          if (from < 2) {
            await m.addColumn(jobs, jobs.triggerDate);
          }
          if (from < 3) {
            // Null means `local`, which is what every pre-migration row meant. Until this
            // column existed the origin was not stored at all, so a remote job that survived
            // a restart came back as local and was handed the shorter countdown.
            await m.addColumn(jobs, jobs.origin);
            await m.createTable(commandDecisions);
          }
        },
      );

  static QueryExecutor _openConnection() {
    return driftDatabase(name: 'weakup_db');
  }
}

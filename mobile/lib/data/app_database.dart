import 'package:drift/drift.dart';
import 'package:drift_flutter/drift_flutter.dart';

import 'tables.dart';

part 'app_database.g.dart';

@DriftDatabase(tables: [Jobs])
class AppDatabase extends _$AppDatabase {
  AppDatabase([QueryExecutor? executor]) : super(executor ?? _openConnection());

  @override
  int get schemaVersion => 2;

  /// There was no migration strategy before version 2, because there had never been a
  /// second version. Without one, Drift leaves an existing database at its old shape
  /// and every query naming `trigger_date` fails at runtime on an upgraded install —
  /// while a fresh install works, which is the shape of bug that reaches users.
  @override
  MigrationStrategy get migration => MigrationStrategy(
        onUpgrade: (m, from, to) async {
          if (from < 2) {
            await m.addColumn(jobs, jobs.triggerDate);
          }
        },
      );

  static QueryExecutor _openConnection() {
    return driftDatabase(name: 'weakup_db');
  }
}

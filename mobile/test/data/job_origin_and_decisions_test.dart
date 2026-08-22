// Tests for persisted job origin and the command decision record.
//
// Mirrors the Rust `origin` and `command_decisions` test modules in
// `desktop/src-tauri/src/data/sqlite_repository_tests.rs`.

import 'dart:io';

// `isNull` is hidden because drift exports a SQL expression of that name and matcher exports
// the test matcher; unqualified it is ambiguous. The matcher is the one these tests want.
import 'package:drift/drift.dart' hide isNull;
import 'package:drift/native.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:weakup/data/app_database.dart';
import 'package:weakup/data/job_dao.dart';
import 'package:weakup/data/job_mapper.dart';
import 'package:weakup/domain/job.dart';
import 'package:weakup/domain/job_enums.dart';
import 'package:weakup/domain/trigger_spec.dart';

AppDatabase _openTestDb() => AppDatabase(NativeDatabase.memory());

/// A drift database with no schema of its own, used only to run raw statements against a file.
///
/// It exists so the pre-migration fixture can be built without importing `package:sqlite3`
/// directly — that is a transitive dependency of drift rather than one this package declares,
/// and depending on it here would be a dependency added for a test fixture.
///
/// [schemaVersion] is 1 and there is no migration strategy, so opening this never writes a
/// `user_version` the fixture does not want.
class _RawExecutorDatabase extends GeneratedDatabase {
  _RawExecutorDatabase(super.executor);

  @override
  Iterable<TableInfo<Table, dynamic>> get allTables => const [];

  @override
  int get schemaVersion => 1;
}

Job _makeJob({JobOrigin origin = JobOrigin.local}) {
  final now = DateTime.utc(2026, 8, 6, 12);
  return Job(
    id: 0,
    type: JobType.powerOff,
    trigger: const DurationTrigger(minutes: 30),
    status: JobStatus.active,
    targetInstantUtc: now.add(const Duration(minutes: 30)),
    createdAt: now,
    updatedAt: now,
    origin: origin,
  );
}

void main() {
  late AppDatabase db;
  late JobDao dao;

  setUp(() {
    db = _openTestDb();
    dao = JobDao(db);
  });

  tearDown(() => db.close());

  group('persisted job origin', () {
    test('a remote job round-trips as remote', () async {
      // Before the origin column existed this came back as local, which is the bug this
      // closes: the read produced a default rather than what was written.
      final inserted = await dao.insertJob(_makeJob(origin: JobOrigin.remote));
      expect(inserted.isSuccess, isTrue);

      final rows = await db.select(db.jobs).get();
      expect(rows, hasLength(1));
      expect(JobMapper.toDomain(rows.first).origin, JobOrigin.remote);
    });

    test('a local job round-trips as local', () async {
      final inserted = await dao.insertJob(_makeJob());
      expect(inserted.isSuccess, isTrue);

      final rows = await db.select(db.jobs).get();
      expect(JobMapper.toDomain(rows.first).origin, JobOrigin.local);
    });

    test('a null origin reads as local', () async {
      // Every row written before this column existed was scheduled at this device, so this
      // is a faithful reading of old data rather than a default standing in for information
      // that was lost.
      await dao.insertJob(_makeJob(origin: JobOrigin.remote));
      await db
          .customStatement('UPDATE jobs SET origin = NULL WHERE id = 1');

      final rows = await db.select(db.jobs).get();
      expect(rows.first.origin, isNull);
      expect(JobMapper.toDomain(rows.first).origin, JobOrigin.local);
    });

    test('an unrecognised stored origin throws rather than reading as local', () async {
      // Silently reading it as local would shorten a power-off countdown, which is the
      // failure this column exists to prevent. Matches the desktop repository, which returns
      // a storage error for the same case.
      await dao.insertJob(_makeJob());
      await db.customStatement(
        "UPDATE jobs SET origin = 'sideways' WHERE id = 1",
      );

      final rows = await db.select(db.jobs).get();
      expect(() => JobMapper.toDomain(rows.first), throwsStateError);
    });

    test('the origin survives a status update', () async {
      // Updating status must not disturb this column: a remote job paused and resumed must
      // not quietly become local.
      await dao.insertJob(_makeJob(origin: JobOrigin.remote));
      await (db.update(db.jobs)..where((j) => j.id.equals(1)))
          .write(JobsCompanion(status: Value(JobStatus.paused.name)));

      final rows = await db.select(db.jobs).get();
      expect(JobMapper.toDomain(rows.first).origin, JobOrigin.remote);
    });
  });

  group('the command decision record', () {
    Future<void> insertDecision({
      String sender = 'phone-a',
      String command = 'powerOff',
      String decision = 'accepted',
      String? rejectionReason,
      int minute = 0,
    }) =>
        db.into(db.commandDecisions).insert(
              CommandDecisionsCompanion.insert(
                senderDeviceId: sender,
                command: command,
                decision: decision,
                rejectionReason: Value(rejectionReason),
                decidedAt: DateTime.utc(2026, 8, 6, 12, minute),
              ),
            );

    test('an accepted decision is recorded with its sender and instant', () async {
      // Asserted on the stored row rather than on a return value: the point of the record is
      // that it can be read back afterwards to attribute a shutdown.
      await insertDecision();

      final rows = await db.select(db.commandDecisions).get();
      expect(rows, hasLength(1));
      expect(rows.first.senderDeviceId, 'phone-a');
      expect(rows.first.command, 'powerOff');
      expect(rows.first.decision, 'accepted');
      expect(rows.first.rejectionReason, isNull);
      // Compared as an instant rather than by equality on the DateTime: this database stores
      // datetimes as Unix seconds, so a value written as UTC reads back in local time. The
      // instant is what the record is about, and it must survive the round trip.
      expect(
        rows.first.decidedAt.toUtc(),
        DateTime.utc(2026, 8, 6, 12),
      );
    });

    test('a refusal is recorded with its single reason', () async {
      // A refused command creates no job, so if the refusal were not written here it would
      // leave no trace anywhere that anything was attempted.
      for (final reason in [
        'authenticityUnverified',
        'replayedNonce',
        'futureDated',
        'stale',
        'notPermitted',
      ]) {
        await insertDecision(decision: 'rejected', rejectionReason: reason);
      }

      final rows = await db.select(db.commandDecisions).get();
      expect(rows, hasLength(5));
      expect(
        rows.map((r) => r.rejectionReason).toSet(),
        {
          'authenticityUnverified',
          'replayedNonce',
          'futureDated',
          'stale',
          'notPermitted',
        },
      );
      expect(rows.every((r) => r.decision == 'rejected'), isTrue);
    });

    test('several refusals from one device are all retained', () async {
      // A series of refusals is the visible signature of an attack in progress, so the record
      // must keep every one rather than collapsing them into a latest-only row.
      for (var minute = 0; minute < 5; minute++) {
        await insertDecision(
          decision: 'rejected',
          rejectionReason: 'authenticityUnverified',
          minute: minute,
        );
      }

      final rows = await db.select(db.commandDecisions).get();
      expect(rows, hasLength(5),
          reason: 'refusals must not overwrite one another');
      expect(rows.every((r) => r.senderDeviceId == 'phone-a'), isTrue);
    });

    test('decisions from different devices stay attributable to each', () async {
      // Attribution is the whole purpose: "which device shut my machine down" must have an
      // answer, so two senders must not be confusable.
      await insertDecision(sender: 'phone-a');
      await insertDecision(
        sender: 'laptop-b',
        decision: 'rejected',
        rejectionReason: 'notPermitted',
        minute: 1,
      );

      final rows = await (db.select(db.commandDecisions)
            ..orderBy([(d) => OrderingTerm.desc(d.decidedAt)]))
          .get();
      expect(rows.first.senderDeviceId, 'laptop-b');
      expect(rows.first.rejectionReason, 'notPermitted');
      expect(rows.last.senderDeviceId, 'phone-a');
      expect(rows.last.rejectionReason, isNull);
    });

    test('the record is a local table, readable with no network', () async {
      // Nothing in this path opens a socket. Stated as a test so a future change that routed
      // reads through a relay would have to delete an assertion that says not to.
      await insertDecision();
      expect(await db.select(db.commandDecisions).get(), hasLength(1));
    });
  });

  group('the version 2 to 3 migration', () {
    test('a v2 job reads back as locally scheduled, and the new table exists', () async {
      // A genuine pre-migration database. The v2 `jobs` shape is written to a real file and
      // the file is then *closed*; only on the second open does Drift see user_version 2 and
      // run the actual onUpgrade path. Asserting against a database that was already at
      // version 3 would prove nothing about upgrading, which is the trap this test exists to
      // avoid.
      //
      // The v2 shape is created with raw statements through a plain executor rather than
      // through AppDatabase's schema, so it genuinely lacks the origin column and the
      // command_decisions table.
      final dir = await Directory.systemTemp.createTemp('weakup_v2_');
      final file = File('${dir.path}/v2.sqlite');

      final seed = _RawExecutorDatabase(NativeDatabase(file));
      await seed.customStatement('''
        CREATE TABLE jobs (
          id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
          type TEXT NOT NULL,
          trigger_kind TEXT NOT NULL,
          trigger_minutes INTEGER NULL,
          trigger_hour INTEGER NULL,
          trigger_minute INTEGER NULL,
          trigger_date TEXT NULL,
          status TEXT NOT NULL,
          target_instant_utc INTEGER NULL,
          created_at INTEGER NOT NULL,
          updated_at INTEGER NOT NULL,
          failure_message TEXT NULL
        )
      ''');
      await seed.customStatement('''
        INSERT INTO jobs (type, trigger_kind, trigger_minutes, status,
                          target_instant_utc, created_at, updated_at)
        VALUES ('powerOff', 'duration', 30, 'active', 1000, 1000, 1000)
      ''');
      await seed.customStatement('PRAGMA user_version = 2');
      await seed.close();

      // Opening runs onUpgrade from 2 to 3.
      final migrated = AppDatabase(NativeDatabase(file));

      final rows = await migrated.select(migrated.jobs).get();
      expect(rows, hasLength(1), reason: 'the pre-migration row must survive');
      expect(rows.first.origin, isNull,
          reason: 'a pre-migration row carries no origin');
      expect(JobMapper.toDomain(rows.first).origin, JobOrigin.local);

      // The new table arrives with the same migration, so an upgraded install can record a
      // decision immediately rather than on some later launch.
      await migrated.into(migrated.commandDecisions).insert(
            CommandDecisionsCompanion.insert(
              senderDeviceId: 'phone-a',
              command: 'powerOff',
              decision: 'accepted',
              decidedAt: DateTime.utc(2026, 8, 6, 12),
            ),
          );
      expect(
        await migrated.select(migrated.commandDecisions).get(),
        hasLength(1),
      );

      // And the migrated file is fully usable, not merely readable: a remote job written
      // afterwards must read back remote.
      await migrated.into(migrated.jobs).insert(
            JobsCompanion.insert(
              type: 'powerOff',
              triggerKind: 'duration',
              triggerMinutes: const Value(30),
              status: 'active',
              targetInstantUtc: Value(DateTime.utc(2026, 8, 6, 13)),
              createdAt: DateTime.utc(2026, 8, 6, 12),
              updatedAt: DateTime.utc(2026, 8, 6, 12),
              origin: const Value('remote'),
            ),
          );
      final afterInsert = await migrated.select(migrated.jobs).get();
      expect(
        JobMapper.toDomain(afterInsert.last).origin,
        JobOrigin.remote,
        reason: 'the appended column must round-trip in a migrated file too',
      );

      await migrated.close();
      await dir.delete(recursive: true);
    });

    test('the migration keeps the trigger_date column from the previous version', () async {
      // The v2 step must not be lost when the v3 step is added: extending onUpgrade rather
      // than replacing it is what keeps a v1 install able to reach v3 intact.
      final fresh = _openTestDb();
      await fresh.into(fresh.jobs).insert(
            JobsCompanion.insert(
              type: 'powerOff',
              triggerKind: 'absoluteTime',
              triggerHour: const Value(22),
              triggerMinute: const Value(30),
              triggerDate: const Value('2026-08-10'),
              status: 'active',
              targetInstantUtc: Value(DateTime.utc(2026, 8, 10, 15, 30)),
              createdAt: DateTime.utc(2026, 8, 6, 12),
              updatedAt: DateTime.utc(2026, 8, 6, 12),
              origin: const Value('remote'),
            ),
          );

      final rows = await fresh.select(fresh.jobs).get();
      expect(rows.first.triggerDate, '2026-08-10');
      expect(rows.first.origin, 'remote');
      await fresh.close();
    });
  });
}

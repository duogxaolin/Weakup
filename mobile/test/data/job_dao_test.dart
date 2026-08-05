import 'package:drift/native.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:weakup/data/app_database.dart';
import 'package:weakup/data/job_dao.dart';
import 'package:weakup/domain/calendar_date.dart';
import 'package:weakup/domain/job.dart';
import 'package:weakup/domain/job_enums.dart';
import 'package:weakup/domain/trigger_spec.dart';

AppDatabase _openTestDb() => AppDatabase(NativeDatabase.memory());

Job _makeJob({
  JobType type = JobType.keepAwake,
  JobStatus status = JobStatus.active,
  TriggerSpec? trigger,
}) {
  final now = DateTime.now().toUtc();
  return Job(
    id: 0, // will be assigned by insert
    type: type,
    trigger: trigger ?? const DurationTrigger(minutes: 30),
    status: status,
    targetInstantUtc: now.add(const Duration(minutes: 30)),
    createdAt: now,
    updatedAt: now,
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

  group('JobDao', () {
    test('insert and watch all', () async {
      final job = _makeJob();
      final result = await dao.insertJob(job);
      expect(result.isSuccess, isTrue);

      final all = await dao.watchAll().first;
      expect(all.length, 1);
      expect(all.first.id, isNonZero);
      expect(all.first.type, JobType.keepAwake);
    });

    test('updateJobStatus changes status', () async {
      final inserted = (await dao.insertJob(_makeJob())).valueOrNull!;
      await dao.updateJobStatus(inserted.id, JobStatus.paused);

      final all = await dao.watchAll().first;
      expect(all.first.status, JobStatus.paused);
    });

    test('deleteJob removes the job', () async {
      final inserted = (await dao.insertJob(_makeJob())).valueOrNull!;
      await dao.deleteJob(inserted.id);

      final all = await dao.watchAll().first;
      expect(all, isEmpty);
    });

    test('watchActiveByType filters by type and status', () async {
      await dao.insertJob(_makeJob(type: JobType.keepAwake));
      await dao.insertJob(_makeJob(type: JobType.powerOff));
      await dao.insertJob(_makeJob(type: JobType.keepAwake, status: JobStatus.completed));

      final active = await dao.watchActiveByType(JobType.keepAwake).first;
      expect(active.length, 1);
      expect(active.first.type, JobType.keepAwake);
    });

    group('replaceActiveJobOfType', () {
      test('cancels existing active job and inserts new one atomically', () async {
        final old = _makeJob(type: JobType.powerOff);
        final oldInserted = (await dao.insertJob(old)).valueOrNull!;

        final newJob = _makeJob(type: JobType.powerOff);
        final replaceResult = await dao.replaceActiveJobOfType(JobType.powerOff, newJob);

        expect(replaceResult.isSuccess, isTrue);

        final all = await dao.watchAll().first;

        // Old job should now be cancelled.
        final oldRow = all.firstWhere((j) => j.id == oldInserted.id);
        expect(oldRow.status, JobStatus.cancelled);

        // Only one active powerOff job should exist.
        final activePowerOff = all
            .where((j) => j.type == JobType.powerOff && j.status == JobStatus.active)
            .toList();
        expect(activePowerOff.length, 1,
            reason: 'Exactly one active powerOff job must exist after replace');

        // No intermediate state with zero or two active jobs is possible because
        // the transaction ensures cancel+insert are atomic.
      });

      test('when no existing active job, simply inserts', () async {
        final newJob = _makeJob(type: JobType.powerOff);
        final result = await dao.replaceActiveJobOfType(JobType.powerOff, newJob);
        expect(result.isSuccess, isTrue);

        final active = await dao.watchActiveByType(JobType.powerOff).first;
        expect(active.length, 1);
      });

      test('does not affect jobs of other types', () async {
        await dao.insertJob(_makeJob(type: JobType.keepAwake));
        await dao.insertJob(_makeJob(type: JobType.powerOff));

        final newPowerOff = _makeJob(type: JobType.powerOff);
        await dao.replaceActiveJobOfType(JobType.powerOff, newPowerOff);

        final activeKeepAwake = await dao.watchActiveByType(JobType.keepAwake).first;
        expect(activeKeepAwake.length, 1,
            reason: 'keepAwake job should be unaffected');
      });
    });

    test('getActiveJobs returns only active jobs', () async {
      await dao.insertJob(_makeJob(status: JobStatus.active));
      await dao.insertJob(_makeJob(status: JobStatus.completed));
      await dao.insertJob(_makeJob(status: JobStatus.cancelled));

      final result = await dao.getActiveJobs();
      expect(result.isSuccess, isTrue);
      expect(result.valueOrNull!.length, 1);
    });

    group('trigger round-trip', () {
      // The mapper writes each trigger to a different subset of columns, so a
      // dropped field shows up as a job that reloads meaning something else — a
      // dated shutdown silently becoming a daily alarm, for instance.
      test('an undated absolute trigger reloads with no date', () async {
        final inserted = (await dao.insertJob(_makeJob(
          type: JobType.powerOff,
          trigger: const AbsoluteTimeTrigger(hour: 23, minute: 30),
        )))
            .valueOrNull!;

        final loaded = (await dao.watchAll().first)
            .firstWhere((j) => j.id == inserted.id);
        expect(loaded.trigger, const AbsoluteTimeTrigger(hour: 23, minute: 30));
        expect((loaded.trigger as AbsoluteTimeTrigger).date, isNull);
      });

      test('a dated absolute trigger reloads with its date intact', () async {
        final trigger = AbsoluteTimeTrigger(
          hour: 22,
          minute: 30,
          date: CalendarDate(year: 2026, month: 8, day: 10),
        );
        final inserted =
            (await dao.insertJob(_makeJob(type: JobType.powerOff, trigger: trigger)))
                .valueOrNull!;

        final loaded = (await dao.watchAll().first)
            .firstWhere((j) => j.id == inserted.id);
        expect(loaded.trigger, trigger);
      });

      test('the other trigger kinds still round-trip', () async {
        for (final trigger in const [
          IndefiniteTrigger(),
          DurationTrigger(minutes: 120),
        ]) {
          final inserted = (await dao.insertJob(_makeJob(trigger: trigger))).valueOrNull!;
          final loaded = (await dao.watchAll().first)
              .firstWhere((j) => j.id == inserted.id);
          expect(loaded.trigger, trigger, reason: 'for $trigger');
        }
      });
    });
  });
}

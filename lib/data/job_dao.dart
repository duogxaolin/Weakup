import 'package:drift/drift.dart';

import '../core/app_error.dart';
import '../core/result.dart';
import '../domain/job.dart';
import '../domain/job_enums.dart';
import 'app_database.dart';
import 'job_mapper.dart';
import 'tables.dart';

part 'job_dao.g.dart';

@DriftAccessor(tables: [Jobs])
class JobDao extends DatabaseAccessor<AppDatabase> with _$JobDaoMixin {
  JobDao(super.db);

  // ---- streams ----

  Stream<List<Job>> watchAll() {
    return (select(jobs)
          ..orderBy([
            (j) => OrderingTerm.desc(j.updatedAt),
          ]))
        .watch()
        .map((rows) => rows.map(JobMapper.toDomain).toList());
  }

  Stream<List<Job>> watchActiveByType(JobType type) {
    return (select(jobs)
          ..where((j) =>
              j.type.equals(type.name) & j.status.equals(JobStatus.active.name)))
        .watch()
        .map((rows) => rows.map(JobMapper.toDomain).toList());
  }

  // ---- queries ----

  Future<Result<List<Job>>> getActiveJobs() async {
    try {
      final rows = await (select(jobs)
            ..where((j) => j.status.equals(JobStatus.active.name)))
          .get();
      return Result.success(rows.map(JobMapper.toDomain).toList());
    } catch (e) {
      return Result.failure(StorageError(message: e.toString()));
    }
  }

  // ---- mutations ----

  Future<Result<Job>> insertJob(Job job) async {
    try {
      final companion = JobMapper.toCompanion(job);
      final id = await into(jobs).insert(companion);
      final inserted = job.copyWith(id: id);
      return Result.success(inserted);
    } catch (e) {
      return Result.failure(StorageError(message: e.toString()));
    }
  }

  Future<Result<void>> updateJobStatus(
    int id,
    JobStatus status, {
    String? failureMessage,
  }) async {
    try {
      await (update(jobs)..where((j) => j.id.equals(id))).write(
        JobsCompanion(
          status: Value(status.name),
          updatedAt: Value(DateTime.now().toUtc()),
          failureMessage: Value(failureMessage),
        ),
      );
      return Result.success(null);
    } catch (e) {
      return Result.failure(StorageError(message: e.toString()));
    }
  }

  Future<Result<void>> updateJobTarget(int id, DateTime targetInstantUtc) async {
    try {
      await (update(jobs)..where((j) => j.id.equals(id))).write(
        JobsCompanion(
          targetInstantUtc: Value(targetInstantUtc),
          updatedAt: Value(DateTime.now().toUtc()),
        ),
      );
      return Result.success(null);
    } catch (e) {
      return Result.failure(StorageError(message: e.toString()));
    }
  }

  Future<Result<void>> deleteJob(int id) async {
    try {
      await (delete(jobs)..where((j) => j.id.equals(id))).go();
      return Result.success(null);
    } catch (e) {
      return Result.failure(StorageError(message: e.toString()));
    }
  }

  /// Atomically cancels any existing active job of [type] and inserts [newJob].
  Future<Result<Job>> replaceActiveJobOfType(JobType type, Job newJob) async {
    try {
      late Job inserted;
      await transaction(() async {
        // Cancel any active jobs of this type.
        await (update(jobs)
              ..where((j) =>
                  j.type.equals(type.name) &
                  j.status.equals(JobStatus.active.name)))
            .write(
          JobsCompanion(
            status: Value(JobStatus.cancelled.name),
            updatedAt: Value(DateTime.now().toUtc()),
          ),
        );
        // Insert the new job.
        final companion = JobMapper.toCompanion(newJob);
        final id = await into(jobs).insert(companion);
        inserted = newJob.copyWith(id: id);
      });
      return Result.success(inserted);
    } catch (e) {
      return Result.failure(StorageError(message: e.toString()));
    }
  }
}

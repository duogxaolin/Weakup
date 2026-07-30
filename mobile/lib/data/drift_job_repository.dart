import '../core/result.dart';
import '../domain/job.dart';
import '../domain/job_enums.dart';
import '../domain/job_repository.dart';
import 'app_database.dart';
import 'job_dao.dart';

class DriftJobRepository implements JobRepository {
  DriftJobRepository(this._db) : _dao = JobDao(_db);

  final AppDatabase _db;
  final JobDao _dao;

  @override
  Stream<List<Job>> watchAll() => _dao.watchAll();

  @override
  Stream<List<Job>> watchActiveByType(JobType type) =>
      _dao.watchActiveByType(type);

  @override
  Future<Result<List<Job>>> getActiveJobs() => _dao.getActiveJobs();

  @override
  Future<Result<Job>> insert(Job job) => _dao.insertJob(job);

  @override
  Future<Result<void>> updateStatus(
    int id,
    JobStatus status, {
    String? failureMessage,
  }) =>
      _dao.updateJobStatus(id, status, failureMessage: failureMessage);

  @override
  Future<Result<void>> updateTarget(int id, DateTime targetInstantUtc) =>
      _dao.updateJobTarget(id, targetInstantUtc);

  @override
  Future<Result<void>> delete(int id) => _dao.deleteJob(id);

  @override
  Future<Result<Job>> replaceActiveJobOfType(JobType type, Job newJob) =>
      _dao.replaceActiveJobOfType(type, newJob);

  @override
  Future<void> close() => _db.close();
}

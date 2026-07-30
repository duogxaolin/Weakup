import '../core/result.dart';
import 'job.dart';
import 'job_enums.dart';

/// Interface for all job persistence operations.
/// The Drift-backed implementation lives in [lib/data/].
abstract interface class JobRepository {
  /// Stream of all jobs, most recently updated first.
  Stream<List<Job>> watchAll();

  /// Stream of active jobs of [type].
  Stream<List<Job>> watchActiveByType(JobType type);

  /// Returns the current list of all active jobs (non-streaming snapshot).
  Future<Result<List<Job>>> getActiveJobs();

  /// Inserts a new job. Returns the inserted [Job] with its assigned id.
  Future<Result<Job>> insert(Job job);

  /// Updates the status of an existing job.
  Future<Result<void>> updateStatus(
    int id,
    JobStatus status, {
    String? failureMessage,
  });

  /// Updates the target instant of an existing job.
  Future<Result<void>> updateTarget(int id, DateTime targetInstantUtc);

  /// Deletes a job by id.
  Future<Result<void>> delete(int id);

  /// Atomically cancels any existing active job of [type] and inserts [newJob].
  /// Ensures exactly one active job of [type] exists at all times.
  Future<Result<Job>> replaceActiveJobOfType(JobType type, Job newJob);

  /// Close the underlying database connection.
  Future<void> close();
}

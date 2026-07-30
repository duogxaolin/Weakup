use chrono::{DateTime, Utc};

use crate::core::AppResult;
use crate::domain::{Job, JobStatus, JobType};

/// Everything the scheduler needs from persistence.
///
/// A trait rather than a concrete type so the scheduler can be tested against an
/// in-memory double, and so a future storage change does not reach into
/// `application/`.
///
/// Mirrors `JobRepository` in `mobile/lib/domain/`, minus the reactive streams:
/// on desktop the front end is refreshed by Tauri events rather than by watching
/// the database.
pub trait JobRepository: Send + Sync {
    /// All jobs, most recently updated first.
    fn list_all(&self) -> AppResult<Vec<Job>>;

    /// Jobs that still hold the active slot for their type — `Active` or `Paused`
    /// (see `JobStatus::occupies_active_slot`). This is what reconciliation reads
    /// on startup.
    fn list_occupying_active_slot(&self) -> AppResult<Vec<Job>>;

    /// A single job by id, or `None` if it does not exist.
    fn find(&self, id: &str) -> AppResult<Option<Job>>;

    /// Inserts a new job. The caller supplies the id.
    fn insert(&self, job: &Job) -> AppResult<()>;

    /// Sets status, and `failure_message` alongside it.
    ///
    /// The message is passed explicitly rather than defaulted so that clearing it
    /// on a transition out of `Failed` cannot be forgotten.
    fn update_status(
        &self,
        id: &str,
        status: JobStatus,
        failure_message: Option<&str>,
    ) -> AppResult<()>;

    /// Sets the target instant, used when an absolute-time job is re-resolved
    /// after a timezone change.
    fn update_target(&self, id: &str, target_instant_utc: DateTime<Utc>) -> AppResult<()>;

    fn delete(&self, id: &str) -> AppResult<()>;

    /// Cancels whatever job currently holds the active slot for `job_type` and
    /// inserts `new_job`, in one transaction.
    ///
    /// Atomicity is the point: a crash between the two halves would otherwise
    /// leave the type with two active jobs (two competing power-off timers) or
    /// none (a job the user created silently absent).
    fn replace_active_job_of_type(&self, job_type: JobType, new_job: &Job) -> AppResult<()>;
}

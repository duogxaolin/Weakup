//! An in-memory [`JobRepository`] for scheduler tests.
//!
//! The SQLite repository is tested against SQLite in `data/sqlite_repository_tests.rs`.
//! Re-testing storage through the scheduler would make scheduler failures ambiguous,
//! so the scheduler is tested against this instead, and gets one thing SQLite cannot
//! easily give: an injectable write failure, to check that a storage error is
//! reported rather than swallowed.

use std::sync::Mutex;

use chrono::{DateTime, Utc};

use crate::core::{AppError, AppResult};
use crate::data::JobRepository;
use crate::domain::{Job, JobStatus, JobType};

#[derive(Default)]
pub struct FakeJobRepository {
    jobs: Mutex<Vec<Job>>,
    fail_writes_with: Mutex<Option<AppError>>,
}

impl FakeJobRepository {
    pub fn new() -> Self {
        Self::default()
    }

    /// Seeds jobs as if they had been loaded from disk on startup.
    pub fn with_jobs(jobs: Vec<Job>) -> Self {
        Self {
            jobs: Mutex::new(jobs),
            fail_writes_with: Mutex::new(None),
        }
    }

    /// Makes every subsequent write fail, to test error propagation.
    pub fn fail_writes(&self, error: AppError) {
        *self.fail_writes_with.lock().unwrap() = Some(error);
    }

    pub fn stop_failing(&self) {
        *self.fail_writes_with.lock().unwrap() = None;
    }

    pub fn snapshot(&self) -> Vec<Job> {
        self.jobs.lock().unwrap().clone()
    }

    pub fn status_of(&self, id: &str) -> Option<JobStatus> {
        self.jobs
            .lock()
            .unwrap()
            .iter()
            .find(|job| job.id == id)
            .map(|job| job.status)
    }

    pub fn find_target(&self, id: &str) -> Option<DateTime<Utc>> {
        self.jobs
            .lock()
            .unwrap()
            .iter()
            .find(|job| job.id == id)
            .and_then(|job| job.target_instant_utc)
    }

    pub fn failure_message_of(&self, id: &str) -> Option<String> {
        self.jobs
            .lock()
            .unwrap()
            .iter()
            .find(|job| job.id == id)
            .and_then(|job| job.failure_message.clone())
    }

    fn check_writable(&self) -> AppResult<()> {
        match self.fail_writes_with.lock().unwrap().clone() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

impl JobRepository for FakeJobRepository {
    fn list_all(&self) -> AppResult<Vec<Job>> {
        Ok(self.jobs.lock().unwrap().clone())
    }

    fn list_occupying_active_slot(&self) -> AppResult<Vec<Job>> {
        Ok(self
            .jobs
            .lock()
            .unwrap()
            .iter()
            .filter(|job| job.occupies_active_slot())
            .cloned()
            .collect())
    }

    fn find(&self, id: &str) -> AppResult<Option<Job>> {
        Ok(self
            .jobs
            .lock()
            .unwrap()
            .iter()
            .find(|job| job.id == id)
            .cloned())
    }

    fn insert(&self, job: &Job) -> AppResult<()> {
        self.check_writable()?;
        self.jobs.lock().unwrap().push(job.clone());
        Ok(())
    }

    fn update_status(
        &self,
        id: &str,
        status: JobStatus,
        failure_message: Option<&str>,
    ) -> AppResult<()> {
        self.check_writable()?;
        let mut jobs = self.jobs.lock().unwrap();
        match jobs.iter_mut().find(|job| job.id == id) {
            Some(job) => {
                job.status = status;
                job.failure_message = failure_message.map(str::to_string);
                job.updated_at_utc = Utc::now();
                Ok(())
            }
            None => Err(AppError::Storage {
                message: format!("no job with id {id}"),
            }),
        }
    }

    fn update_target(&self, id: &str, target_instant_utc: DateTime<Utc>) -> AppResult<()> {
        self.check_writable()?;
        let mut jobs = self.jobs.lock().unwrap();
        match jobs.iter_mut().find(|job| job.id == id) {
            Some(job) => {
                job.target_instant_utc = Some(target_instant_utc);
                job.updated_at_utc = Utc::now();
                Ok(())
            }
            None => Err(AppError::Storage {
                message: format!("no job with id {id}"),
            }),
        }
    }

    fn delete(&self, id: &str) -> AppResult<()> {
        self.check_writable()?;
        self.jobs.lock().unwrap().retain(|job| job.id != id);
        Ok(())
    }

    fn replace_active_job_of_type(&self, job_type: JobType, new_job: &Job) -> AppResult<()> {
        self.check_writable()?;
        let mut jobs = self.jobs.lock().unwrap();
        for job in jobs.iter_mut() {
            if job.job_type == job_type && job.occupies_active_slot() {
                job.status = JobStatus::Cancelled;
                job.updated_at_utc = Utc::now();
            }
        }
        jobs.push(new_job.clone());
        Ok(())
    }
}

//! Persistence. Depends on `domain` and `core`; nothing here knows about Tauri
//! or the web view.
//!
//! `rusqlite` is used directly rather than exposing SQL to the front end (D17),
//! so the scheduler stays the only writer of job state.

mod job_repository;
mod sqlite_repository;

pub use job_repository::JobRepository;
pub use sqlite_repository::SqliteJobRepository;

#[cfg(test)]
mod sqlite_repository_tests;

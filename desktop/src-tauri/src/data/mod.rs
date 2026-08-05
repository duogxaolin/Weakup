//! Persistence. Depends on `domain` and `core`; nothing here knows about Tauri
//! or the web view.
//!
//! `rusqlite` is used directly rather than exposing SQL to the front end (D17),
//! so the scheduler stays the only writer of job state.

mod job_repository;
mod settings;
mod sqlite_repository;

pub use job_repository::JobRepository;
pub use settings::{default_timezone, Language, Settings, SettingsStore, Theme};
pub use sqlite_repository::SqliteJobRepository;

#[cfg(test)]
mod sqlite_repository_tests;

use crate::core::AppError;

/// Lives here rather than in `core` deliberately. `core` documents itself as having
/// no dependencies on anything else, and a conversion from the SQLite driver's error
/// type is a dependency on the storage engine.
///
/// It also has a practical payoff: with `core` free of `rusqlite`, the platform code
/// can be type-checked for Windows and Linux without a cross C toolchain for bundled
/// SQLite. See `desktop/platform-check`.
impl From<rusqlite::Error> for AppError {
    fn from(err: rusqlite::Error) -> Self {
        Self::Storage {
            message: err.to_string(),
        }
    }
}

#[cfg(test)]
mod error_conversion_tests {
    use crate::core::AppError;

    #[test]
    fn sqlite_errors_convert_into_storage_errors() {
        let err: AppError = rusqlite::Error::QueryReturnedNoRows.into();
        assert!(matches!(err, AppError::Storage { .. }));
    }
}

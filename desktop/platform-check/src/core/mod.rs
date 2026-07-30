//! The real `core::error`, so the platform modules' `crate::core::` paths resolve
//! unchanged.

#[path = "../../../src-tauri/src/core/error.rs"]
mod error;

pub use error::{AppError, AppResult};

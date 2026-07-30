//! Error and result types. This module has no dependencies on any other module in
//! the crate; everything else may depend on it.

mod error;

pub use error::{AppError, AppResult};

//! Pure job model and trigger resolution. Depends only on `core`, never on
//! persistence, platform, or Tauri. These are the modules the shared cross-language
//! test vectors exercise.

mod job;
mod job_enums;
mod trigger_resolver;
mod trigger_spec;

#[cfg(test)]
mod trigger_resolver_tests;

pub use job::Job;
pub use job_enums::{JobStatus, JobType};
pub use trigger_resolver::{ReconcileOutcome, TriggerResolver, POWER_OFF_OVERTOLERANCE_MINUTES};
pub use trigger_spec::TriggerSpec;

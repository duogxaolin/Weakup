//! Pure job model and trigger resolution. Depends only on `core`, never on
//! persistence, platform, or Tauri. These are the modules the shared cross-language
//! test vectors exercise.

mod command_acceptance;
mod command_envelope;
mod device_id;
mod job;
mod job_enums;
mod pairing_grant;
mod presence;
mod remote_command;
mod trigger_resolver;
mod trigger_spec;

#[cfg(test)]
mod command_acceptance_tests;
#[cfg(test)]
mod remote_command_tests;
#[cfg(test)]
mod trigger_resolver_tests;

pub use command_acceptance::{
    evaluate_command, CommandAcceptance, CommandTargetState, RejectionReason,
    FRESHNESS_WINDOW_SECONDS, FUTURE_TOLERANCE_SECONDS, NONCE_RETENTION_SECONDS,
};
pub use command_envelope::CommandEnvelope;
pub use device_id::DeviceId;
pub use job::Job;
pub use job_enums::{JobOrigin, JobStatus, JobType};
pub use pairing_grant::{
    evaluate_grant, GrantDelivery, GrantRejection, GrantValidity, PairingGrant,
    AT_MACHINE_GRANT_LIFETIME_SECONDS, OUT_OF_BAND_GRANT_LIFETIME_SECONDS,
};
pub use presence::{
    evaluate_presence, PresenceEvaluation, PresenceState, OFFLINE_THRESHOLD_SECONDS,
    ONLINE_THRESHOLD_SECONDS,
};
pub use remote_command::{
    authorize, DenialReason, RemoteCommand, RemoteCommandContext, RemoteCommandDecision,
};
pub use trigger_resolver::{ReconcileOutcome, TriggerResolver, POWER_OFF_OVERTOLERANCE_MINUTES};
pub use trigger_spec::TriggerSpec;

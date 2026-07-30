use serde::{Deserialize, Serialize};

/// The two job types the scheduler supports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum JobType {
    KeepAwake,
    PowerOff,
}

impl JobType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::KeepAwake => "keepAwake",
            Self::PowerOff => "powerOff",
        }
    }

    pub fn from_str_value(value: &str) -> Option<Self> {
        match value {
            "keepAwake" => Some(Self::KeepAwake),
            "powerOff" => Some(Self::PowerOff),
            _ => None,
        }
    }
}

/// All possible states a job can be in.
///
/// `Degraded` is retained from the mobile implementation even though its only
/// producer there is the Android 15 foreground-service cap. Keeping the variant
/// means a job row written by either implementation stays readable, and the desktop
/// build can use it for its own degraded-runtime cases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum JobStatus {
    /// Scheduled and actively running.
    Active,
    /// Paused by the user; OS resources released but the job retained.
    Paused,
    /// Completed normally because the trigger condition was met.
    Completed,
    /// Cancelled by the user.
    Cancelled,
    /// The power-off executor returned an error, or the scheduler hit a fatal issue.
    Failed,
    /// The power-off target passed by more than the tolerance; execution was skipped.
    Overdue,
    /// A runtime limit degraded the job while it was pending.
    Degraded,
}

impl JobStatus {
    /// Whether a job in this state still occupies the single active slot for its
    /// type. Only `Active` and `Paused` do: a paused job is retained and can be
    /// resumed, so creating another job of the same type must still replace it.
    pub fn occupies_active_slot(self) -> bool {
        matches!(self, Self::Active | Self::Paused)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Paused => "paused",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
            Self::Overdue => "overdue",
            Self::Degraded => "degraded",
        }
    }

    pub fn from_str_value(value: &str) -> Option<Self> {
        match value {
            "active" => Some(Self::Active),
            "paused" => Some(Self::Paused),
            "completed" => Some(Self::Completed),
            "cancelled" => Some(Self::Cancelled),
            "failed" => Some(Self::Failed),
            "overdue" => Some(Self::Overdue),
            "degraded" => Some(Self::Degraded),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_and_paused_occupy_the_active_slot() {
        assert!(JobStatus::Active.occupies_active_slot());
        assert!(JobStatus::Paused.occupies_active_slot());
    }

    #[test]
    fn terminal_states_do_not_occupy_the_active_slot() {
        for status in [
            JobStatus::Completed,
            JobStatus::Cancelled,
            JobStatus::Failed,
            JobStatus::Overdue,
            JobStatus::Degraded,
        ] {
            assert!(
                !status.occupies_active_slot(),
                "{status:?} must not hold the active slot"
            );
        }
    }

    #[test]
    fn status_strings_round_trip() {
        for status in [
            JobStatus::Active,
            JobStatus::Paused,
            JobStatus::Completed,
            JobStatus::Cancelled,
            JobStatus::Failed,
            JobStatus::Overdue,
            JobStatus::Degraded,
        ] {
            assert_eq!(JobStatus::from_str_value(status.as_str()), Some(status));
        }
    }

    #[test]
    fn job_type_strings_round_trip_and_match_the_dart_wire_names() {
        // These exact strings appear in the shared test vectors, so they are a
        // cross-language contract rather than an internal detail.
        assert_eq!(JobType::KeepAwake.as_str(), "keepAwake");
        assert_eq!(JobType::PowerOff.as_str(), "powerOff");
        assert_eq!(
            JobType::from_str_value("keepAwake"),
            Some(JobType::KeepAwake)
        );
        assert_eq!(JobType::from_str_value("nonsense"), None);
    }
}

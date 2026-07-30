use serde::{Deserialize, Serialize};

/// Maximum accepted duration, in minutes (24 hours).
pub const MAX_DURATION_MINUTES: i64 = 1440;

/// What condition fires a job.
///
/// The `kind` tag values match the Dart implementation's wire names, because the
/// shared test vectors are parsed by both.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TriggerSpec {
    /// Runs until cancelled manually. Valid only for `JobType::KeepAwake`.
    Indefinite,

    /// Fires `minutes` after activation. Must be in 1..=1440.
    Duration { minutes: i64 },

    /// Fires at a wall-clock time, resolved to the next future occurrence in the
    /// device's timezone.
    AbsoluteTime { hour: u32, minute: u32 },
}

impl TriggerSpec {
    pub fn kind_str(&self) -> &'static str {
        match self {
            Self::Indefinite => "indefinite",
            Self::Duration { .. } => "duration",
            Self::AbsoluteTime { .. } => "absoluteTime",
        }
    }

    /// Whether this trigger yields a concrete target instant. `Indefinite` does not,
    /// which is why power-off rejects it.
    pub fn has_target_instant(&self) -> bool {
        !matches!(self, Self::Indefinite)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_indefinite_lacks_a_target_instant() {
        assert!(!TriggerSpec::Indefinite.has_target_instant());
        assert!(TriggerSpec::Duration { minutes: 30 }.has_target_instant());
        assert!(TriggerSpec::AbsoluteTime { hour: 23, minute: 30 }.has_target_instant());
    }

    #[test]
    fn kind_names_match_the_shared_vector_wire_format() {
        assert_eq!(TriggerSpec::Indefinite.kind_str(), "indefinite");
        assert_eq!(TriggerSpec::Duration { minutes: 1 }.kind_str(), "duration");
        assert_eq!(
            TriggerSpec::AbsoluteTime { hour: 0, minute: 0 }.kind_str(),
            "absoluteTime"
        );
    }

    #[test]
    fn triggers_round_trip_through_serde() {
        for trigger in [
            TriggerSpec::Indefinite,
            TriggerSpec::Duration { minutes: 120 },
            TriggerSpec::AbsoluteTime { hour: 6, minute: 5 },
        ] {
            let json = serde_json::to_string(&trigger).expect("serialize");
            let parsed: TriggerSpec = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(trigger, parsed);
        }
    }
}

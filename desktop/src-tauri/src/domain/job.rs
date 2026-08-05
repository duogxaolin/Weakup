use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::job_enums::{JobStatus, JobType};
use crate::domain::trigger_spec::TriggerSpec;

/// A scheduled job.
///
/// `target_instant_utc` is the absolute instant the job fires, stored alongside the
/// original `trigger` that produced it. Remaining time is always computed as
/// `target - now` at display time; a decrementing counter is never the source of
/// truth, because process suspension and clock changes make it wrong.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub id: String,
    pub job_type: JobType,
    pub trigger: TriggerSpec,
    pub status: JobStatus,
    /// `None` only for an indefinite keep-awake job.
    pub target_instant_utc: Option<DateTime<Utc>>,
    pub created_at_utc: DateTime<Utc>,
    /// Last state change. The job list orders by this, so it is not
    /// interchangeable with `created_at_utc`.
    pub updated_at_utc: DateTime<Utc>,
    /// IANA timezone name in effect when the job was created, retained so an
    /// absolute-time job can be re-resolved after a timezone change.
    pub timezone: String,
    /// Set when `status` is `Failed`. A power-off can fail for reasons the user
    /// must be able to tell apart — privileges, consent, or policy — so the
    /// reason travels with the job rather than being lost at the process boundary.
    pub failure_message: Option<String>,
}

impl Job {
    /// Remaining time until the target instant, or `None` for an indefinite job.
    /// Negative when the target has already passed, which the reconciler relies on.
    pub fn remaining(&self, now: DateTime<Utc>) -> Option<chrono::Duration> {
        self.target_instant_utc
            .map(|target| target.signed_duration_since(now))
    }

    /// Whether this job still holds the single active slot for its type.
    pub fn occupies_active_slot(&self) -> bool {
        self.status.occupies_active_slot()
    }

    /// Whether a timezone change should cause this job's target to be recomputed.
    /// Only absolute wall-clock times move; a duration was anchored at creation.
    pub fn is_timezone_sensitive(&self) -> bool {
        matches!(self.trigger, TriggerSpec::AbsoluteTime { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn utc(y: i32, m: u32, d: u32, h: u32, min: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, m, d, h, min, 0).unwrap()
    }

    fn job(trigger: TriggerSpec, target: Option<DateTime<Utc>>) -> Job {
        Job {
            id: "job-1".into(),
            job_type: JobType::PowerOff,
            trigger,
            status: JobStatus::Active,
            target_instant_utc: target,
            created_at_utc: utc(2026, 7, 30, 10, 0),
            updated_at_utc: utc(2026, 7, 30, 10, 0),
            timezone: "Asia/Ho_Chi_Minh".into(),
            failure_message: None,
        }
    }

    #[test]
    fn remaining_is_computed_from_the_absolute_target() {
        let j = job(
            TriggerSpec::Duration { minutes: 60 },
            Some(utc(2026, 7, 30, 11, 0)),
        );
        let remaining = j.remaining(utc(2026, 7, 30, 10, 30)).unwrap();
        assert_eq!(remaining.num_minutes(), 30);
    }

    #[test]
    fn remaining_goes_negative_once_the_target_has_passed() {
        // The reconciler depends on this sign, so it is asserted rather than assumed.
        let j = job(
            TriggerSpec::Duration { minutes: 60 },
            Some(utc(2026, 7, 30, 11, 0)),
        );
        let remaining = j.remaining(utc(2026, 7, 30, 11, 20)).unwrap();
        assert_eq!(remaining.num_minutes(), -20);
    }

    #[test]
    fn indefinite_job_has_no_remaining_time() {
        let mut j = job(TriggerSpec::Indefinite, None);
        j.job_type = JobType::KeepAwake;
        assert!(j.remaining(utc(2026, 7, 30, 10, 30)).is_none());
    }

    #[test]
    fn only_absolute_time_jobs_are_timezone_sensitive() {
        assert!(job(
            TriggerSpec::at_time(23, 0),
            Some(utc(2026, 7, 30, 16, 0))
        )
        .is_timezone_sensitive());

        // A dated absolute time is sensitive too, and for the same reason: 22:30 on
        // 10 August is a different instant in a different zone. `is_timezone_sensitive`
        // matches on the variant and ignores its fields, so this needed no change —
        // asserted rather than assumed.
        assert!(job(
            TriggerSpec::on_date(
                chrono::NaiveDate::from_ymd_opt(2026, 8, 10).unwrap(),
                22,
                30
            ),
            Some(utc(2026, 8, 10, 15, 30))
        )
        .is_timezone_sensitive());

        assert!(!job(
            TriggerSpec::Duration { minutes: 60 },
            Some(utc(2026, 7, 30, 11, 0))
        )
        .is_timezone_sensitive());

        assert!(!job(TriggerSpec::Indefinite, None).is_timezone_sensitive());
    }

    #[test]
    fn job_round_trips_through_serde() {
        let original = job(
            TriggerSpec::at_time(6, 30),
            Some(utc(2026, 7, 30, 23, 30)),
        );
        let json = serde_json::to_string(&original).expect("serialize");
        let parsed: Job = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(original, parsed);
    }
}

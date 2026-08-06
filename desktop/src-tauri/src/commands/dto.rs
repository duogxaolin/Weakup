//! The shapes crossing the IPC boundary.
//!
//! Separate types from the domain ones on purpose. A domain `Job` is what the rules
//! operate on; a [`JobView`] is what a web view can render, which means resolved
//! instants, prose labels, and a remaining-seconds figure the UI does not have to
//! compute. That last point is the whole reason this module exists: if the web view
//! derived remaining time itself it would need the trigger rules in JavaScript too,
//! and the two would drift.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::{Job, JobStatus, JobType, TriggerSpec};
use crate::platform::power_off::PowerOffPermission;

/// A job as the UI sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobView {
    pub id: String,
    pub job_type: String,
    pub job_type_label: String,
    pub status: String,
    pub status_label: String,
    pub trigger_label: String,
    /// The absolute instant this job fires, as RFC 3339. `None` for indefinite jobs.
    ///
    /// The single source of truth for the countdown (task 12.2). The UI counts down
    /// against this rather than against a number it decrements, so a tab that was
    /// backgrounded for an hour shows the right figure when it comes back.
    pub target_instant_utc: Option<String>,
    /// Seconds until the target, negative once it has passed, `None` if indefinite.
    pub remaining_seconds: Option<i64>,
    pub timezone: String,
    pub failure_message: Option<String>,
    pub is_active: bool,
}

impl JobView {
    pub fn from_job(job: &Job, now: DateTime<Utc>) -> Self {
        Self {
            id: job.id.clone(),
            job_type: job.job_type.as_str().to_string(),
            job_type_label: job_type_label(job.job_type).to_string(),
            status: job.status.as_str().to_string(),
            status_label: status_label(job.status).to_string(),
            trigger_label: trigger_label(&job.trigger),
            target_instant_utc: job.target_instant_utc.map(|target| target.to_rfc3339()),
            remaining_seconds: job.remaining(now).map(|remaining| remaining.num_seconds()),
            timezone: job.timezone.clone(),
            failure_message: job.failure_message.clone(),
            is_active: job.status == JobStatus::Active,
        }
    }
}

/// A trigger as it arrives from the UI.
///
/// Carries no instant. Task 11.2: the web view describes what the user asked for and
/// the Rust side resolves it, using the same resolver the shared cross-language vectors
/// pin down.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TriggerInput {
    Indefinite,
    #[serde(rename_all = "camelCase")]
    Duration { minutes: i64 },
    #[serde(rename_all = "camelCase")]
    AbsoluteTime {
        hour: u32,
        minute: u32,
        /// Optional. Omitted means the next occurrence of `hour:minute`; supplied
        /// means exactly that date, which is not rolled forward.
        ///
        /// Typed as `NaiveDate` so a malformed date is refused here at the boundary
        /// rather than reaching the resolver — the UI sends whatever is in the date
        /// box without checking it, by design, and gets prose back.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        date: Option<NaiveDate>,
    },
}

impl From<TriggerInput> for TriggerSpec {
    fn from(input: TriggerInput) -> Self {
        match input {
            TriggerInput::Indefinite => TriggerSpec::Indefinite,
            TriggerInput::Duration { minutes } => TriggerSpec::Duration { minutes },
            TriggerInput::AbsoluteTime { hour, minute, date } => {
                TriggerSpec::AbsoluteTime { hour, minute, date }
            }
        }
    }
}

/// What the UI must do next after asking to create a job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "camelCase")]
pub enum CreateJobResponse {
    #[serde(rename_all = "camelCase")]
    Created { job: JobView },
    /// A job of this type is already running; the user must decide (task 10.9).
    #[serde(rename_all = "camelCase")]
    NeedsConfirmation {
        job_type: String,
        existing: Box<JobView>,
        message: String,
    },
}

/// A resolved instant, for the preview the form shows before anything is created.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedTrigger {
    /// `None` for an indefinite trigger, which has no instant by definition.
    pub target_instant_utc: Option<String>,
    pub remaining_seconds: Option<i64>,
    pub timezone: String,
}

/// The countdown before a shutdown, for the UI (task 12.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraceView {
    pub job_id: String,
    pub seconds_remaining: u64,
}

/// The answer to "will a scheduled shutdown be allowed to run?", for the UI.
///
/// Three states rather than a boolean, mirroring
/// [`PowerOffPermission`](crate::platform::power_off::PowerOffPermission). The UI
/// needs to tell them apart: a denial blocks the form and names the settings path,
/// while an unknown lets the user carry on and merely warns. Collapsing them would
/// force the UI to either block working machines or stay silent about broken ones.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionView {
    /// `"granted"`, `"denied"`, or `"unknown"` — a string the UI can switch on.
    pub state: String,
    /// Whether the UI should refuse to create the job. Only a denial sets this.
    pub blocks_scheduling: bool,
    /// Prose for the user. `None` only when permission is granted.
    pub reason: Option<String>,
    /// Whether this platform can raise a consent dialog at all.
    ///
    /// The UI needs this separately from `reason`: the Windows and Linux fallback is
    /// "undetermined, because this platform only reports at shutdown time", which
    /// carries a reason but is *not* a prompt about to appear. Keying the
    /// "macOS will ask for permission" notice on the reason alone would show a
    /// sentence about System Events to a Windows user.
    pub prompts_for_consent: bool,
}

impl From<PowerOffPermission> for PermissionView {
    fn from(permission: PowerOffPermission) -> Self {
        let state = match &permission {
            PowerOffPermission::Granted => "granted",
            PowerOffPermission::Denied { .. } => "denied",
            PowerOffPermission::Unknown { .. } => "unknown",
        };

        Self {
            state: state.to_string(),
            blocks_scheduling: permission.blocks_scheduling(),
            reason: permission.reason().map(str::to_string),
            prompts_for_consent: crate::platform::power_off::platform_prompts_for_consent(),
        }
    }
}

pub fn job_type_label(job_type: JobType) -> &'static str {
    match job_type {
        JobType::KeepAwake => "Keep screen awake",
        JobType::PowerOff => "Shut down",
    }
}

pub fn status_label(status: JobStatus) -> &'static str {
    match status {
        JobStatus::Active => "Running",
        JobStatus::Paused => "Paused",
        JobStatus::Completed => "Finished",
        JobStatus::Cancelled => "Cancelled",
        JobStatus::Failed => "Failed",
        // Not "late": the user needs to know nothing happened, which is the point.
        JobStatus::Overdue => "Missed — not carried out",
        JobStatus::Degraded => "Running with limits",
    }
}

pub fn trigger_label(trigger: &TriggerSpec) -> String {
    match *trigger {
        TriggerSpec::Indefinite => "Until I turn it off".to_string(),
        TriggerSpec::Duration { minutes } => match (minutes / 60, minutes % 60) {
            (0, minutes) => format!("For {minutes} min"),
            (1, 0) => "For 1 hour".to_string(),
            (hours, 0) => format!("For {hours} hours"),
            (hours, minutes) => format!("For {hours} h {minutes} min"),
        },
        TriggerSpec::AbsoluteTime {
            hour,
            minute,
            date: None,
        } => {
            format!("At {hour:02}:{minute:02}")
        }
        // The date is spelled out rather than left implicit: "At 22:30" on a job set
        // eleven days out would read as though it fires tonight.
        TriggerSpec::AbsoluteTime {
            hour,
            minute,
            date: Some(date),
        } => {
            format!("At {hour:02}:{minute:02} on {date}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, TimeZone};

    fn job(trigger: TriggerSpec, target: Option<DateTime<Utc>>) -> Job {
        let now = Utc.with_ymd_and_hms(2026, 7, 30, 12, 0, 0).unwrap();
        Job {
            id: "job-1".to_string(),
            job_type: JobType::PowerOff,
            trigger,
            status: JobStatus::Active,
            target_instant_utc: target,
            created_at_utc: now,
            updated_at_utc: now,
            timezone: "Asia/Ho_Chi_Minh".to_string(),
            failure_message: None,
            origin: crate::domain::JobOrigin::default(),
        }
    }

    #[test]
    fn the_view_carries_the_absolute_target_so_the_ui_never_recomputes_it() {
        let now = Utc.with_ymd_and_hms(2026, 7, 30, 12, 0, 0).unwrap();
        let target = now + Duration::minutes(30);

        let view = JobView::from_job(&job(TriggerSpec::Duration { minutes: 30 }, Some(target)), now);

        assert_eq!(view.remaining_seconds, Some(1800));
        assert_eq!(
            view.target_instant_utc.as_deref(),
            Some(target.to_rfc3339().as_str())
        );
    }

    #[test]
    fn a_passed_target_reports_negative_remaining_rather_than_zero() {
        // Clamping to zero would hide how late a job is, and the UI needs to be able
        // to say "missed by 90 minutes".
        let now = Utc.with_ymd_and_hms(2026, 7, 30, 12, 0, 0).unwrap();
        let target = now - Duration::minutes(90);

        let view = JobView::from_job(&job(TriggerSpec::at_time(2, 0), Some(target)), now);

        assert_eq!(view.remaining_seconds, Some(-5400));
    }

    #[test]
    fn an_indefinite_job_has_no_target_and_no_countdown() {
        let now = Utc.with_ymd_and_hms(2026, 7, 30, 12, 0, 0).unwrap();

        let view = JobView::from_job(&job(TriggerSpec::Indefinite, None), now);

        assert!(view.target_instant_utc.is_none());
        assert!(view.remaining_seconds.is_none());
    }

    #[test]
    fn every_status_has_a_label_and_the_missed_one_says_nothing_happened() {
        for status in [
            JobStatus::Active,
            JobStatus::Paused,
            JobStatus::Completed,
            JobStatus::Cancelled,
            JobStatus::Failed,
            JobStatus::Overdue,
            JobStatus::Degraded,
        ] {
            assert!(!status_label(status).trim().is_empty(), "{status:?}");
        }

        let missed = status_label(JobStatus::Overdue).to_lowercase();
        assert!(
            missed.contains("not carried out"),
            "an overdue shutdown must not read as though it happened: {missed}"
        );
    }

    #[test]
    fn duration_labels_read_like_english() {
        assert_eq!(trigger_label(&TriggerSpec::Duration { minutes: 45 }), "For 45 min");
        assert_eq!(trigger_label(&TriggerSpec::Duration { minutes: 60 }), "For 1 hour");
        assert_eq!(trigger_label(&TriggerSpec::Duration { minutes: 120 }), "For 2 hours");
        assert_eq!(
            trigger_label(&TriggerSpec::Duration { minutes: 150 }),
            "For 2 h 30 min"
        );
    }

    #[test]
    fn absolute_time_labels_are_zero_padded() {
        // "At 9:5" is not a time.
        assert_eq!(trigger_label(&TriggerSpec::at_time(9, 5)), "At 09:05");
    }

    #[test]
    fn a_dated_label_names_the_day_it_fires() {
        // Without the date this would read "At 22:30", which on a job eleven days out
        // reads as though it fires tonight.
        let date = NaiveDate::from_ymd_opt(2026, 8, 10).unwrap();
        assert_eq!(
            trigger_label(&TriggerSpec::on_date(date, 22, 30)),
            "At 22:30 on 2026-08-10"
        );
    }

    #[test]
    fn a_trigger_input_round_trips_through_json_as_the_ui_sends_it() {
        let json = r#"{"kind":"absoluteTime","hour":22,"minute":30}"#;
        let input: TriggerInput = serde_json::from_str(json).unwrap();

        assert_eq!(TriggerSpec::from(input), TriggerSpec::at_time(22, 30));
    }

    #[test]
    fn a_dated_trigger_input_round_trips_and_forwards_the_date() {
        let json = r#"{"kind":"absoluteTime","hour":22,"minute":30,"date":"2026-08-10"}"#;
        let input: TriggerInput = serde_json::from_str(json).unwrap();

        let date = NaiveDate::from_ymd_opt(2026, 8, 10).unwrap();
        assert_eq!(
            TriggerSpec::from(input),
            TriggerSpec::on_date(date, 22, 30)
        );
    }

    #[test]
    fn a_malformed_date_from_the_ui_is_refused_at_the_boundary() {
        // The web view deliberately does not validate the date box, so this is the
        // first place a nonsense value can be caught. It must be an error rather than
        // a nearby real date.
        let json = r#"{"kind":"absoluteTime","hour":22,"minute":30,"date":"2026-02-30"}"#;
        assert!(serde_json::from_str::<TriggerInput>(json).is_err());
    }

    #[test]
    fn a_duration_input_round_trips_through_json() {
        let json = r#"{"kind":"duration","minutes":120}"#;
        let input: TriggerInput = serde_json::from_str(json).unwrap();

        assert_eq!(TriggerSpec::from(input), TriggerSpec::Duration { minutes: 120 });
    }

    #[test]
    fn the_view_serialises_in_camel_case_for_javascript() {
        let now = Utc.with_ymd_and_hms(2026, 7, 30, 12, 0, 0).unwrap();
        let view = JobView::from_job(&job(TriggerSpec::Indefinite, None), now);

        let json = serde_json::to_value(&view).unwrap();
        assert!(json.get("jobTypeLabel").is_some());
        assert!(json.get("remainingSeconds").is_some());
        assert!(
            json.get("job_type_label").is_none(),
            "snake_case keys would silently read as undefined in the UI"
        );
    }
}

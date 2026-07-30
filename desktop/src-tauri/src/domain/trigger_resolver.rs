use chrono::{DateTime, Duration, MappedLocalTime, NaiveDate, TimeZone, Utc};
use chrono_tz::Tz;

use crate::core::{AppError, AppResult};
use crate::domain::job_enums::JobType;
use crate::domain::trigger_spec::{TriggerSpec, MAX_DURATION_MINUTES};

/// How far past its target a `PowerOff` job may be and still execute.
///
/// Beyond this the job is marked overdue and the machine is NOT powered off. A
/// laptop that slept overnight must not shut down the moment it wakes.
pub const POWER_OFF_OVERTOLERANCE_MINUTES: i64 = 15;

/// The outcome of reconciling one overdue job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ReconcileOutcome {
    /// Target has not been reached; the job keeps waiting.
    StillPending,
    /// A keep-awake job whose target passed: complete it and release the assertion.
    Completed,
    /// A power-off job overdue within tolerance: run the grace-period countdown.
    ProceedToGracePeriod,
    /// A power-off job overdue beyond tolerance: do NOT power off; notify the user.
    Overdue,
}

/// Pure trigger validation and resolution. No persistence, platform, or Tauri
/// dependencies, so it is fully unit-testable and is what the shared cross-language
/// vectors exercise.
pub struct TriggerResolver;

impl TriggerResolver {
    /// Validates `trigger` for `job_type`.
    ///
    /// - `Indefinite` is valid only for `KeepAwake`.
    /// - `Duration` minutes must be in 1..=1440.
    /// - `AbsoluteTime` must be a real wall-clock time (hour 0-23, minute 0-59).
    pub fn validate(trigger: &TriggerSpec, job_type: JobType) -> AppResult<()> {
        match *trigger {
            TriggerSpec::Indefinite => {
                if job_type == JobType::PowerOff {
                    Err(AppError::validation(
                        "Power-off requires a duration or a specific time, not indefinite.",
                    ))
                } else {
                    Ok(())
                }
            }
            TriggerSpec::Duration { minutes } => {
                if minutes <= 0 {
                    Err(AppError::validation(
                        "Duration must be positive (at least 1 minute).",
                    ))
                } else if minutes > MAX_DURATION_MINUTES {
                    Err(AppError::validation(
                        "Duration cannot exceed 24 hours (1440 minutes).",
                    ))
                } else {
                    Ok(())
                }
            }
            TriggerSpec::AbsoluteTime { hour, minute } => {
                if hour > 23 {
                    Err(AppError::validation("Hour must be between 0 and 23."))
                } else if minute > 59 {
                    Err(AppError::validation("Minute must be between 0 and 59."))
                } else {
                    Ok(())
                }
            }
        }
    }

    /// Resolves `trigger` to an absolute UTC target instant.
    ///
    /// Returns `Ok(None)` for `Indefinite`, which has no target instant by design.
    ///
    /// `now` is injected rather than read from the clock so that resolution is
    /// deterministic and the DST edge cases are testable.
    pub fn resolve(
        trigger: &TriggerSpec,
        tz: Tz,
        now: DateTime<Utc>,
    ) -> AppResult<Option<DateTime<Utc>>> {
        match *trigger {
            TriggerSpec::Indefinite => Ok(None),

            TriggerSpec::Duration { minutes } => {
                // Anchored at creation time and therefore timezone-independent: a
                // later timezone change must not move it.
                let target = now
                    .checked_add_signed(Duration::minutes(minutes))
                    .ok_or_else(|| AppError::validation("Duration overflows the calendar."))?;
                Ok(Some(target))
            }

            TriggerSpec::AbsoluteTime { hour, minute } => {
                let local_now = now.with_timezone(&tz);
                let today = local_now.date_naive();

                let candidate = Self::resolve_wall_time(tz, today, hour, minute)?;

                // Must be strictly in the future; otherwise roll to the same wall
                // time tomorrow.
                if candidate > now {
                    return Ok(Some(candidate));
                }

                let tomorrow = today
                    .succ_opt()
                    .ok_or_else(|| AppError::validation("Date overflows the calendar."))?;
                let rolled = Self::resolve_wall_time(tz, tomorrow, hour, minute)?;
                Ok(Some(rolled))
            }
        }
    }

    /// Maps a wall-clock time on a given local date to a UTC instant, resolving the
    /// two DST anomalies explicitly rather than relying on a library default:
    ///
    /// - **Fall-back overlap**: the wall time occurs twice. Resolve to the *earlier*
    ///   occurrence, per spec.
    /// - **Spring-forward gap**: the wall time does not exist. Resolve to the instant
    ///   the clock jumps forward to, found by walking forward in wall-clock minutes
    ///   until a real time is found.
    fn resolve_wall_time(
        tz: Tz,
        date: NaiveDate,
        hour: u32,
        minute: u32,
    ) -> AppResult<DateTime<Utc>> {
        let naive = date
            .and_hms_opt(hour, minute, 0)
            .ok_or_else(|| AppError::validation("Invalid time of day."))?;

        match tz.from_local_datetime(&naive) {
            MappedLocalTime::Single(dt) => Ok(dt.with_timezone(&Utc)),

            // Ambiguous returns (earliest, latest); the spec calls for the earlier.
            MappedLocalTime::Ambiguous(earliest, _latest) => Ok(earliest.with_timezone(&Utc)),

            MappedLocalTime::None => {
                // Inside a spring-forward gap. Real gaps are at most a few hours;
                // scanning a bounded window forward finds the first wall time that
                // exists, which is exactly the post-transition instant.
                const MAX_GAP_SCAN_MINUTES: i64 = 24 * 60;
                for offset in 1..=MAX_GAP_SCAN_MINUTES {
                    let probe = naive + Duration::minutes(offset);
                    match tz.from_local_datetime(&probe) {
                        MappedLocalTime::Single(dt) => return Ok(dt.with_timezone(&Utc)),
                        MappedLocalTime::Ambiguous(earliest, _) => {
                            return Ok(earliest.with_timezone(&Utc))
                        }
                        MappedLocalTime::None => continue,
                    }
                }
                Err(AppError::validation(
                    "Could not resolve that time in this timezone.",
                ))
            }
        }
    }

    /// Decides what to do with a job whose state is being reconciled on startup or
    /// after the machine resumes from sleep.
    ///
    /// This is the safety-critical branch: a power-off job that is too stale must not
    /// execute.
    pub fn reconcile(
        job_type: JobType,
        target_instant_utc: Option<DateTime<Utc>>,
        now: DateTime<Utc>,
    ) -> ReconcileOutcome {
        let Some(target) = target_instant_utc else {
            // Indefinite keep-awake: no target, so nothing to reconcile.
            return ReconcileOutcome::StillPending;
        };

        if target > now {
            return ReconcileOutcome::StillPending;
        }

        match job_type {
            JobType::KeepAwake => ReconcileOutcome::Completed,
            JobType::PowerOff => {
                let overdue_by = now.signed_duration_since(target);
                if overdue_by <= Duration::minutes(POWER_OFF_OVERTOLERANCE_MINUTES) {
                    ReconcileOutcome::ProceedToGracePeriod
                } else {
                    ReconcileOutcome::Overdue
                }
            }
        }
    }
}

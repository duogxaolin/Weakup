//! The IPC surface (section 11).
//!
//! Two rules hold across every command here.
//!
//! Triggers are resolved on this side, never in the web view (task 11.2). The UI sends
//! "22:30", and [`crate::domain::TriggerResolver`] — the same code the shared
//! cross-language vectors pin down — turns it into an instant. Resolving in JavaScript
//! would mean a second implementation of the DST and rollover rules.
//!
//! No command powers the machine off (task 11.4). Nothing reachable from here holds a
//! [`PowerOffExecutor`](crate::platform::power_off::PowerOffExecutor): it lives inside
//! the grace-period gate, which only [`JobScheduler`] can drive and only after a
//! countdown. There is a test asserting no command in this file mentions it.

pub mod dto;
pub mod state;

use chrono::Utc;
use tauri::{AppHandle, Manager, Runtime, State};

use crate::application::grace_period::GRACE_PERIOD_SECONDS;
use crate::application::scheduler::{ConfirmationRequired, CreateJobRequest, CreateOutcome};
use crate::data::Settings;
use crate::domain::{JobType, TriggerResolver, TriggerSpec};
use crate::platform::capabilities::{Capability, CapabilityReport};
use crate::platform::power_off::{check_power_off_permission, PowerOffPermission};
use dto::{
    CreateJobResponse, GraceView, JobView, PermissionView, ResolvedTrigger, TriggerInput,
};
use state::AppState;

/// Errors as the web view receives them.
///
/// A string, deliberately: it is [`crate::core::AppError::user_message`], which is
/// written for a person. Serialising the whole variant would push the decision about
/// what to say into the UI, in JavaScript, away from the code that knows what went
/// wrong.
type CommandResult<T> = Result<T, String>;

fn to_command_error(error: crate::core::AppError) -> String {
    log::warn!("command failed: {error}");
    error.user_message()
}

// ---------------------------------------------------------------------------
// Jobs
// ---------------------------------------------------------------------------

/// Creates a job, or asks for confirmation if one of that type is already running.
#[tauri::command]
pub fn create_job(
    state: State<'_, AppState>,
    job_type: String,
    trigger: TriggerInput,
) -> CommandResult<CreateJobResponse> {
    let job_type = parse_job_type(&job_type)?;
    let request = CreateJobRequest {
        job_type,
        trigger: TriggerSpec::from(trigger),
        timezone: state.timezone(),
    };

    let now = Utc::now();
    match state.scheduler().create_job(&request).map_err(to_command_error)? {
        CreateOutcome::Created(job) => Ok(CreateJobResponse::Created {
            job: JobView::from_job(&job, now),
        }),

        CreateOutcome::NeedsConfirmation(ConfirmationRequired::ReplaceActiveJob {
            existing_id,
            ..
        }) => {
            let existing = state
                .scheduler()
                .find_job(&existing_id)
                .map_err(to_command_error)?
                .ok_or_else(|| "the existing job could not be read".to_string())?;

            Ok(CreateJobResponse::NeedsConfirmation {
                job_type: job_type.as_str().to_string(),
                message: format!(
                    "{} is already scheduled. Replacing it will cancel the current one.",
                    dto::job_type_label(job_type)
                ),
                existing: Box::new(JobView::from_job(&existing, now)),
            })
        }
    }
}

/// Creates a job, cancelling the existing one of that type.
///
/// A separate command rather than a `replace: bool` on [`create_job`], so the
/// destructive path cannot be taken by a default or a forgotten argument.
#[tauri::command]
pub fn create_job_replacing(
    state: State<'_, AppState>,
    job_type: String,
    trigger: TriggerInput,
) -> CommandResult<JobView> {
    let request = CreateJobRequest {
        job_type: parse_job_type(&job_type)?,
        trigger: TriggerSpec::from(trigger),
        timezone: state.timezone(),
    };

    let job = state
        .scheduler()
        .create_job_replacing_active(&request)
        .map_err(to_command_error)?;

    Ok(JobView::from_job(&job, Utc::now()))
}

#[tauri::command]
pub fn list_jobs(state: State<'_, AppState>) -> CommandResult<Vec<JobView>> {
    let now = Utc::now();
    Ok(state
        .scheduler()
        .list_jobs()
        .map_err(to_command_error)?
        .iter()
        .map(|job| JobView::from_job(job, now))
        .collect())
}

#[tauri::command]
pub fn pause_job(state: State<'_, AppState>, id: String) -> CommandResult<()> {
    state.scheduler().pause_job(&id).map_err(to_command_error)
}

#[tauri::command]
pub fn resume_job(state: State<'_, AppState>, id: String) -> CommandResult<()> {
    state.scheduler().resume_job(&id).map_err(to_command_error)
}

#[tauri::command]
pub fn cancel_job(state: State<'_, AppState>, id: String) -> CommandResult<()> {
    state.scheduler().cancel_job(&id).map_err(to_command_error)
}

/// Whether any job is still running. The UI asks before quitting (task 9.3).
#[tauri::command]
pub fn has_active_job(state: State<'_, AppState>) -> CommandResult<bool> {
    state.scheduler().has_active_job().map_err(to_command_error)
}

// ---------------------------------------------------------------------------
// Resolution (task 11.2)
// ---------------------------------------------------------------------------

/// Resolves a trigger without creating anything, for the form's preview.
///
/// This is what keeps the trigger rules in one language. The UI shows "will shut down
/// at 22:30 tomorrow" using this, rather than working the date out itself.
#[tauri::command]
pub fn resolve_trigger(
    state: State<'_, AppState>,
    job_type: String,
    trigger: TriggerInput,
) -> CommandResult<ResolvedTrigger> {
    let job_type = parse_job_type(&job_type)?;
    let spec = TriggerSpec::from(trigger);
    TriggerResolver::validate(&spec, job_type).map_err(to_command_error)?;

    let timezone_name = state.timezone();
    let timezone: chrono_tz::Tz = timezone_name
        .parse()
        .map_err(|_| format!("Unknown timezone: {timezone_name}"))?;

    let now = Utc::now();
    let target = TriggerResolver::resolve(&spec, timezone, now).map_err(to_command_error)?;

    Ok(ResolvedTrigger {
        target_instant_utc: target.map(|instant| instant.to_rfc3339()),
        remaining_seconds: target.map(|instant| instant.signed_duration_since(now).num_seconds()),
        timezone: timezone_name,
    })
}

// ---------------------------------------------------------------------------
// The grace period (task 11.5)
// ---------------------------------------------------------------------------

/// The countdown in progress, if any.
#[tauri::command]
pub fn grace_state(state: State<'_, AppState>) -> CommandResult<Option<GraceView>> {
    Ok(state.scheduler().grace_state().map(|grace| GraceView {
        job_id: grace.job_id,
        seconds_remaining: grace.seconds_remaining,
    }))
}

/// Cancels a countdown in progress (task 11.5).
///
/// The one command in this file that changes whether a shutdown happens, and it can
/// only ever prevent one.
#[tauri::command]
pub fn cancel_grace_period(state: State<'_, AppState>) -> CommandResult<()> {
    state.scheduler().cancel_grace_period();
    Ok(())
}

/// The full grace length, so the UI can draw a progress bar without hardcoding 60.
#[tauri::command]
pub fn grace_period_length() -> u64 {
    GRACE_PERIOD_SECONDS
}

// ---------------------------------------------------------------------------
// Capabilities (task 11.3)
// ---------------------------------------------------------------------------

/// Every capability and its state, including what is degraded and why.
#[tauri::command]
pub fn capability_state(state: State<'_, AppState>) -> Vec<CapabilityReport> {
    state.capabilities().report()
}

/// Asks the OS whether a scheduled shutdown will be allowed to run.
///
/// `ask_user` chooses between the two moments this is needed:
///
/// - `false` — a silent report, used to decide whether to *explain* that macOS is
///   about to ask. Raises no dialog, so it is safe to call while the user is only
///   filling in a form.
/// - `true` — may raise the OS consent dialog. Called from the submit path, which
///   is a deliberate user action, and only after the explanation above has been
///   shown. The spec requires that ordering: a consent dialog with no preceding
///   sentence is unexplained.
///
/// Either way this is a preflight, not an attempt: on macOS it goes through
/// `AEDeterminePermissionToAutomateTarget`, which reports on consent *without*
/// sending an Apple Event. It holds no executor and cannot shut anything down, so
/// the grace-period rule is untouched.
///
/// The verdict is mirrored into the capability registry so the dashboard's
/// degradation banner agrees with what the user was just told.
#[tauri::command]
pub fn check_shutdown_permission(
    state: State<'_, AppState>,
    ask_user: bool,
) -> CommandResult<PermissionView> {
    let verdict = check_power_off_permission(ask_user).map_err(to_command_error)?;

    match &verdict {
        PowerOffPermission::Denied { reason } => {
            state
                .capabilities()
                .mark_unavailable(Capability::PowerOff, reason.clone());
        }
        // A grant clears any earlier denial: consent granted in System Settings
        // mid-session should not leave a stale banner claiming it is missing.
        PowerOffPermission::Granted => {
            state.capabilities().mark_available(Capability::PowerOff);
        }
        // Deliberately neither. "Undecided" is not evidence of breakage, and
        // writing it into the registry would show a warning banner on a machine
        // that has simply never been asked.
        PowerOffPermission::Unknown { .. } => {}
    }

    Ok(PermissionView::from(verdict))
}

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> CommandResult<Settings> {
    state.settings().map_err(to_command_error)
}

/// Saves settings and re-resolves absolute-time jobs if the timezone changed.
///
/// Doing both here is what makes the timezone setting mean anything: a stored name
/// that did not move existing jobs would leave a 22:00 shutdown firing at the old
/// hour, which is the bug the setting exists to fix.
#[tauri::command]
pub fn save_settings(state: State<'_, AppState>, settings: Settings) -> CommandResult<usize> {
    settings
        .timezone
        .parse::<chrono_tz::Tz>()
        .map_err(|_| format!("Unknown timezone: {}", settings.timezone))?;

    let previous = state.settings().map_err(to_command_error)?;
    state.save_settings(&settings).map_err(to_command_error)?;

    if previous.timezone == settings.timezone {
        return Ok(0);
    }

    state
        .scheduler()
        .apply_timezone_change(&settings.timezone)
        .map_err(to_command_error)
}

/// The IANA names the UI offers, host timezone first.
#[tauri::command]
pub fn available_timezones() -> Vec<String> {
    let host = crate::data::default_timezone();
    let mut names: Vec<String> = chrono_tz::TZ_VARIANTS
        .iter()
        .map(|zone| zone.name().to_string())
        .filter(|name| name != &host)
        .collect();
    names.sort();
    names.insert(0, host);
    names
}

// ---------------------------------------------------------------------------
// Autostart (tasks 9.5, 9.6)
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn autostart_enabled<R: Runtime>(app: AppHandle<R>) -> CommandResult<bool> {
    crate::platform::autostart::is_enabled(&app).map_err(to_command_error)
}

#[tauri::command]
pub fn set_autostart_enabled<R: Runtime>(app: AppHandle<R>, enabled: bool) -> CommandResult<()> {
    crate::platform::autostart::set_enabled(&app, enabled).map_err(to_command_error)
}

// ---------------------------------------------------------------------------
// Window and quit (tasks 9.2, 9.3)
// ---------------------------------------------------------------------------

/// Hides the window, leaving jobs running (task 9.4).
#[tauri::command]
pub fn hide_to_tray<R: Runtime>(app: AppHandle<R>) -> CommandResult<()> {
    match app.get_webview_window(crate::platform::tray::MAIN_WINDOW_LABEL) {
        Some(window) => {
            crate::platform::tray::hide_window(&window);
            Ok(())
        }
        None => Err("There is no window to hide.".to_string()),
    }
}

/// Quits. Called only after the UI has confirmed with the user (task 9.3).
#[tauri::command]
pub fn quit_app<R: Runtime>(app: AppHandle<R>) -> CommandResult<()> {
    crate::platform::tray::quit_now(&app);
    Ok(())
}


fn parse_job_type(value: &str) -> Result<JobType, String> {
    JobType::from_str_value(value).ok_or_else(|| format!("Unknown job type: {value}"))
}

#[cfg(test)]
mod tests;

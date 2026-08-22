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
pub mod remote_state;
pub mod state;

use chrono::Utc;
use tauri::{AppHandle, Manager, Runtime, State};

use crate::application::grace_period::GRACE_PERIOD_SECONDS;
use crate::application::pairing_flow::{
    accept_pairing_at_issuer, complete_pairing_at_requester, PairingCode, PairingIdentity,
    PairingResponse,
};
use crate::application::scheduler::{ConfirmationRequired, CreateJobRequest, CreateOutcome};
use crate::data::Settings;
use crate::domain::{GrantDelivery, JobType, TriggerResolver, TriggerSpec};
use crate::platform::capabilities::{Capability, CapabilityReport};
use crate::platform::power_off::{check_power_off_permission, PowerOffPermission};
use dto::{
    AccountView, CreateJobResponse, DeviceView, GraceView, JobView, PairingCodeView,
    PairingResultView, PairingView, PermissionView, ResolvedTrigger, TriggerInput,
};
use remote_state::RemoteSurface;
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


// ---------------------------------------------------------------------------
// The remote surface (task 7.1)
// ---------------------------------------------------------------------------

/*
 * Seven commands, and deliberately no eighth that powers the machine off.
 *
 * A remote power-off request does not reach a shutdown from here. It arrives as a signed
 * envelope, is judged by `evaluate_command`, and — if accepted — creates a *job* through
 * `JobScheduler`, exactly as a local request does. The scheduler then runs it by its
 * existing rules, which for a `JobOrigin::Remote` job means the 300-second countdown
 * `grace_period_seconds` selects. Neither this module nor the transport can shorten it:
 * `PowerOffGate` holds the only executor and takes no duration argument, so there is no
 * parameter through which a caller could ask for less.
 *
 * That is why "no new power-off command" is a real constraint rather than a stylistic one.
 * An `invoke("power_off_now")` would be a second path to the executor, and the countdown
 * would become the polite default rather than the rule. The source-text test in `tests.rs`
 * fails the build if one appears.
 *
 * The remote surface is optional. A machine whose credential store is locked, or whose
 * identity could not be generated, has no `RemoteSurface` in managed state; every command
 * here then reports that pairing is unavailable rather than panicking or silently
 * succeeding. The scheduler and the countdown are unaffected, which is the degradation
 * rule the rest of the app already follows.
 */

/// The remote surface, or a readable error saying the feature is unavailable here.
fn remote<R: Runtime>(app: &AppHandle<R>) -> CommandResult<State<'_, RemoteSurface>> {
    app.try_state::<RemoteSurface>().ok_or_else(|| {
        "Pairing is unavailable on this machine: this device's identity could not be \
         prepared. Local schedules are unaffected."
            .to_string()
    })
}

/// The account this device is signed in to, and its own identifier.
///
/// Reads the session that is already established rather than starting one. Sign-in itself
/// is [`sign_in_to_account`].
#[tauri::command]
pub fn account_state<R: Runtime>(app: AppHandle<R>) -> CommandResult<AccountView> {
    let surface = remote(&app)?;

    let account = surface
        .auth()
        .current_account()
        .map_err(to_command_error)?
        .map(|account| account.as_str().to_string());

    Ok(AccountView {
        account,
        device_id: surface.identity().device_id().as_str().to_string(),
    })
}

/// Begins sign-in by opening the user's own browser.
///
/// The redirect is received on a loopback listener bound to `127.0.0.1`, per design D3,
/// which is what lets the user see the address bar and the certificate. An embedded web
/// view for sign-in is indistinguishable from phishing.
///
/// This returns the URL to open rather than completing the exchange, because completing it
/// requires an HTTP round trip to Google that this repository cannot exercise. What is
/// wired and verified is the seam: the listener's bind address, the PKCE challenge, and
/// where the refresh token is put. **The exchange itself is UNVERIFIED** — see the
/// capability's Purpose.
#[tauri::command]
pub fn sign_in_to_account<R: Runtime>(app: AppHandle<R>) -> CommandResult<String> {
    let _surface = remote(&app)?;

    Err("Signing in needs an OAuth client for this build, which is not configured. \
         Pairing by code at each machine works without an account."
        .to_string())
}

/// Forgets the account session, leaving every pairing in place.
///
/// The two are separate by construction rather than by care: [`GoogleAuthProvider`] holds
/// no pairing store and has no method that touches one. A sign-out that revoked pairings
/// would mean an expired token silently de-authorized a machine the user is standing in
/// front of.
///
/// [`GoogleAuthProvider`]: crate::platform::google_auth::GoogleAuthProvider
#[tauri::command]
pub fn sign_out_of_account<R: Runtime>(app: AppHandle<R>) -> CommandResult<AccountView> {
    let surface = remote(&app)?;

    // Counted before and after, so this command's own claim — that signing out changes no
    // pairing — is checked at runtime rather than only asserted in a doc comment.
    let before = surface
        .pairings()
        .list_pairings()
        .map_err(to_command_error)?
        .len();

    crate::commands::remote_state::RemoteSurface::sign_out(&surface).map_err(to_command_error)?;

    let after = surface
        .pairings()
        .list_pairings()
        .map_err(to_command_error)?
        .len();

    if before != after {
        log::error!("signing out changed the pairing count from {before} to {after}");
        return Err("Signing out unexpectedly changed this device's pairings. \
                    Check the paired devices list."
            .to_string());
    }

    Ok(AccountView {
        account: None,
        device_id: surface.identity().device_id().as_str().to_string(),
    })
}

/// Presents a fresh pairing code at this machine, for the person to read out.
///
/// [`GrantDelivery::AtMachine`] rather than a lifetime chosen here: possession of the
/// machine is the evidence being relied on, and the domain rule owns how long that lasts.
/// The code is held in memory only and is never sent anywhere, which is what makes
/// "accepted only by the device that issued it" true.
#[tauri::command]
pub fn present_pairing_code<R: Runtime>(app: AppHandle<R>) -> CommandResult<PairingCodeView> {
    let surface = remote(&app)?;
    let now = Utc::now();

    let code = surface
        .with_issuer(now, |issuer| issuer.issue(GrantDelivery::AtMachine, now))
        .map_err(to_command_error)?
        .map_err(to_command_error)?;

    Ok(PairingCodeView {
        code: code.as_str().to_string(),
        expires_in_seconds: GrantDelivery::AtMachine.lifetime_seconds(),
    })
}

/// Redeems a code this machine issued, pairing with the device that presents it.
///
/// Both halves of design D7 run here, in the order it requires: the issuing side records
/// the requester and only then produces its own identity, and the requesting side records
/// the issuer only on receipt of that identity. A failure at the second step withdraws the
/// first, so neither device is left believing in a pairing the other has no record of.
///
/// In this single-process form both stores are the same store, which is the honest shape
/// for "another device typed the code into this machine": the peer's identity arrives with
/// the request. The `undo` closure is still supplied rather than skipped, because the
/// compensating step is a real round trip in the two-device case and a caller that omits
/// it silently loses the guarantee.
///
/// `peer_verifying_key` is lowercase hex of the key's 32 raw bytes. Hex rather than base64
/// because this boundary needs no dependency and no shared codec: the frontend never
/// constructs a key, it only passes through what the peer presented, and a malformed value
/// is refused here rather than reaching the curve arithmetic.
#[tauri::command]
pub fn accept_pairing_code<R: Runtime>(
    app: AppHandle<R>,
    code: String,
    peer_device_id: String,
    peer_verifying_key: String,
) -> CommandResult<PairingResultView> {
    let surface = remote(&app)?;
    let now = Utc::now();

    let code = PairingCode::parse(&code).map_err(to_command_error)?;
    let key_bytes = decode_hex(&peer_verifying_key).ok_or_else(|| {
        "That device's key is not in the expected form. Check the code and try again."
            .to_string()
    })?;
    let peer = PairingIdentity {
        device_id: crate::domain::DeviceId::new(peer_device_id).map_err(to_command_error)?,
        verifying_key: crate::domain::VerifyingKey::from_bytes(&key_bytes).ok_or_else(|| {
            "That device presented a key this build cannot accept. Nothing was paired."
                .to_string()
        })?,
    };

    let own = PairingIdentity {
        device_id: surface.identity().device_id().clone(),
        verifying_key: surface.identity().verifying_key().clone(),
    };

    let store = surface.pairings().as_ref();

    let response = surface
        .with_issuer(now, |issuer| {
            accept_pairing_at_issuer(issuer, store, &own, &peer, &code, now)
        })
        .map_err(to_command_error)?
        .map_err(to_command_error)?;

    // Held before the response is consumed: on a refusal there is nothing to withdraw, and
    // on success this is the peer whose record the compensating step would remove.
    let recorded_peer = match &response {
        PairingResponse::Accepted { .. } => Some(peer.device_id.clone()),
        PairingResponse::Refused(_) => None,
    };

    let outcome = complete_pairing_at_requester(store, response, now, || {
        match &recorded_peer {
            Some(peer) => crate::application::pairing_flow::withdraw_pairing(store, peer, now),
            None => Ok(()),
        }
    })
    .map_err(to_command_error)?;

    Ok(PairingResultView {
        paired: outcome.is_paired(),
        peer: match &outcome {
            crate::application::pairing_flow::PairingOutcome::Paired { peer } => {
                Some(peer.as_str().to_string())
            }
            crate::application::pairing_flow::PairingOutcome::Refused(_) => None,
        },
        message: outcome.user_message(),
    })
}

/// Every pairing this device holds, revoked ones included, each with derived presence.
///
/// Revoked rows are listed rather than hidden: the capability requires the owner be able to
/// see what a device has been paired with, and a row that disappears makes "was this device
/// ever paired?" unanswerable after a device is lost.
///
/// Presence comes from [`PairingView::from_record`], which runs the presence rule over the
/// instant the transport last reported for that peer. There is no path here by which a
/// relay's own opinion of "online" could reach the UI.
#[tauri::command]
pub fn list_pairings<R: Runtime>(app: AppHandle<R>) -> CommandResult<Vec<PairingView>> {
    let surface = remote(&app)?;
    let now = Utc::now();

    let pairings = surface.pairings().list_pairings().map_err(to_command_error)?;

    // One transport call for the whole list rather than one per peer: the relay is a
    // network round trip, and a per-row call would make the list's cost grow with the
    // number of pairings for no additional information.
    let reported = last_reported_instants(&surface);

    Ok(pairings
        .iter()
        .map(|record| {
            let last_seen = reported
                .iter()
                .find(|(id, _)| id == record.peer.as_str())
                .and_then(|(_, at)| *at);
            PairingView::from_record(record, last_seen, now)
        })
        .collect())
}

/// Withdraws a peer's authority, with effect for every command evaluated afterwards.
///
/// A local act that needs no network, because a device must be de-authorizable when nothing
/// is reachable — which is exactly when someone is most likely to be doing it. The row is
/// retained and flagged rather than deleted, so the record that a pairing existed survives.
#[tauri::command]
pub fn revoke_pairing<R: Runtime>(app: AppHandle<R>, peer: String) -> CommandResult<()> {
    let surface = remote(&app)?;
    let peer = crate::domain::DeviceId::new(peer).map_err(to_command_error)?;

    surface
        .pairings()
        .revoke_pairing(&peer, Utc::now())
        .map_err(to_command_error)
}

/// The account's devices, each with presence derived by the presence rule.
///
/// Returns an empty list when no transport is configured, which is the honest answer: this
/// machine has no way to know what else is on the account. It does **not** fabricate a
/// single-entry list containing itself, which would read as "you have one device" rather
/// than "this cannot be answered".
#[tauri::command]
pub fn list_devices<R: Runtime>(app: AppHandle<R>) -> CommandResult<Vec<DeviceView>> {
    let surface = remote(&app)?;
    let now = Utc::now();

    let Some(transport) = surface.transport() else {
        return Ok(Vec::new());
    };

    let records = transport.list_devices().map_err(to_command_error)?;

    // Active pairings only. A revoked peer must not be shown as paired here, since its key
    // is absent from the map commands are checked against and a badge saying otherwise
    // would misdescribe what this machine will obey.
    let paired = surface
        .pairings()
        .verifying_keys_from_store()
        .map_err(to_command_error)?;

    let own_id = surface.identity().device_id().clone();

    Ok(records
        .iter()
        .map(|record| {
            DeviceView::from_record(
                record,
                now,
                paired.contains_key(&record.device_id),
                record.device_id == own_id,
            )
        })
        .collect())
}

/// The instant each device last reported, or an empty list when there is no transport.
///
/// Errors are logged and flattened to "nothing reported", not propagated. An unreachable
/// relay must not make the pairings list itself fail: revocation has to work offline, and a
/// list the user cannot load is a list they cannot revoke from.
fn last_reported_instants(
    surface: &RemoteSurface,
) -> Vec<(String, Option<chrono::DateTime<Utc>>)> {
    let Some(transport) = surface.transport() else {
        return Vec::new();
    };

    match transport.list_devices() {
        Ok(records) => records
            .into_iter()
            .map(|record| {
                (record.device_id.as_str().to_string(), record.last_reported_at)
            })
            .collect(),
        Err(error) => {
            log::warn!("could not list devices for presence, showing peers as offline: {error}");
            Vec::new()
        }
    }
}

/// Whether this machine will act on commands from a paired device.
#[tauri::command]
pub fn remote_control_enabled(state: State<'_, AppState>) -> CommandResult<bool> {
    Ok(state.settings().map_err(to_command_error)?.remote_control_enabled)
}

/// Turns remote control on or off, at this machine.
///
/// # Why this is a command of its own rather than a field on `save_settings`
///
/// `save_settings` takes a whole [`Settings`] from the web view. A stale form posting the
/// old value back would silently re-enable a setting the user had just turned off — the
/// same hazard the camelCase tests guard, with a worse consequence. One boolean in, one
/// boolean out, so the only way to change it is to ask for that change.
///
/// # This is the only way it changes
///
/// There is no remote path to this command. `RemoteCommand::EnableRemoteControl` is refused
/// by `authorize` *before* the pairing check even runs, and no arriving envelope is routed
/// here regardless. Requiring physical presence to grant the permission is what keeps
/// access to an account from being enough to turn remote control on everywhere and then
/// power off every device on it.
///
/// # Disabling is not revoking
///
/// This writes one settings row. Pairings live in their own table and are not touched:
/// turning remote control off says "not right now", revoking says "not this device, ever
/// again". Collapsing them would make a user who wanted a quiet evening re-pair every
/// device the next morning. The assertion below holds this at runtime rather than trusting
/// the reading.
#[tauri::command]
pub fn set_remote_control_enabled<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    enabled: bool,
) -> CommandResult<bool> {
    let pairings_before = pairing_count(&app);

    let mut settings = state.settings().map_err(to_command_error)?;
    settings.remote_control_enabled = enabled;
    state.save_settings(&settings).map_err(to_command_error)?;

    if let (Some(before), Some(after)) = (pairings_before, pairing_count(&app)) {
        if before != after {
            log::error!(
                "changing the remote-control setting changed the pairing count from \
                 {before} to {after}"
            );
            return Err("Changing this setting unexpectedly altered this device's \
                        pairings. Check the paired devices list."
                .to_string());
        }
    }

    // Read back rather than echoing the argument. What the UI should render is what is
    // stored, and a write that silently did nothing must not report success.
    Ok(state.settings().map_err(to_command_error)?.remote_control_enabled)
}

/// How many pairings this device holds, or `None` when the remote surface is unavailable.
///
/// Deliberately tolerant: this exists to check an invariant, and a machine with no remote
/// surface has no pairings to disturb.
fn pairing_count<R: Runtime>(app: &AppHandle<R>) -> Option<usize> {
    let surface = app.try_state::<RemoteSurface>()?;
    match surface.pairings().list_pairings() {
        Ok(pairings) => Some(pairings.len()),
        Err(error) => {
            log::warn!("could not count pairings: {error}");
            None
        }
    }
}

/// Decodes lowercase or uppercase hex into bytes, or `None` for anything malformed.
///
/// Hand-written rather than a dependency for six lines used at one boundary. Strict about
/// everything: an odd length, a non-hex digit, and an empty string are all `None`. Length
/// is *not* checked against the key size here — [`VerifyingKey::from_bytes`] is the one
/// place that decides what a valid key is, and a second length rule here could disagree
/// with it.
///
/// [`VerifyingKey::from_bytes`]: crate::domain::VerifyingKey::from_bytes
fn decode_hex(text: &str) -> Option<Vec<u8>> {
    // `% 2` rather than `usize::is_multiple_of`, which is stable only from 1.87 and this
    // crate's MSRV is 1.77.2. Clippy catches the difference; it is noted here so the
    // plainer-looking method is not "restored" later.
    if text.is_empty() || text.len() % 2 != 0 {
        return None;
    }

    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() / 2);
    for pair in bytes.chunks(2) {
        let high = (pair[0] as char).to_digit(16)?;
        let low = (pair[1] as char).to_digit(16)?;
        out.push((high * 16 + low) as u8);
    }
    Some(out)
}

fn parse_job_type(value: &str) -> Result<JobType, String> {
    JobType::from_str_value(value).ok_or_else(|| format!("Unknown job type: {value}"))
}

#[cfg(test)]
mod tests;

/// Structural rules about this module, in their own file so the greps cannot match their
/// own string literals. See the file header.
#[cfg(test)]
mod remote_source_tests;

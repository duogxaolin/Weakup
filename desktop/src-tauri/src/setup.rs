//! Startup wiring.
//!
//! Separated from `lib.rs` so the parts that do not need a running app can be tested:
//! where the database goes, and the order the pieces are built in.
//!
//! The rule this file exists to keep is task 9.8. Every optional component goes through
//! [`run_init_step`], which cannot return `Err`, so a session with no tray or no
//! notifications still gets a working scheduler and a window. The two things that are
//! *not* optional are the database and the scheduler: without them there is no app to
//! degrade into, and pretending otherwise would give the user a window whose buttons
//! quietly do nothing.

use std::path::PathBuf;
use std::sync::Arc;

use tauri::{AppHandle, Manager, Runtime};

use crate::application::grace_period::{PowerOffGate, RealGraceClock};
use crate::application::scheduler::{JobScheduler, SchedulerObserver, SystemSchedulerClock};
use crate::commands::state::AppState;
use crate::core::{AppError, AppResult};
use crate::data::{JobRepository, SettingsStore, SqliteJobRepository};
use crate::platform::capabilities::{Capability, CapabilityRegistry};
use crate::platform::notify::{NotificationPreference, NotifyingObserver};

/// The database filename inside the app's data directory.
pub const DATABASE_FILENAME: &str = "weakup.db";

/// Where the database lives.
///
/// Under the OS's own app-data directory rather than next to the executable: an app
/// bundle on macOS and a Program Files install on Windows are both read-only, so a
/// database beside the binary fails on first write.
pub fn database_path<R: Runtime>(app: &AppHandle<R>) -> AppResult<PathBuf> {
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|error| AppError::Storage {
            message: format!("could not locate the app data directory: {error}"),
        })?;

    std::fs::create_dir_all(&directory).map_err(|error| AppError::Storage {
        message: format!("could not create {}: {error}", directory.display()),
    })?;

    Ok(directory.join(DATABASE_FILENAME))
}

/// Builds everything and registers it as managed state.
///
/// Returns `Err` only for the two genuinely fatal conditions — no database and no
/// scheduler. Everything else degrades.
pub fn initialise<R: Runtime>(app: &AppHandle<R>) -> AppResult<Arc<JobScheduler>> {
    let capabilities = Arc::new(CapabilityRegistry::new());

    let repository = Arc::new(SqliteJobRepository::open(database_path(app)?)?);
    let settings: Arc<dyn SettingsStore> = repository.clone();
    let jobs: Arc<dyn JobRepository> = repository.clone();

    // Recorded before anything is built, so the UI can explain a host that cannot do
    // one of the two things the app is for.
    record_platform_capabilities(&capabilities);

    let keep_awake = Arc::new(crate::platform::keep_awake::KeepAwakeCoordinator::new(
        crate::platform::keep_awake::host_controller(),
    ));

    let gate = Arc::new(PowerOffGate::new(
        crate::platform::power_off::host_executor(),
        Arc::new(RealGraceClock),
    ));

    // Seeded from the stored setting, then shared: the observer reads this instead of
    // the database, so the setting takes effect the moment it is saved and costs nothing
    // on the scheduler's hot path.
    let preference = Arc::new(NotificationPreference::new(
        settings
            .load()
            .map(|stored| stored.notifications_enabled)
            .unwrap_or(true),
    ));

    let observer: Arc<dyn SchedulerObserver> = Arc::new(NotifyingObserver::for_app(
        app.clone(),
        capabilities.clone(),
        preference.clone(),
    ));

    let scheduler = Arc::new(JobScheduler::new(
        jobs,
        keep_awake,
        gate,
        Arc::new(SystemSchedulerClock),
        observer,
    ));

    app.manage(AppState::new(
        scheduler.clone(),
        settings,
        capabilities.clone(),
        preference,
    ));

    // The first reconciliation is the first iteration of `JobScheduler::spawn`, not a
    // synchronous call here. Tauri runs setup on AppKit's launch callback on macOS; a
    // due power-off can spend 60 seconds in its grace period, and blocking setup would
    // prevent the web view from appearing to show the countdown or cancel button.
    Ok(scheduler)
}

/// Installs the optional components. None of these can fail startup (task 9.8).
pub fn initialise_optional<R: Runtime>(app: &AppHandle<R>) {
    let Some(state) = app.try_state::<AppState>() else {
        log::error!("app state missing; skipping optional components");
        return;
    };
    let capabilities = state.capabilities().clone();

    crate::platform::tray::install_or_degrade(app, &capabilities);
    crate::platform::autostart::probe_or_degrade(app, &capabilities);
    crate::platform::notify::probe_or_degrade(app, &capabilities);

    if capabilities.has_degradation() {
        for entry in capabilities.degraded() {
            log::warn!("degraded: {} — {:?}", entry.label, entry.state);
        }
    }
}

/// Records whether this host can keep awake and power off at all.
///
/// Both are asked of the platform rather than assumed from the target OS: a Linux
/// desktop with no D-Bus screensaver interface cannot hold the display awake even
/// though Linux in general can.
fn record_platform_capabilities(capabilities: &Arc<CapabilityRegistry>) {
    let controller = crate::platform::keep_awake::host_controller();
    if !controller.is_available() {
        capabilities.mark_unavailable(
            Capability::KeepAwake,
            controller.unavailable_reason().unwrap_or_else(|| {
                "this system has no way to keep the display awake".to_string()
            }),
        );
    }

    let executor = crate::platform::power_off::host_executor();
    if !executor.is_supported() {
        capabilities.mark_unavailable(
            Capability::PowerOff,
            "this platform cannot be shut down by an app",
        );
    }
}

/// Runs one optional step, degrading rather than failing. Re-exported so the tray,
/// autostart, and notification modules share one definition of that guarantee.
pub use crate::platform::capabilities::run_init_step;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_database_filename_is_stable() {
        // Changing this orphans every existing user's jobs, silently: the app would
        // create a fresh empty database and their schedules would simply be gone.
        assert_eq!(DATABASE_FILENAME, "weakup.db");
    }

    #[test]
    fn setup_never_runs_a_scheduler_tick_on_the_appkit_callback() {
        // A due power-off can spend 60 seconds inside its grace period. Tauri calls
        // `initialise` from AppKit's launch callback on macOS, so a synchronous tick
        // here would keep the web view — including its cancel button — from appearing.
        let source = include_str!("setup.rs");
        let initialise = source
            .split_once("pub fn initialise<R: Runtime>")
            .expect("initialise exists")
            .1
            .split_once("/// Installs the optional components")
            .expect("initialise has an end marker")
            .0;

        let forbidden = ["scheduler", ".tick()"].concat();
        assert!(
            !initialise.contains(&forbidden),
            "initialise must leave reconciliation to the background loop"
        );
    }

    #[test]
    fn a_host_that_cannot_keep_awake_is_recorded_rather_than_assumed_working() {
        // What a headless Linux session looks like. The UI must be able to say the
        // keep-awake feature will not work here.
        let capabilities = Arc::new(CapabilityRegistry::new());
        capabilities.mark_unavailable(
            Capability::KeepAwake,
            "no D-Bus screensaver interface and no systemd-inhibit",
        );

        let degraded = capabilities.degraded();
        assert_eq!(degraded.len(), 1);
        assert!(degraded[0].essential, "keep-awake is one of the two features");
    }

    #[test]
    fn the_optional_components_are_the_ones_that_may_fail() {
        // A guard on the classification itself. Marking keep-awake or power-off as
        // non-essential would let the app report itself healthy while unable to do
        // either thing it exists for.
        for capability in [
            Capability::Tray,
            Capability::Autostart,
            Capability::Notifications,
        ] {
            assert!(!capability.is_essential(), "{capability:?}");
        }
        for capability in [Capability::KeepAwake, Capability::PowerOff] {
            assert!(capability.is_essential(), "{capability:?}");
        }
    }

    #[test]
    fn a_failing_optional_step_leaves_the_registry_usable() {
        let capabilities = Arc::new(CapabilityRegistry::new());

        let result = run_init_step(&capabilities, Capability::Tray, || {
            Err(AppError::RuntimeInit {
                component: "System tray".to_string(),
                message: "no tray".to_string(),
            })
        });

        assert!(result.is_ok());
        assert!(capabilities.is_available(Capability::PowerOff));
        assert!(capabilities.is_available(Capability::KeepAwake));
    }
}

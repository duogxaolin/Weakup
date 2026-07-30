//! Launching at login (tasks 9.5, 9.6).
//!
//! Off by default, and deliberately so: an app the user has not yet decided to trust
//! should not add itself to their login items. Turning it on is a choice they make in
//! the UI, which is also what makes the setting worth persisting.

use std::sync::Arc;

use tauri::{AppHandle, Runtime};
use tauri_plugin_autostart::ManagerExt;

use crate::core::{AppError, AppResult};
use crate::platform::capabilities::{Capability, CapabilityRegistry};

/// Whether the app is registered to launch at login.
///
/// Task 9.6: on a fresh install nothing is registered, and that is a normal `false`
/// rather than an error. Some platforms report a missing registration as a failure to
/// read it, so a plugin error is treated as "not enabled" — the honest answer, since
/// the app plainly is not registered if we cannot find a registration.
pub fn is_enabled<R: Runtime>(app: &AppHandle<R>) -> AppResult<bool> {
    Ok(interpret_read(app.autolaunch().is_enabled()))
}

/// The fallback decision, split out from [`is_enabled`] because that function needs a
/// live `AppHandle` and so cannot be tested. This one can, which matters: the fallback
/// has to be `false`. Answering `true` when the read failed would show the user a
/// switch that is on while the app is not actually registered to launch.
fn interpret_read<E: std::fmt::Display>(result: Result<bool, E>) -> bool {
    match result {
        Ok(enabled) => enabled,
        Err(error) => {
            log::debug!("could not read the autostart state, treating it as disabled: {error}");
            false
        }
    }
}

/// Turns launch-at-login on or off.
///
/// Unlike [`is_enabled`], a failure here is reported: the user asked for a change and
/// must not be told it happened when it did not.
pub fn set_enabled<R: Runtime>(app: &AppHandle<R>, enabled: bool) -> AppResult<()> {
    let manager = app.autolaunch();

    let result = if enabled {
        manager.enable()
    } else {
        manager.disable()
    };

    // `component` is what the user reads — `AppError::user_message` renders it as
    // "<component> is unavailable in this session." — so it is prose, not an
    // identifier. The driver's own wording goes in `message`, for the log.
    result.map_err(|error| AppError::RuntimeInit {
        component: "Launch at login".to_string(),
        message: format!(
            "could not {} launch at login: {error}",
            if enabled { "enable" } else { "disable" }
        ),
    })
}

/// Checks at startup that the autostart state can be read at all.
///
/// Does not change anything. Task 9.5's "off by default" is satisfied by never
/// enabling it, not by disabling it here — calling `disable` on every launch would
/// undo the user's own choice on the next start.
pub fn probe_or_degrade<R: Runtime>(app: &AppHandle<R>, registry: &Arc<CapabilityRegistry>) {
    let _ = crate::platform::capabilities::run_init_step(registry, Capability::Autostart, || {
        let enabled = is_enabled(app)?;
        log::info!("launch at login is currently {}", if enabled { "on" } else { "off" });
        Ok(())
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unreadable_autostart_state_counts_as_off() {
        // Task 9.6. Reporting "on" here would leave the user with a switch that says the
        // app starts at login while nothing is registered to start it.
        assert!(!interpret_read::<String>(Err("no login items on this session".to_string())));
    }

    #[test]
    fn a_readable_autostart_state_is_passed_through_unchanged() {
        assert!(interpret_read::<String>(Ok(true)));
        assert!(!interpret_read::<String>(Ok(false)));
    }

    #[test]
    fn a_failure_to_change_the_setting_names_the_feature_in_plain_words() {
        // The user toggled a switch and it did not take effect. `user_message` renders
        // only the component, so the component has to be the readable part.
        let error = AppError::RuntimeInit {
            component: "Launch at login".to_string(),
            message: "denied".to_string(),
        };

        let shown = error.user_message();
        assert!(shown.contains("Launch at login"), "got: {shown}");
        assert!(
            !shown.contains("autostart"),
            "the plugin's name is not the user's word for it: {shown}"
        );
    }

    #[test]
    fn nothing_here_enables_autostart_on_its_own() {
        // Task 9.5, checked against the source rather than by inspection: the only
        // call to `enable()` is inside `set_enabled`, which needs an explicit `true`.
        let source = include_str!("autostart.rs");
        let body = source
            .split("pub fn probe_or_degrade")
            .nth(1)
            .expect("the probe function exists");
        let body = body.split("#[cfg(test)]").next().unwrap();

        assert!(
            !body.contains(".enable()"),
            "the startup probe must not register the app at login"
        );
        assert!(
            !body.contains(".disable()"),
            "nor unregister it, which would discard the user's own choice"
        );
    }
}

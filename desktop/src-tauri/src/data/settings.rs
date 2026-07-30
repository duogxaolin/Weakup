//! Persisted user settings.
//!
//! A key-value table rather than a column per setting, because adding a setting should
//! not need a schema migration for what is a handful of small values read once at
//! startup.
//!
//! Autostart is deliberately absent: the operating system already stores whether the
//! app is registered at login, and a second copy here would be the one that goes
//! stale — the user can remove a login item without the app ever being told.

use serde::{Deserialize, Serialize};

use crate::core::AppResult;

/// The settings, with the defaults applied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    /// IANA timezone used to resolve absolute-time triggers.
    pub timezone: String,
    /// Whether the app should try to notify at all. Separate from whether the OS
    /// permits it: this is the user's choice, that is the system's.
    pub notifications_enabled: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            timezone: default_timezone(),
            notifications_enabled: true,
        }
    }
}

/// The host's timezone, falling back to UTC.
///
/// UTC is a poor default for a 22:00 shutdown, so this is a fallback rather than a
/// choice. The UI shows the resolved timezone for exactly this reason: a user in
/// Vietnam seeing "UTC" can correct it before scheduling anything.
pub fn default_timezone() -> String {
    iana_time_zone::get_timezone().unwrap_or_else(|error| {
        log::warn!("could not read the system timezone, falling back to UTC: {error}");
        "UTC".to_string()
    })
}

/// Reads and writes [`Settings`].
pub trait SettingsStore: Send + Sync {
    fn load(&self) -> AppResult<Settings>;
    fn save(&self, settings: &Settings) -> AppResult<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_timezone_is_a_timezone_the_resolver_accepts() {
        // A default the resolver rejects would make every absolute-time job fail on a
        // fresh install.
        let timezone = default_timezone();
        assert!(
            timezone.parse::<chrono_tz::Tz>().is_ok(),
            "not a usable IANA name: {timezone}"
        );
    }

    #[test]
    fn notifications_default_to_on() {
        // The app runs with no window most of the time; a notification is usually the
        // only way the user hears about a shutdown.
        assert!(Settings::default().notifications_enabled);
    }
}

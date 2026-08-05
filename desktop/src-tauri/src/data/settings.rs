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
///
/// camelCase on the wire to match every other DTO. This type is stored as explicit
/// key-value rows rather than serialised, so the rename only affects IPC — but it has to
/// be here rather than on a separate DTO, because `save_settings` takes this type
/// directly and one snake_case field in an otherwise camelCase surface is the kind of
/// inconsistency the web view gets wrong silently, reading `undefined` as `false`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// IANA timezone used to resolve absolute-time triggers.
    pub timezone: String,
    /// Whether the app should try to notify at all. Separate from whether the OS
    /// permits it: this is the user's choice, that is the system's.
    pub notifications_enabled: bool,
    /// Which colour theme the window paints in. Purely presentational — no scheduling
    /// behaviour reads it.
    pub theme: Theme,
    /// Which language the interface is written in.
    pub language: Language,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            timezone: default_timezone(),
            notifications_enabled: true,
            theme: Theme::default(),
            language: Language::default(),
        }
    }
}

/// The window's colour theme.
///
/// An enum rather than a string because the web view sets `data-theme` from it, and a
/// typo reaching that attribute produces an unstyled window rather than an error. Serde
/// rejects an unknown variant at the IPC boundary instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Theme {
    /// Follow the operating system's light/dark preference.
    #[default]
    Auto,
    Light,
    Dark,
}

impl Theme {
    /// The stored spelling. Stable across releases: this string is in the user's database.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    /// Parses a stored value, falling back to the default.
    ///
    /// A row written by a newer version must not stop the app loading — the rest of the
    /// settings are still good, and an unreadable theme costs the user nothing but their
    /// colour choice.
    pub fn from_stored(value: &str) -> Self {
        match value {
            "light" => Self::Light,
            "dark" => Self::Dark,
            "auto" => Self::Auto,
            other => {
                log::debug!("unknown stored theme {other:?}, using the default");
                Self::default()
            }
        }
    }
}

/// The interface language.
///
/// Only languages with a complete translation are listed. A variant with no strings
/// behind it would show a half-translated window, which is harder to use than English.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Language {
    #[default]
    En,
    Vi,
}

impl Language {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Vi => "vi",
        }
    }

    pub fn from_stored(value: &str) -> Self {
        match value {
            "vi" => Self::Vi,
            "en" => Self::En,
            other => {
                log::debug!("unknown stored language {other:?}, using the default");
                Self::default()
            }
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

    #[test]
    fn the_wire_field_is_camel_case_like_every_other_dto() {
        // The web view writes `notificationsEnabled`. If this ever serialises as
        // `notifications_enabled` again, the checkbox reads `undefined`, which is falsey —
        // so the UI would show notifications as off and silently turn them off on the next
        // save. A wrong-but-plausible value is worse than an error, hence a test on the
        // key rather than on a round-trip alone.
        let json = serde_json::to_value(Settings {
            timezone: "Asia/Ho_Chi_Minh".to_string(),
            notifications_enabled: true,
            theme: Theme::Dark,
            language: Language::Vi,
        })
        .unwrap();

        assert_eq!(json["notificationsEnabled"], serde_json::json!(true));
        assert!(
            json.get("notifications_enabled").is_none(),
            "both spellings present: {json}"
        );
    }

    #[test]
    fn theme_and_language_serialise_as_the_strings_the_web_view_switches_on() {
        // `main.js` writes the theme straight into `data-theme` and looks the language up
        // in a string table. A different spelling here paints an unstyled window or falls
        // back to English silently, so the wire form is pinned rather than assumed.
        let json = serde_json::to_value(Settings {
            timezone: "UTC".to_string(),
            notifications_enabled: true,
            theme: Theme::Dark,
            language: Language::Vi,
        })
        .unwrap();

        assert_eq!(json["theme"], serde_json::json!("dark"));
        assert_eq!(json["language"], serde_json::json!("vi"));
    }

    #[test]
    fn theme_defaults_to_following_the_system() {
        // A fresh install should look like the rest of the desktop rather than forcing a
        // choice the user never made.
        assert_eq!(Settings::default().theme, Theme::Auto);
    }

    #[test]
    fn language_defaults_to_english() {
        assert_eq!(Settings::default().language, Language::En);
    }

    #[test]
    fn every_theme_round_trips_through_its_stored_spelling() {
        for theme in [Theme::Auto, Theme::Light, Theme::Dark] {
            assert_eq!(Theme::from_stored(theme.as_str()), theme);
        }
    }

    #[test]
    fn every_language_round_trips_through_its_stored_spelling() {
        for language in [Language::En, Language::Vi] {
            assert_eq!(Language::from_stored(language.as_str()), language);
        }
    }

    #[test]
    fn a_theme_from_a_newer_version_falls_back_rather_than_failing_the_load() {
        // The whole settings row is read together. Refusing to parse one presentational
        // value would take the timezone down with it.
        assert_eq!(Theme::from_stored("solarized"), Theme::default());
        assert_eq!(Language::from_stored("kl"), Language::default());
    }

    #[test]
    fn an_unknown_theme_over_ipc_is_rejected_rather_than_defaulted() {
        // Unlike a stored row, a bad IPC payload is a bug in the web view. Failing loudly
        // is what surfaces it, and there is no user data at risk in rejecting the call.
        let result: Result<Settings, _> = serde_json::from_str(
            r#"{"timezone":"UTC","notificationsEnabled":true,"theme":"neon","language":"en"}"#,
        );

        assert!(result.is_err(), "accepted a theme that does not exist");
    }

    #[test]
    fn a_snake_case_payload_is_rejected_rather_than_silently_defaulted() {
        // Deserialising the old spelling must fail loudly. Were it to fall back to the
        // default instead, a stale front end would keep re-enabling notifications for a
        // user who turned them off.
        let result: Result<Settings, _> = serde_json::from_str(
            r#"{"timezone":"UTC","notifications_enabled":false}"#,
        );

        assert!(result.is_err(), "accepted the wrong spelling");
    }
}

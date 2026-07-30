use std::fmt;

/// The result type for every fallible operation in this crate.
///
/// The Dart implementation used sealed `Result<T>` classes rather than exceptions
/// specifically to make failure modes explicit in signatures. Rust's `Result` is that
/// same shape natively, so the port is direct.
pub type AppResult<T> = Result<T, AppError>;

/// Typed error hierarchy. Each variant carries enough information to produce a
/// specific, actionable user message — a generic "something went wrong" is never
/// adequate for these failures, because the remedy differs per variant.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AppError {
    /// Power-off is impossible on this target. Returned by the unsupported executor,
    /// which invokes nothing.
    PowerOffUnsupported,

    /// Windows: `ERROR_PRIVILEGE_NOT_HELD` (1314) — domain policy stripped
    /// `SeShutdownPrivilege`.
    PowerOffPrivilegeDenied { detail: Option<String> },

    /// macOS: the Automation (TCC) consent for System Events was denied. Reachable
    /// only when `NSAppleEventsUsageDescription` is declared; without it macOS
    /// terminates the process instead of prompting.
    PowerOffConsentDenied { detail: Option<String> },

    /// Linux: polkit denied `org.freedesktop.login1.power-off`.
    PowerOffPolicyDenied { detail: Option<String> },

    /// A power-off command failed for a reason that could not be classified.
    PowerOffFailed { message: String },

    /// This host has no keep-awake mechanism at all — chiefly Linux with neither a
    /// freedesktop D-Bus inhibitor nor `systemd-inhibit` (task 8.6).
    ///
    /// Deliberately distinct from `KeepAwakeFailed`: "there is nothing here to try"
    /// and "it exists and refused" call for different words to the user, and only
    /// the second is worth retrying.
    KeepAwakeUnavailable { detail: Option<String> },

    /// A keep-awake mechanism exists on this host but the request failed.
    KeepAwakeFailed { message: String },

    /// A runtime OS permission was denied.
    PermissionDenied { permission: String },

    /// Persistence failure.
    Storage { message: String },

    /// Notification delivery or scheduling failure.
    Notification { message: String },

    /// A desktop runtime component (tray, autostart, notifications) failed to
    /// initialize. Never fatal: startup continues and the capability is reported
    /// unavailable.
    RuntimeInit { component: String, message: String },

    /// User input is invalid. `message` is directly displayable.
    Validation { message: String },
}

impl AppError {
    pub fn validation(message: impl Into<String>) -> Self {
        Self::Validation {
            message: message.into(),
        }
    }

    pub fn storage(message: impl Into<String>) -> Self {
        Self::Storage {
            message: message.into(),
        }
    }

    pub fn power_off_failed(message: impl Into<String>) -> Self {
        Self::PowerOffFailed {
            message: message.into(),
        }
    }

    pub fn runtime_init(component: impl Into<String>, message: impl Into<String>) -> Self {
        Self::RuntimeInit {
            component: component.into(),
            message: message.into(),
        }
    }

    /// A message safe and useful to show a user, including the remedy where one
    /// exists. Denial variants name the exact settings path, because "permission
    /// denied" without a path leaves the feature broken with no way forward.
    pub fn user_message(&self) -> String {
        match self {
            Self::PowerOffUnsupported => {
                "This platform does not allow an app to power off the device.".to_string()
            }
            Self::PowerOffPrivilegeDenied { .. } => {
                "Your organization's policy prevents this app from shutting down the PC."
                    .to_string()
            }
            Self::PowerOffConsentDenied { .. } => {
                "Permission to control System Events was denied. Re-enable it in \
                 System Settings > Privacy & Security > Automation."
                    .to_string()
            }
            Self::PowerOffPolicyDenied { .. } => {
                "The system's power policy blocked the shutdown request.".to_string()
            }
            Self::PowerOffFailed { message } => {
                format!("The shutdown command failed: {message}")
            }
            Self::KeepAwakeUnavailable { .. } => {
                "This system provides no way to keep the screen awake, so that part \
                 of the job cannot run. Scheduled power-off still works."
                    .to_string()
            }
            Self::KeepAwakeFailed { message } => {
                format!("Could not keep the screen awake: {message}")
            }
            Self::PermissionDenied { permission } => {
                format!("Permission '{permission}' was denied.")
            }
            Self::Storage { message } => format!("Could not save or load jobs: {message}"),
            Self::Notification { message } => format!("Could not show a notification: {message}"),
            Self::RuntimeInit { component, .. } => {
                format!("{component} is unavailable in this session.")
            }
            Self::Validation { message } => message.clone(),
        }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PowerOffUnsupported => write!(f, "PowerOffUnsupported"),
            Self::PowerOffPrivilegeDenied { detail } => {
                write!(f, "PowerOffPrivilegeDenied({})", detail.as_deref().unwrap_or(""))
            }
            Self::PowerOffConsentDenied { detail } => {
                write!(f, "PowerOffConsentDenied({})", detail.as_deref().unwrap_or(""))
            }
            Self::PowerOffPolicyDenied { detail } => {
                write!(f, "PowerOffPolicyDenied({})", detail.as_deref().unwrap_or(""))
            }
            Self::PowerOffFailed { message } => write!(f, "PowerOffFailed({message})"),
            Self::KeepAwakeUnavailable { detail } => {
                write!(f, "KeepAwakeUnavailable({})", detail.as_deref().unwrap_or(""))
            }
            Self::KeepAwakeFailed { message } => write!(f, "KeepAwakeFailed({message})"),
            Self::PermissionDenied { permission } => write!(f, "PermissionDenied({permission})"),
            Self::Storage { message } => write!(f, "StorageError({message})"),
            Self::Notification { message } => write!(f, "NotificationError({message})"),
            Self::RuntimeInit { component, message } => {
                write!(f, "RuntimeInitError({component}: {message})")
            }
            Self::Validation { message } => write!(f, "ValidationError({message})"),
        }
    }
}

impl std::error::Error for AppError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn power_off_unsupported_message_states_the_platform_limit() {
        let msg = AppError::PowerOffUnsupported.user_message();
        assert!(msg.contains("does not allow"));
    }

    #[test]
    fn consent_denied_message_names_the_settings_path() {
        // A denial message without the re-enable path leaves the user stuck, so the
        // path is part of the contract rather than a nicety.
        let msg = AppError::PowerOffConsentDenied { detail: None }.user_message();
        assert!(msg.contains("Automation"));
        assert!(msg.contains("Privacy & Security"));
    }

    #[test]
    fn privilege_denied_message_blames_policy_not_the_user() {
        let msg = AppError::PowerOffPrivilegeDenied { detail: None }.user_message();
        assert!(msg.contains("policy"));
    }

    #[test]
    fn validation_message_is_passed_through_verbatim_for_display() {
        let err = AppError::validation("Duration must be positive");
        assert_eq!(err.user_message(), "Duration must be positive");
    }

    #[test]
    fn runtime_init_failure_is_reported_per_component() {
        let err = AppError::runtime_init("Tray", "icon load failed");
        assert!(err.user_message().contains("Tray"));
        assert_eq!(err.to_string(), "RuntimeInitError(Tray: icon load failed)");
    }

    #[test]
    fn variants_round_trip_through_serde_with_their_payload() {
        let original = AppError::PowerOffConsentDenied {
            detail: Some("osascript exit 1".to_string()),
        };
        let json = serde_json::to_string(&original).expect("serialize");
        let parsed: AppError = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(original, parsed);
    }
}

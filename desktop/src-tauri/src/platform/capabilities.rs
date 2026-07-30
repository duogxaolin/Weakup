//! What this installation can actually do, and what it cannot.
//!
//! Task 9.8 requires that no failing init step aborts startup. The obvious way to get
//! that is a `let _ = ...` around each step, but then the user is left with an app
//! whose tray silently never appeared and no way to find out why. So each step reports
//! into this registry instead: a failure is logged, recorded with a reason a person can
//! read, and startup continues.
//!
//! Kept free of Tauri types so the degradation logic is unit-testable without a running
//! app — see [`run_init_step`], which is what task 9.9 exercises.

use std::collections::BTreeMap;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::core::AppResult;

/// A thing the app might or might not be able to do on this host.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    /// The system tray icon and its menu.
    Tray,
    /// Launching at login.
    Autostart,
    /// Desktop notifications.
    Notifications,
    /// Holding the display awake.
    KeepAwake,
    /// Shutting the machine down.
    PowerOff,
}

impl Capability {
    pub const ALL: [Capability; 5] = [
        Capability::Tray,
        Capability::Autostart,
        Capability::Notifications,
        Capability::KeepAwake,
        Capability::PowerOff,
    ];

    /// A name for logs and for the UI.
    pub fn label(self) -> &'static str {
        match self {
            Capability::Tray => "system tray",
            Capability::Autostart => "start at login",
            Capability::Notifications => "notifications",
            Capability::KeepAwake => "keep screen awake",
            Capability::PowerOff => "scheduled power-off",
        }
    }

    /// Whether losing this capability stops the app being useful.
    ///
    /// Drives how loudly the UI says so: a missing tray is an inconvenience, a missing
    /// power-off means a scheduled shutdown will never happen.
    pub fn is_essential(self) -> bool {
        matches!(self, Capability::KeepAwake | Capability::PowerOff)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum CapabilityState {
    Available,
    /// Present but not working, with a reason written for a person.
    Unavailable { reason: String },
}

impl CapabilityState {
    pub fn is_available(&self) -> bool {
        matches!(self, CapabilityState::Available)
    }
}

/// One capability and its state, for the IPC surface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityReport {
    pub capability: Capability,
    pub label: String,
    pub essential: bool,
    #[serde(flatten)]
    pub state: CapabilityState,
}

/// The live record of what works.
///
/// Starts with everything available and is degraded by whatever fails. Starting from
/// "available" rather than "unknown" matters: a capability nobody had reason to
/// question should not be reported as broken.
#[derive(Debug, Default)]
pub struct CapabilityRegistry {
    states: Mutex<BTreeMap<Capability, String>>,
}

impl CapabilityRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Records that a capability does not work here, with a reason for the user.
    pub fn mark_unavailable(&self, capability: Capability, reason: impl Into<String>) {
        let reason = reason.into();
        log::warn!("{} is unavailable: {reason}", capability.label());
        if let Ok(mut states) = self.states.lock() {
            states.insert(capability, reason);
        }
    }

    /// Clears a degradation, for a capability that later starts working.
    pub fn mark_available(&self, capability: Capability) {
        if let Ok(mut states) = self.states.lock() {
            states.remove(&capability);
        }
    }

    pub fn state(&self, capability: Capability) -> CapabilityState {
        match self.states.lock() {
            Ok(states) => match states.get(&capability) {
                Some(reason) => CapabilityState::Unavailable {
                    reason: reason.clone(),
                },
                None => CapabilityState::Available,
            },
            // A poisoned lock is itself a degradation, and claiming availability we
            // cannot verify is the one answer that could mislead the user.
            Err(_) => CapabilityState::Unavailable {
                reason: "capability state could not be read".to_string(),
            },
        }
    }

    pub fn is_available(&self, capability: Capability) -> bool {
        self.state(capability).is_available()
    }

    /// Every capability and its state, in a stable order.
    pub fn report(&self) -> Vec<CapabilityReport> {
        Capability::ALL
            .into_iter()
            .map(|capability| CapabilityReport {
                capability,
                label: capability.label().to_string(),
                essential: capability.is_essential(),
                state: self.state(capability),
            })
            .collect()
    }

    /// Only the capabilities that are degraded. What the UI shows a banner for.
    pub fn degraded(&self) -> Vec<CapabilityReport> {
        self.report()
            .into_iter()
            .filter(|entry| !entry.state.is_available())
            .collect()
    }

    pub fn has_degradation(&self) -> bool {
        !self.degraded().is_empty()
    }
}

/// Runs one startup step and refuses to let it abort startup.
///
/// This function *is* task 9.8's guarantee: it returns `Ok(())` whatever `step` does,
/// so a caller that routes every init step through it cannot fail startup even by
/// using `?`. Expressing it as a function rather than as a rule about writing
/// `let _ =` at nine call sites means the ninth call site cannot get it wrong.
pub fn run_init_step<F>(
    registry: &CapabilityRegistry,
    capability: Capability,
    step: F,
) -> AppResult<()>
where
    F: FnOnce() -> AppResult<()>,
{
    match step() {
        Ok(()) => {
            registry.mark_available(capability);
            Ok(())
        }
        Err(error) => {
            registry.mark_unavailable(capability, error.user_message());
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::AppError;
    use std::sync::Arc;

    #[test]
    fn everything_starts_available() {
        let registry = CapabilityRegistry::new();

        for capability in Capability::ALL {
            assert!(registry.is_available(capability), "{capability:?}");
        }
        assert!(!registry.has_degradation());
        assert!(registry.degraded().is_empty());
    }

    #[test]
    fn a_failed_init_step_still_returns_ok() {
        // Task 9.9. The setup callback uses `?`, so a step returning `Err` here would
        // abort startup and leave the user with no window at all.
        let registry = CapabilityRegistry::new();

        let result = run_init_step(&registry, Capability::Tray, || {
            Err(AppError::RuntimeInit {
                component: "tray".to_string(),
                message: "no tray on this session".to_string(),
            })
        });

        assert!(result.is_ok(), "startup must never abort on a failed step");
        assert!(!registry.is_available(Capability::Tray));
    }

    #[test]
    fn a_failed_step_records_a_reason_the_user_can_read() {
        // "unavailable" with no reason gives the user nothing to act on.
        let registry = CapabilityRegistry::new();

        run_init_step(&registry, Capability::Notifications, || {
            Err(AppError::Notification {
                message: "notifications are turned off for this app".to_string(),
            })
        })
        .unwrap();

        match registry.state(Capability::Notifications) {
            CapabilityState::Unavailable { reason } => {
                assert!(!reason.trim().is_empty());
                assert!(
                    reason.contains("notification"),
                    "the reason should name the problem, got: {reason}"
                );
            }
            CapabilityState::Available => panic!("expected the capability to be degraded"),
        }
    }

    #[test]
    fn one_failing_step_does_not_degrade_the_others() {
        // Task 9.8's "independently". A tray failure must not cost the user
        // notifications, which is what a single shared success flag would do.
        let registry = CapabilityRegistry::new();

        run_init_step(&registry, Capability::Tray, || {
            Err(AppError::RuntimeInit {
                component: "tray".to_string(),
                message: "failed".to_string(),
            })
        })
        .unwrap();
        run_init_step(&registry, Capability::Notifications, || Ok(())).unwrap();
        run_init_step(&registry, Capability::Autostart, || Ok(())).unwrap();

        assert!(!registry.is_available(Capability::Tray));
        assert!(registry.is_available(Capability::Notifications));
        assert!(registry.is_available(Capability::Autostart));
        assert_eq!(registry.degraded().len(), 1);
    }

    #[test]
    fn every_step_failing_still_yields_a_successful_setup() {
        // The worst case: a headless session where nothing works. The app must still
        // start, so the user can at least read why.
        let registry = CapabilityRegistry::new();

        for capability in Capability::ALL {
            let result = run_init_step(&registry, capability, || {
                Err(AppError::RuntimeInit {
                    component: "everything".to_string(),
                    message: "unavailable".to_string(),
                })
            });
            assert!(result.is_ok());
        }

        assert_eq!(registry.degraded().len(), Capability::ALL.len());
    }

    #[test]
    fn a_capability_that_recovers_is_reported_as_available_again() {
        let registry = CapabilityRegistry::new();
        registry.mark_unavailable(Capability::Autostart, "not registered yet");
        assert!(!registry.is_available(Capability::Autostart));

        run_init_step(&registry, Capability::Autostart, || Ok(())).unwrap();

        assert!(registry.is_available(Capability::Autostart));
    }

    #[test]
    fn a_registry_that_cannot_be_read_reports_unavailable_rather_than_working() {
        // A panic while the lock is held poisons it for the rest of the process. The
        // honest answer then is "unavailable": telling the user a feature works when we
        // can no longer check is the one wrong answer, because they would rely on it.
        let registry = Arc::new(CapabilityRegistry::new());
        let poisoner = Arc::clone(&registry);

        let panicked = std::thread::spawn(move || {
            let _guard = poisoner.states.lock().unwrap();
            panic!("deliberate panic to poison the lock");
        })
        .join();
        assert!(panicked.is_err(), "the helper thread must actually panic");

        for capability in Capability::ALL {
            assert!(
                !registry.is_available(capability),
                "{capability:?} must not be reported as working"
            );
        }
        assert!(registry.has_degradation());
        assert_eq!(registry.degraded().len(), Capability::ALL.len());
    }

    #[test]
    fn the_essential_capabilities_are_the_two_features_the_app_exists_for() {
        assert!(Capability::KeepAwake.is_essential());
        assert!(Capability::PowerOff.is_essential());
        assert!(!Capability::Tray.is_essential());
        assert!(!Capability::Autostart.is_essential());
        assert!(!Capability::Notifications.is_essential());
    }

    #[test]
    fn every_capability_has_a_human_label() {
        for capability in Capability::ALL {
            let label = capability.label();
            assert!(!label.trim().is_empty(), "{capability:?}");
            assert!(
                label.chars().any(|character| character.is_lowercase()),
                "labels are prose, not identifiers: {label}"
            );
        }
    }

    #[test]
    fn the_report_serialises_flat_for_the_web_view() {
        let registry = CapabilityRegistry::new();
        registry.mark_unavailable(Capability::Tray, "no tray");

        let report = registry.report();
        let json = serde_json::to_value(&report).unwrap();
        let tray = json
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["capability"] == "tray")
            .expect("the tray is in the report");

        assert_eq!(tray["state"], "unavailable");
        assert_eq!(tray["reason"], "no tray");
        assert_eq!(tray["essential"], false);
    }
}

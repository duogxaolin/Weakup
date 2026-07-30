//! Which Linux keep-awake mechanism to use, decided purely.
//!
//! Linux is the only target where keep-awake may be genuinely absent, so it is the
//! only one where task 8.6 — report unavailability honestly — has real work to do.
//! Probing the system needs Linux; *deciding what the probe means* does not, so the
//! decision lives here, always compiled and tested on every host. Same split as the
//! power-off classifiers.

/// The mechanisms, in preference order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinuxMechanism {
    /// `org.freedesktop.ScreenSaver.Inhibit`. Preferred because it is what actually
    /// stops the screen blanking under GNOME, KDE, and XFCE. The inhibition is bound
    /// to the D-Bus connection, so the connection has to outlive the job.
    DBusScreenSaver,

    /// `systemd-inhibit --what=idle:sleep`, holding a child process open.
    ///
    /// A weaker fallback, and worth being precise about why: this sets logind's idle
    /// and sleep inhibitors, which stop the machine suspending and stop logind's own
    /// IdleAction. It does not stop a desktop environment's own screen blanking.
    /// On a bare session with no D-Bus screensaver service that is the whole story
    /// anyway; under a desktop environment the D-Bus path above would have been
    /// chosen.
    SystemdInhibit,

    /// Neither is present. Keep-awake cannot work on this host.
    None,
}

/// What was found on the system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LinuxProbe {
    /// A session bus reachable *and* a name owner for `org.freedesktop.ScreenSaver`.
    ///
    /// Both, not either: a reachable bus with nothing implementing the interface
    /// would make `Inhibit` fail at call time, which would look like a broken
    /// mechanism rather than an absent one and would skip the fallback.
    pub dbus_screensaver: bool,

    /// `systemd-inhibit` present and executable.
    pub systemd_inhibit: bool,
}

impl LinuxProbe {
    pub fn new(dbus_screensaver: bool, systemd_inhibit: bool) -> Self {
        Self {
            dbus_screensaver,
            systemd_inhibit,
        }
    }
}

/// Picks the mechanism. D-Bus wins when available; `systemd-inhibit` is the fallback.
pub fn select_mechanism(probe: &LinuxProbe) -> LinuxMechanism {
    if probe.dbus_screensaver {
        LinuxMechanism::DBusScreenSaver
    } else if probe.systemd_inhibit {
        LinuxMechanism::SystemdInhibit
    } else {
        LinuxMechanism::None
    }
}

/// The reason keep-awake is unavailable, or `None` when it is available.
///
/// Names both things that were looked for. "Keep-awake is unavailable" with no
/// explanation leaves a user with no idea whether to install something, and this is
/// the one platform where installing something is the actual remedy.
pub fn unavailable_reason(probe: &LinuxProbe) -> Option<String> {
    match select_mechanism(probe) {
        LinuxMechanism::None => Some(
            "No screen-awake mechanism found: no D-Bus service owns \
             org.freedesktop.ScreenSaver, and systemd-inhibit is not installed. \
             Scheduled power-off is unaffected."
                .to_string(),
        ),
        _ => None,
    }
}

/// Whether the chosen mechanism can hold the *display* awake, as opposed to only
/// keeping the system from suspending.
///
/// The UI needs this to avoid promising something the fallback does not deliver.
pub fn holds_display_awake(mechanism: LinuxMechanism) -> bool {
    match mechanism {
        LinuxMechanism::DBusScreenSaver => true,
        LinuxMechanism::SystemdInhibit => false,
        LinuxMechanism::None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dbus_is_preferred_when_both_are_present() {
        // The fallback does not stop screen blanking, so preferring it when D-Bus
        // exists would silently downgrade the feature the user asked for.
        let probe = LinuxProbe::new(true, true);
        assert_eq!(select_mechanism(&probe), LinuxMechanism::DBusScreenSaver);
        assert!(unavailable_reason(&probe).is_none());
    }

    #[test]
    fn systemd_inhibit_is_used_when_dbus_is_absent() {
        let probe = LinuxProbe::new(false, true);
        assert_eq!(select_mechanism(&probe), LinuxMechanism::SystemdInhibit);
        assert!(unavailable_reason(&probe).is_none());
    }

    #[test]
    fn dbus_alone_is_enough() {
        let probe = LinuxProbe::new(true, false);
        assert_eq!(select_mechanism(&probe), LinuxMechanism::DBusScreenSaver);
        assert!(unavailable_reason(&probe).is_none());
    }

    #[test]
    fn neither_mechanism_is_reported_as_unavailable_with_both_causes_named() {
        // Task 8.6.
        let probe = LinuxProbe::default();
        assert_eq!(select_mechanism(&probe), LinuxMechanism::None);

        let reason = unavailable_reason(&probe).expect("a reason");
        assert!(reason.contains("org.freedesktop.ScreenSaver"));
        assert!(reason.contains("systemd-inhibit"));
        // Power-off is a separate feature and still works; saying so keeps the user
        // from assuming the whole app is broken.
        assert!(reason.contains("power-off"));
    }

    #[test]
    fn only_the_dbus_mechanism_claims_to_hold_the_display_awake() {
        assert!(holds_display_awake(LinuxMechanism::DBusScreenSaver));
        assert!(!holds_display_awake(LinuxMechanism::SystemdInhibit));
        assert!(!holds_display_awake(LinuxMechanism::None));
    }

    #[test]
    fn an_unavailable_mechanism_is_never_reported_as_available() {
        // Guards the invariant tying the two functions together: a reason exists
        // exactly when there is no mechanism.
        for dbus in [false, true] {
            for systemd in [false, true] {
                let probe = LinuxProbe::new(dbus, systemd);
                let mechanism = select_mechanism(&probe);
                let reason = unavailable_reason(&probe);

                assert_eq!(
                    mechanism == LinuxMechanism::None,
                    reason.is_some(),
                    "probe {probe:?} disagreed: mechanism {mechanism:?}, reason {reason:?}"
                );
            }
        }
    }
}

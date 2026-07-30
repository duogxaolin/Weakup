//! Power-off: the one irreversible action in the app.
//!
//! Structure, and the reason for it:
//!
//! - [`PowerOffExecutor`] is the injected trait (D6). The scheduler depends on it
//!   and never on a concrete implementation.
//! - [`classify`] holds *pure* functions mapping a captured exit status and
//!   stderr to an [`AppError`], one per OS. They are compiled and tested on every
//!   host, because classification is the part of the per-OS behaviour that can be
//!   verified without the OS — and two of the three targets cannot be run here.
//! - The executors that actually spawn a shutdown command are `#[cfg]`-gated, so
//!   only the host's own is in the binary (task 7.5). A pure string classifier for
//!   another OS cannot shut anything down; a process spawner can.
//!
//! Tests bind [`FakePowerOffExecutor`] and nothing else. A real executor in a test
//! would shut down the machine running it.

pub mod classify;

mod executor;
mod fake;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

pub use classify::CommandOutcome;
pub use executor::{PowerOffExecutor, UnsupportedPowerOffExecutor};
pub use fake::{FakePowerOffExecutor, FakeResponse};

#[cfg(target_os = "linux")]
pub use linux::LinuxPowerOffExecutor;
#[cfg(target_os = "macos")]
pub use macos::MacOsPowerOffExecutor;
#[cfg(target_os = "windows")]
pub use windows::WindowsPowerOffExecutor;

use std::sync::Arc;

/// The executor for the host OS.
///
/// Returns [`UnsupportedPowerOffExecutor`] on any target without a real
/// implementation, so the caller gets an honest `PowerOffUnsupported` rather than
/// a silent success.
pub fn host_executor() -> Arc<dyn PowerOffExecutor> {
    #[cfg(target_os = "macos")]
    {
        Arc::new(MacOsPowerOffExecutor::new())
    }
    #[cfg(target_os = "windows")]
    {
        Arc::new(WindowsPowerOffExecutor::new())
    }
    #[cfg(target_os = "linux")]
    {
        Arc::new(LinuxPowerOffExecutor::new())
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        Arc::new(UnsupportedPowerOffExecutor)
    }
}

#[cfg(test)]
mod classify_tests;

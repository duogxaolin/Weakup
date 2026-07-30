//! Keeping the screen awake.
//!
//! Layered so that the parts worth testing are testable everywhere:
//!
//! - `controller` — the trait, plus the controller used where no mechanism exists
//! - `coordinator` — decides *when* the assertion is held, from the set of jobs
//!   wanting it. Pure bookkeeping over the trait, so it carries the "no assertion
//!   outlives the last active job" guarantee (task 8.5) and is fully tested here.
//! - `mechanism` — which Linux mechanism to use, given what a probe found. Pure, so
//!   the Linux availability logic (task 8.6) runs on this host too.
//! - `macos` / `windows` / `linux` — the actual OS calls, `#[cfg]`-gated.
//!
//! Only the third group is unverifiable on a macOS development machine, and it is
//! deliberately the thinnest.

pub mod controller;
pub mod coordinator;
pub mod mechanism;

mod fake;

pub use controller::{KeepAwakeController, UnavailableKeepAwakeController};
pub use coordinator::{AssertionChange, KeepAwakeCoordinator};
pub use fake::{FakeKeepAwakeController, KeepAwakeEvent};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::MacosKeepAwakeController;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::WindowsKeepAwakeController;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::LinuxKeepAwakeController;

use std::sync::Arc;

/// The controller for the OS this binary was built for.
///
/// One `#[cfg]` chain, in one place. Everything above this line is OS-independent
/// and the tests bind a fake, so no test path reaches these constructors.
pub fn host_controller() -> Arc<dyn KeepAwakeController> {
    #[cfg(target_os = "macos")]
    {
        Arc::new(MacosKeepAwakeController::default())
    }

    #[cfg(target_os = "windows")]
    {
        Arc::new(WindowsKeepAwakeController::new())
    }

    #[cfg(target_os = "linux")]
    {
        Arc::new(LinuxKeepAwakeController::new())
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        // Android and iOS reach here. Keep-awake on mobile is the Flutter app's
        // problem; this crate does not target it.
        Arc::new(UnavailableKeepAwakeController::new(
            "Keep-awake is not implemented for this platform.",
        ))
    }
}

#[cfg(test)]
mod coordinator_tests;

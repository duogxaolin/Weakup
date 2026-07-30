//! The real power-off sources.
//!
//! The classifiers are pure and already run on the host. `windows` and `linux` are
//! the process-spawning executors that cannot be compiled by the main crate here.

#[path = "../../../../src-tauri/src/platform/power_off/classify.rs"]
pub mod classify;

#[path = "../../../../src-tauri/src/platform/power_off/executor.rs"]
pub mod executor;

#[cfg(target_os = "windows")]
#[path = "../../../../src-tauri/src/platform/power_off/windows.rs"]
pub mod windows;

#[cfg(target_os = "linux")]
#[path = "../../../../src-tauri/src/platform/power_off/linux.rs"]
pub mod linux;

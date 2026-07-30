//! The real keep-awake sources.
//!
//! `controller`, `mechanism`, and `coordinator` are OS-independent and already tested
//! on the host; they are here because the Windows and Linux implementations depend on
//! them. `windows` and `linux` are the point of this crate.

#[path = "../../../../src-tauri/src/platform/keep_awake/controller.rs"]
pub mod controller;

#[path = "../../../../src-tauri/src/platform/keep_awake/mechanism.rs"]
pub mod mechanism;

#[path = "../../../../src-tauri/src/platform/keep_awake/coordinator.rs"]
pub mod coordinator;

#[cfg(target_os = "windows")]
#[path = "../../../../src-tauri/src/platform/keep_awake/windows.rs"]
pub mod windows;

#[cfg(target_os = "linux")]
#[path = "../../../../src-tauri/src/platform/keep_awake/linux.rs"]
pub mod linux;

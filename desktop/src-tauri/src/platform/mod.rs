//! The boundary between the app's logic and the operating system.
//!
//! Everything OS-specific lives behind a trait defined here, for two reasons.
//! Tests bind a fake instead of the real thing — a test that bound a real
//! power-off executor would shut down the machine running it. And the pure parts
//! of each OS's behaviour (classifying what a command reported) stay compiled and
//! tested on every host, so the Windows and Linux paths are exercised even though
//! neither can be booted here.
//!
//! `#[cfg(target_os = ...)]` appears only around the code that genuinely touches
//! the OS, never around decision logic.

pub mod power_off;

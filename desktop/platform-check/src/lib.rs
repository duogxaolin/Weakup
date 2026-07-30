//! Compiles the real Windows and Linux platform code on a macOS machine.
//!
//! Every module reachable from here is the actual file the app ships, pulled in with
//! `#[path]`. Nothing is copied or reimplemented, so a wrong FFI signature, a bad
//! constant, or a misused zbus API in those files fails here — the only place it can
//! be caught without the target OS.
//!
//! What this does NOT do: prove the code behaves correctly on Windows or Linux. It
//! type-checks. Runtime behaviour on those two platforms remains unverified.
//!
//! The directory layout under `src/` mirrors the app's, because `#[path]` is resolved
//! against the declaring module's directory and `..` needs real directories to
//! traverse.

pub mod core;
pub mod platform;

/// Names the per-OS types and asserts they implement their traits.
///
/// A `pub` item is type-checked anyway; this additionally catches a missing or
/// mismatched trait impl, which is the failure most likely to slip through when the
/// implementation cannot be run.
#[cfg(target_os = "windows")]
pub fn windows_types_implement_their_traits() {
    fn assert_keep_awake<T: platform::keep_awake::controller::KeepAwakeController>() {}
    fn assert_power_off<T: platform::power_off::executor::PowerOffExecutor>() {}

    assert_keep_awake::<platform::keep_awake::windows::WindowsKeepAwakeController>();
    assert_power_off::<platform::power_off::windows::WindowsPowerOffExecutor>();
}

#[cfg(target_os = "linux")]
pub fn linux_types_implement_their_traits() {
    fn assert_keep_awake<T: platform::keep_awake::controller::KeepAwakeController>() {}
    fn assert_power_off<T: platform::power_off::executor::PowerOffExecutor>() {}

    assert_keep_awake::<platform::keep_awake::linux::LinuxKeepAwakeController>();
    assert_power_off::<platform::power_off::linux::LinuxPowerOffExecutor>();
}

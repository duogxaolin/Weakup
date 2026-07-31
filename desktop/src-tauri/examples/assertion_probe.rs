//! Proves the macOS assertion is real: acquires, asks pmset what the OS thinks,
//! releases, asks again. A unit test can only check our own bookkeeping.
//!
//! Only meaningful on macOS; on other targets the binary compiles but prints a
//! reminder and exits so CI does not reject the crate for having a platform-specific
//! example.

#[cfg(target_os = "macos")]
use weakup_lib::platform::keep_awake::{KeepAwakeController, MacosKeepAwakeController};

#[cfg(target_os = "macos")]
fn pmset_sees_our_assertion() -> bool {
    let out = std::process::Command::new("pmset").args(["-g", "assertions"]).output().unwrap();
    String::from_utf8_lossy(&out.stdout).contains("Weakup assertion probe")
}

fn main() {
    #[cfg(target_os = "macos")]
    {
        let c = MacosKeepAwakeController::new("Weakup assertion probe");
        println!("before acquire: pmset sees it = {}", pmset_sees_our_assertion());
        c.acquire().expect("acquire");
        let during = pmset_sees_our_assertion();
        println!("while held:     pmset sees it = {during}");
        c.release().expect("release");
        let after = pmset_sees_our_assertion();
        println!("after release:  pmset sees it = {after}");
        println!("{}", if during && !after { "ASSERTION_PROBE_OK" } else { "ASSERTION_PROBE_FAIL" });
    }

    #[cfg(not(target_os = "macos"))]
    println!("assertion_probe: macOS only — skipped on this platform.");
}

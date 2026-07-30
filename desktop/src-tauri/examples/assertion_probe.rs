//! Proves the macOS assertion is real: acquires, asks pmset what the OS thinks,
//! releases, asks again. A unit test can only check our own bookkeeping.
use weakup_lib::platform::keep_awake::{KeepAwakeController, MacosKeepAwakeController};

fn pmset_sees_our_assertion() -> bool {
    let out = std::process::Command::new("pmset").args(["-g", "assertions"]).output().unwrap();
    String::from_utf8_lossy(&out.stdout).contains("Weakup assertion probe")
}

fn main() {
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

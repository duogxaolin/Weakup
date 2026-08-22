//! The structural guarantees for the Firestore transport.
//!
//! # Why this file exists separately
//!
//! The tests below grep `firebase_transport.rs` for name fragments that must never appear in
//! it. Put inline as a `mod tests` in that file, each test's own string literals would be
//! part of the source it searches, and it would fail against itself. That is not
//! hypothetical — this codebase has hit it before, which is why `transport_tests.rs`,
//! `remote_command_tests.rs`, and `command_acceptance_tests.rs` are also separate files.
//!
//! # Why source text rather than review
//!
//! Because the failure is additive. Someone adds a helpful-looking flag to the transport,
//! meaning only to plumb through something the relay already knows; every existing test
//! still passes, and the guarantee is gone with nothing turning red. Source text is a blunt
//! instrument, but it is the mechanism already used here for exactly this class of rule, and
//! for a check whose whole job is to be hard to remove by accident, consistency beats
//! elegance.

/// The source of the Firestore transport, read at compile time.
const FIREBASE_TRANSPORT_SOURCE: &str = include_str!("firebase_transport.rs");

// ---------------------------------------------------------------------------
// The transport cannot vouch for a command.
// ---------------------------------------------------------------------------

#[test]
fn the_firestore_transport_contains_no_way_to_claim_a_command_is_genuine() {
    // This is the same check the transport trait carries, applied to the first real
    // implementation of it. The trait offering no such member is only half the guarantee:
    // an implementation could still grow its own field, return it from an inherent method,
    // and have a caller read it. A relay that could vouch for a command would make
    // compromise of the relay equivalent to control of every user's machine.
    //
    // Deliberately checked over the whole file, comments included. A doc comment describing
    // a field that does not exist is the first step towards the field existing, and a
    // reviewer skimming a diff sees prose as harmless.
    for forbidden in ["verified", "trusted", "attested", "authentic"] {
        let uses: Vec<&str> = FIREBASE_TRANSPORT_SOURCE
            .lines()
            .filter(|line| line.to_lowercase().contains(forbidden))
            .collect();

        assert!(
            uses.is_empty(),
            "firebase_transport.rs contains {forbidden:?}, which suggests a member by which \
             a relay could assert that a command is genuine. A transport moves bytes and \
             makes no such claim; the target checks the signature itself: {uses:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// The transport goes nowhere near the power-off path.
// ---------------------------------------------------------------------------

#[test]
fn the_firestore_transport_never_names_the_power_off_path() {
    // A remote request creates a *job*, which the existing scheduler runs by its existing
    // rules — including the mandatory cancellable countdown a remote-origin job earns. The
    // transport reaching the gate or the executor directly would be a second power-off path
    // with no countdown in front of it, and the whole safety structure rests on there being
    // exactly one.
    //
    // The same check the other remote modules carry, for the same reason: this is a failure
    // that arrives by addition, so its absence is asserted rather than assumed.
    for forbidden in [
        "power_off(",
        "PowerOffGate",
        "PowerOffExecutor",
        "GraceOutcome",
    ] {
        let uses: Vec<&str> = FIREBASE_TRANSPORT_SOURCE
            .lines()
            .filter(|line| line.contains(forbidden))
            .collect();

        assert!(
            uses.is_empty(),
            "firebase_transport.rs names {forbidden:?}. A transport delivers bytes; a remote \
             request becomes a job and the scheduler runs it, which is what keeps the \
             countdown mandatory: {uses:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// The presence interval is stated once, next to its reasoning.
// ---------------------------------------------------------------------------

#[test]
fn the_presence_interval_is_documented_against_the_online_threshold() {
    // The 60-second interval and the 90-second threshold are one decision expressed in two
    // files. A contributor changing the interval without reading why would silently change
    // what "online" means for every device, so the constant's comment must name the
    // threshold it is derived from — and this test fails if that link is deleted.
    let mentions_threshold = FIREBASE_TRANSPORT_SOURCE.contains("ONLINE_THRESHOLD_SECONDS");

    assert!(
        mentions_threshold,
        "the presence interval must be documented against ONLINE_THRESHOLD_SECONDS, because \
         the 90-second threshold was chosen to tolerate one missed report on this heartbeat; \
         changing one without the other silently redefines what online means"
    );
}

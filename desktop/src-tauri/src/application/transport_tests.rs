//! The structural guarantee for the transport seam.
//!
//! # Why this file exists separately
//!
//! The test below greps `transport.rs` for name fragments that must never appear in it. Put
//! inline as a `mod tests` in that file, the test's own string literals would be part of the
//! source it searches, and it would fail against itself. That is not hypothetical — this
//! codebase has hit it before, which is why `remote_command_tests.rs` and
//! `command_acceptance_tests.rs` are also separate files.
//!
//! # Why source text rather than review
//!
//! Because the failure is additive. Someone adds a helpful-looking flag to the trait,
//! meaning only to plumb through something a relay already knows; every existing test still
//! passes, and the guarantee is gone with nothing turning red. Source text is a blunt
//! instrument, but it is the mechanism already used here for exactly this class of rule, and
//! for a check whose whole job is to be hard to remove by accident, consistency beats
//! elegance.

use chrono::{TimeZone, Utc};

use crate::application::transport::{
    AccountId, AuthProvider, DevicePresenceRecord, FakeAuthProvider, FakeTransport,
    RemoteTransport, TransportFaults,
};
use crate::domain::DeviceId;

/// The source of the transport module, read at compile time.
const TRANSPORT_SOURCE: &str = include_str!("transport.rs");

// ---------------------------------------------------------------------------
// The structural guarantee. This is the test that must never be weakened.
// ---------------------------------------------------------------------------

#[test]
fn the_transport_interface_offers_no_way_to_claim_a_command_is_genuine() {
    // A transport that could vouch for a command would make compromise of the relay
    // equivalent to control of every user's machine. The interface therefore has no member
    // an implementation could use to say so — and because such a member would be *added*
    // rather than break anything existing, its absence is asserted rather than assumed.
    //
    // Deliberately checked over the whole file, comments included. A doc comment describing
    // a field that does not exist is the first step towards the field existing, and a
    // reviewer skimming a diff sees prose as harmless.
    for forbidden in ["verified", "trusted", "attested", "authentic"] {
        let uses: Vec<&str> = TRANSPORT_SOURCE
            .lines()
            .filter(|line| line.to_lowercase().contains(forbidden))
            .collect();

        assert!(
            uses.is_empty(),
            "the transport module contains {forbidden:?}, which suggests a member by which \
             a relay could assert that a command is genuine. A transport moves bytes and \
             makes no such claim; the target checks the signature itself: {uses:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// Liveness is data, not a verdict.
// ---------------------------------------------------------------------------

#[test]
fn the_transport_reports_an_instant_rather_than_an_online_state() {
    // Two devices asking the same relay about the same peer must not be able to receive
    // different answers, so the relay reports when a device last spoke and the presence rule
    // decides what that means, at the asking device.
    let transport = FakeTransport::new();
    let phone = DeviceId::new("phone-a").expect("non-empty");
    let at = Utc.with_ymd_and_hms(2026, 8, 6, 12, 0, 0).unwrap();

    transport
        .register_device(&phone, "Phone A")
        .expect("register");
    transport.report_presence(&phone, at).expect("presence");

    let devices = transport.list_devices().expect("list");
    let record: &DevicePresenceRecord = devices.first().expect("one device");

    assert_eq!(record.device_id, phone);
    assert_eq!(record.last_reported_at, Some(at));
}

#[test]
fn a_device_that_has_never_reported_presence_has_no_instant() {
    // `None` rather than a distant past instant: "never spoke" and "spoke long ago" are
    // different facts, and flattening them would make a brand-new device look stale.
    let transport = FakeTransport::new();
    let phone = DeviceId::new("phone-a").expect("non-empty");

    transport
        .register_device(&phone, "Phone A")
        .expect("register");

    let devices = transport.list_devices().expect("list");
    assert_eq!(devices[0].last_reported_at, None);
}

// ---------------------------------------------------------------------------
// The fake's faults behave as advertised.
// ---------------------------------------------------------------------------

#[test]
fn the_default_fake_is_well_behaved() {
    // A test that does not opt into a fault should read plainly, so the default must be
    // boring.
    let transport = FakeTransport::new();

    assert_eq!(transport.queued_count(&device()), 0);
}

#[test]
fn registering_the_same_device_twice_updates_it_rather_than_duplicating_it() {
    let transport = FakeTransport::new();
    let phone = device();

    transport.register_device(&phone, "Old name").expect("first");
    transport.register_device(&phone, "New name").expect("again");

    let devices = transport.list_devices().expect("list");
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0].display_name, "New name");
}

#[test]
fn faults_are_opt_in_and_independent() {
    let faults = TransportFaults {
        duplicate_delivery: true,
        ..TransportFaults::default()
    };

    assert!(faults.duplicate_delivery);
    assert!(!faults.drop_everything);
    assert!(!faults.delay_delivery);
    assert!(!faults.reorder_delivery);
}

// ---------------------------------------------------------------------------
// Account identity is a separate concern.
// ---------------------------------------------------------------------------

#[test]
fn the_auth_provider_reports_an_account_and_nothing_about_permission() {
    // The whole surface of the provider. It answers whose devices to show, and says nothing
    // about what may be commanded — pairing decides that, and the authorization rules
    // already state that same-account access is not sufficient.
    let provider = FakeAuthProvider::new(AccountId::new("account-1"));

    let account = provider.current_account().expect("account");
    assert_eq!(account.map(|a| a.as_str().to_string()), Some("account-1".to_string()));
}

#[test]
fn a_signed_out_device_reports_no_account() {
    let provider = FakeAuthProvider::signed_out();

    assert_eq!(provider.current_account().expect("account"), None);
}

#[test]
fn the_two_interfaces_are_separately_implementable() {
    // Stated as a compiling fact rather than as prose: each is satisfied by a type that
    // does not implement the other, so replacing one cannot require touching the other.
    fn takes_transport(_: &dyn RemoteTransport) {}
    fn takes_auth(_: &dyn AuthProvider) {}

    takes_transport(&FakeTransport::new());
    takes_auth(&FakeAuthProvider::signed_out());
}

fn device() -> DeviceId {
    DeviceId::new("phone-a").expect("non-empty")
}

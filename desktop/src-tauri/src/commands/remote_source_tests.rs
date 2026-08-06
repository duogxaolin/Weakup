//! Structural rules about the command module, checked against its source text.
//!
//! In its own file, not an inline `mod tests` inside `mod.rs`. That is not a style
//! preference: a grep for `"pub fn power_off"` written inside the file it greps matches its
//! own string literal, so the test passes whether or not the rule holds. This codebase has
//! hit that trap before, which is why `transport_tests.rs`,
//! `command_acceptance_tests.rs`, and `firebase_transport_source_tests.rs` all sit beside
//! the files they check rather than inside them.
//!
//! `commands/tests.rs` already carries the checks that predate the remote surface. This file
//! carries the ones the remote surface adds, which are all versions of one claim: adding a
//! network does not add a second route to the executor.

use crate::commands::dto::{DeviceView, PairingView};

/// The command module's source, read at compile time.
const COMMANDS_SOURCE: &str = include_str!("mod.rs");

/// The registration list's source. A command defined and not registered is unreachable; one
/// registered and not defined does not compile.
const LIB_SOURCE: &str = include_str!("../lib.rs");

/// The DTO module, where presence is derived.
const DTO_SOURCE: &str = include_str!("dto.rs");

/// Lines of `source` that are not comments.
///
/// Doc comments naming a forbidden symbol are the point of the module docs — the remote
/// surface's own header explains at length why there is no power-off command, and naming it
/// there must not fail the build.
fn code_lines(source: &str) -> impl Iterator<Item = &str> {
    source.lines().filter(|line| {
        let trimmed = line.trim_start();
        !trimmed.starts_with("//") && !trimmed.starts_with("*") && !trimmed.starts_with("/*")
    })
}

#[test]
fn the_remote_surface_adds_no_command_that_powers_the_machine_off() {
    // Task 7.1's central constraint. A remote request creates a *job*; the scheduler runs
    // it, and a `JobOrigin::Remote` job counts down for `REMOTE_GRACE_PERIOD_SECONDS`. A
    // command here that shut the machine down would be a second path to the executor, and
    // the countdown would become the usual case rather than the rule.
    //
    // `commands/tests.rs` asserts the same for the three names it knew about. This adds the
    // shapes a remote surface specifically invites.
    for forbidden in [
        "pub fn power_off",
        "pub fn shutdown_now",
        "pub fn skip_grace",
        "pub fn power_off_now",
        "pub fn remote_power_off",
        "pub fn execute_remote_command",
        "PowerOffExecutor",
        "PowerOffGate",
        "host_executor",
    ] {
        let hits: Vec<&str> = code_lines(COMMANDS_SOURCE)
            .filter(|line| line.contains(forbidden))
            .collect();

        assert!(
            hits.is_empty(),
            "a command references {forbidden}, which would let a remote request bypass \
             the countdown: {hits:?}"
        );
    }
}

#[test]
fn no_registered_command_names_a_shutdown() {
    // The other half of the same rule, read from the registration list rather than the
    // definitions. A command could in principle be defined in another module and
    // registered here, which the source check above would not see.
    let handler = LIB_SOURCE
        .split_once("generate_handler!")
        .expect("the handler list exists")
        .1;
    let list = &handler[..handler.find("])").expect("the list ends")];

    let registered: Vec<&str> = list
        .lines()
        .filter_map(|line| line.trim().strip_prefix("commands::"))
        .map(|name| name.trim_end_matches(','))
        .collect();

    assert!(
        registered.len() >= 30,
        "parsed {} registrations, which is not the real list",
        registered.len()
    );

    // Filtered on what a name *means*, not on whether it contains a substring.
    //
    // `check_shutdown_permission` is registered and contains "shutdown", and it is
    // deliberately exempt: it asks the OS about Automation consent through
    // `AEDeterminePermissionToAutomateTarget`, which reports *without* sending an Apple
    // Event. It holds no executor and cannot shut anything down. `commands/tests.rs`
    // separately asserts its body reaches for neither an executor nor `Command::new`, which
    // is what makes the exemption safe rather than merely asserted.
    //
    // `cancel_grace_period` is likewise exempt and in the opposite direction: it can only
    // ever prevent a shutdown.
    const ASKS_RATHER_THAN_ACTS: [&str; 2] = ["check_shutdown_permission", "cancel_grace_period"];

    let actors: Vec<&&str> = registered
        .iter()
        .filter(|name| !ASKS_RATHER_THAN_ACTS.contains(name))
        .filter(|name| {
            ["power_off", "shutdown", "force_", "execute_power", "skip_grace"]
                .iter()
                .any(|forbidden| name.contains(forbidden))
        })
        .collect();

    assert!(
        actors.is_empty(),
        "registered commands that could reach a shutdown directly: {actors:?}"
    );

    // And the countdown command that *is* registered can only ever stop one.
    assert!(registered.contains(&"cancel_grace_period"));
}

#[test]
fn every_remote_command_is_registered() {
    // A `#[tauri::command]` that is never registered compiles, exports nothing, and fails
    // at runtime with "command not found" — which in this surface would look like pairing
    // silently not working.
    let handler = LIB_SOURCE
        .split_once("generate_handler!")
        .expect("the handler list exists")
        .1;
    let list = &handler[..handler.find("])").expect("the list ends")];

    for command in [
        "account_state",
        "sign_in_to_account",
        "sign_out_of_account",
        "present_pairing_code",
        "accept_pairing_code",
        "list_pairings",
        "revoke_pairing",
        "list_devices",
        "remote_control_enabled",
        "set_remote_control_enabled",
    ] {
        assert!(
            COMMANDS_SOURCE.contains(&format!("pub fn {command}")),
            "{command} is registered but not defined here"
        );
        assert!(
            list.contains(&format!("commands::{command}")),
            "{command} is defined but never registered, so the UI cannot reach it"
        );
    }
}

#[test]
fn presence_is_derived_by_the_rule_and_never_read_from_a_field() {
    // Task 7.2. The relay is untrusted, so a boolean from it is a claim rather than a fact,
    // and two devices asking the same relay could otherwise disagree about the same peer.
    //
    // Checked structurally as well as behaviourally (see the tests below) because the
    // tempting shortcut is not a wrong computation — it is skipping the computation and
    // passing a field straight through.
    assert!(
        DTO_SOURCE.contains("evaluate_presence"),
        "the device views no longer run the presence rule"
    );

    for forbidden in [
        "is_online",
        "isOnline",
        "online: bool",
        "pub online",
        "reported_online",
    ] {
        let hits: Vec<&str> = code_lines(DTO_SOURCE)
            .chain(code_lines(COMMANDS_SOURCE))
            .filter(|line| line.contains(forbidden))
            .collect();

        assert!(
            hits.is_empty(),
            "the surface carries {forbidden}, which would let a relay assert presence \
             rather than have it derived: {hits:?}"
        );
    }
}

#[test]
fn a_peer_the_relay_says_nothing_about_is_offline_rather_than_unknown() {
    // The behavioural half of the rule above, at the boundary that matters: a pairing whose
    // peer has never reported. `None` must go through the presence rule and come out
    // `Offline` with no age, not be special-cased into a fourth state the UI would have to
    // invent a label for.
    use crate::data::PairingRecord;
    use crate::domain::{DeviceId, VerifyingKey};

    let record = PairingRecord {
        peer: DeviceId::new("peer-that-never-reported").expect("a valid id"),
        verifying_key: VerifyingKey::from_bytes(&[
            0xd7, 0x5a, 0x98, 0x01, 0x82, 0xb1, 0x0a, 0xb7, 0xd5, 0x4b, 0xfe, 0xd3, 0xc9, 0x64,
            0x07, 0x3a, 0x0e, 0xe1, 0x72, 0xf3, 0xda, 0xa6, 0x23, 0x25, 0xaf, 0x02, 0x1a, 0x68,
            0xf7, 0x07, 0x51, 0x1a,
        ])
        .expect("a valid key"),
        revoked: false,
    };

    let view = PairingView::from_record(&record, None, chrono::Utc::now());

    assert_eq!(view.presence, "offline");
    assert_eq!(
        view.elapsed_seconds, None,
        "a device that has never reported has no age; zero would read as 'just now'"
    );
}

#[test]
fn presence_changes_with_the_reported_instant_rather_than_being_fixed() {
    // A view that hardcoded "offline", or that ignored `last_reported_at`, would satisfy
    // the previous test. This one walks all three states across the rule's own boundaries,
    // so a constant cannot pass.
    use crate::application::transport::DevicePresenceRecord;
    use crate::domain::{DeviceId, OFFLINE_THRESHOLD_SECONDS, ONLINE_THRESHOLD_SECONDS};

    let now = chrono::Utc::now();
    let device_id = DeviceId::new("peer-1").expect("a valid id");

    for (silent_for, expected) in [
        (0, "online"),
        (ONLINE_THRESHOLD_SECONDS, "online"),
        (ONLINE_THRESHOLD_SECONDS + 1, "stale"),
        (OFFLINE_THRESHOLD_SECONDS, "stale"),
        (OFFLINE_THRESHOLD_SECONDS + 1, "offline"),
    ] {
        let record = DevicePresenceRecord {
            device_id: device_id.clone(),
            display_name: "A phone".to_string(),
            last_reported_at: Some(now - chrono::Duration::seconds(silent_for)),
        };

        let view = DeviceView::from_record(&record, now, false, false);

        assert_eq!(
            view.presence, expected,
            "a device silent for {silent_for}s should read {expected}"
        );
        assert_eq!(view.elapsed_seconds, Some(silent_for));
    }
}

#[test]
fn presence_is_three_states_and_never_a_boolean() {
    // The `Stale` state is the whole reason this is not a boolean. A backgrounded phone
    // reported as offline reads as broken; reported as online it makes a command that never
    // arrives look ignored. Any collapse to two values loses one of those.
    let states = crate::commands::dto::presence_states();

    assert_eq!(states, vec!["online", "stale", "offline"]);
    assert_eq!(states.len(), 3, "presence must not collapse to a boolean");
}

#[test]
fn a_revoked_pairing_is_still_listed_but_not_reported_as_paired() {
    // The capability requires the owner see what a device has been paired with. A row that
    // vanishes on revocation makes "was this device ever paired?" unanswerable, which is
    // the question that matters most after a device is lost.
    //
    // `revoked` is what the UI greys the row out by, and it must be the *only* thing that
    // changes: the peer's identifier stays visible.
    use crate::data::PairingRecord;
    use crate::domain::{DeviceId, VerifyingKey};

    let key = VerifyingKey::from_bytes(&[
        0xd7, 0x5a, 0x98, 0x01, 0x82, 0xb1, 0x0a, 0xb7, 0xd5, 0x4b, 0xfe, 0xd3, 0xc9, 0x64, 0x07,
        0x3a, 0x0e, 0xe1, 0x72, 0xf3, 0xda, 0xa6, 0x23, 0x25, 0xaf, 0x02, 0x1a, 0x68, 0xf7, 0x07,
        0x51, 0x1a,
    ])
    .expect("a valid key");

    let revoked = PairingView::from_record(
        &PairingRecord {
            peer: DeviceId::new("lost-phone").expect("a valid id"),
            verifying_key: key,
            revoked: true,
        },
        None,
        chrono::Utc::now(),
    );

    assert!(revoked.revoked);
    assert_eq!(revoked.peer, "lost-phone", "a revoked peer must stay visible");
}

#[test]
fn the_remote_control_setting_is_written_by_its_own_command_only() {
    // Two hazards, one rule.
    //
    // A stale form posting a whole `Settings` back would silently re-enable a setting the
    // user had just turned off — the same hazard the camelCase tests guard, with a worse
    // consequence. So the setting has a command of its own that takes one boolean.
    //
    // And nothing arriving from a peer routes to it: `RemoteCommand::EnableRemoteControl`
    // is refused by `authorize` before the pairing check runs, and no envelope is dispatched
    // to a command function at all.
    assert!(
        COMMANDS_SOURCE.contains("pub fn set_remote_control_enabled"),
        "the setting has no command of its own"
    );

    let command = COMMANDS_SOURCE
        .split_once("pub fn set_remote_control_enabled")
        .expect("the command exists")
        .1;
    let body = &command[..command.find("\n}").expect("the body ends")];

    // It must not accept an envelope, a peer, or a signature — any of which would mean a
    // remote request could reach it.
    for forbidden in ["CommandEnvelope", "envelope", "peer:", "signature"] {
        assert!(
            !body.contains(forbidden),
            "the remote-control setter takes {forbidden}, so a peer could change the \
             setting that decides whether peers are obeyed"
        );
    }
}

#[test]
fn disabling_remote_control_touches_no_pairing() {
    // "Disabling is not revoking." The setter writes one settings row and counts the
    // pairings either side of the write, so the claim is checked at runtime rather than
    // only in prose. Asserted here on the source because the count itself needs a running
    // app; what a file can show is that the check exists and that no revocation is reachable.
    let command = COMMANDS_SOURCE
        .split_once("pub fn set_remote_control_enabled")
        .expect("the command exists")
        .1;
    let body = &command[..command.find("\n}").expect("the body ends")];

    assert!(
        body.contains("pairing_count"),
        "the setter does not check that it left the pairings alone"
    );

    for forbidden in ["revoke_pairing", "record_pairing", "withdraw_pairing"] {
        assert!(
            !body.contains(forbidden),
            "disabling remote control reaches {forbidden}; a user who wanted a quiet \
             evening would have to re-pair every device the next morning"
        );
    }
}

#[test]
fn signing_out_touches_no_pairing() {
    // Same shape, different act. A sign-out that revoked pairings would mean an expired
    // token silently de-authorized a machine the user is standing in front of.
    let command = COMMANDS_SOURCE
        .split_once("pub fn sign_out_of_account")
        .expect("the command exists")
        .1;
    let body = &command[..command.find("\n}").expect("the body ends")];

    assert!(
        body.contains("list_pairings"),
        "the sign-out does not check that it left the pairings alone"
    );

    for forbidden in ["revoke_pairing", "withdraw_pairing"] {
        assert!(
            !body.contains(forbidden),
            "signing out reaches {forbidden}, which would de-authorize a device the \
             identity provider was never granted authority over"
        );
    }
}

#[test]
fn the_grace_length_is_never_named_at_the_surface() {
    // Neither 60 nor 300 belongs here. The origin decides, through
    // `grace_period_seconds(job.origin)`, and a constant repeated at the surface is how the
    // UI comes to draw a 60-second countdown over a 300-second wait.
    let hits: Vec<&str> = code_lines(COMMANDS_SOURCE)
        .filter(|line| line.contains("REMOTE_GRACE_PERIOD_SECONDS") || line.contains("= 300"))
        .collect();

    assert!(
        hits.is_empty(),
        "the surface names a countdown length instead of letting the origin decide: {hits:?}"
    );
}

#[test]
fn the_outstanding_code_map_is_swept() {
    // The loose end Batch D left: `PairingCodeIssuer::forget_stale` existed and nothing
    // called it, so the map grew for the life of a process that runs for weeks.
    //
    // Named here rather than only in `remote_state.rs`'s own tests so that removing the
    // sweep fails a test whose name says what was lost.
    let state_source = include_str!("remote_state.rs");

    assert!(
        state_source.contains("forget_stale"),
        "nothing sweeps the outstanding pairing codes, so the map grows without bound"
    );
}

#[test]
fn a_malformed_peer_key_is_refused_rather_than_truncated() {
    // The hex decoder at the boundary. A permissive decoder that ignored a bad digit would
    // produce *a* key — just not the peer's — and the pairing would record something the
    // peer cannot sign as.
    for bad in ["", "abc", "zz", "d75a98g1", " d75a98"] {
        assert!(
            super::decode_hex(bad).is_none(),
            "accepted {bad:?} as a key"
        );
    }

    assert_eq!(super::decode_hex("00ff10"), Some(vec![0x00, 0xff, 0x10]));
    assert_eq!(super::decode_hex("D75A"), Some(vec![0xd7, 0x5a]));
}

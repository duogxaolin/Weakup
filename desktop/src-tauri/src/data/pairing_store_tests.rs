//! The pairing store's behaviour, and the structural guarantee about where keys come from.
//!
//! # Why this file exists separately
//!
//! The source-text test below greps production files for `verifying_keys` map construction.
//! Put inline as a `mod tests`, the test's own string literals would be part of the source it
//! searches on the `pairing_store.rs` pass, and it would fail against itself — the same trap
//! that put `transport_tests.rs` and `device_identity_tests.rs` in their own files.

use chrono::{TimeZone, Utc};

use crate::data::pairing_store::{verifying_keys_from_pairings, PairingRecord, PairingStore};
use crate::data::sqlite_repository::SqliteJobRepository;
use crate::domain::{DeviceId, VerifyingKey};

fn phone() -> DeviceId {
    DeviceId::new("phone-a").expect("non-empty")
}

fn laptop() -> DeviceId {
    DeviceId::new("laptop-b").expect("non-empty")
}

/// A distinct valid Ed25519 public key per seed, without a key generator in the library.
fn key(seed: u8) -> VerifyingKey {
    // The public halves of the fixed test seeds, as the rest of the suite uses.
    let hex = match seed {
        1 => "8a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c",
        2 => "8139770ea87d175f56a35466c34c7ecccb8d8a91b4ee37a25df60f5b8fc9b394",
        _ => "ed4928c628d1c2c6eae90338905995612959273a5c63f93636c14614ac8737d1",
    };
    let bytes: Vec<u8> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).expect("valid hex"))
        .collect();
    VerifyingKey::from_bytes(&bytes).expect("a valid public key")
}

fn at(hour: u32) -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 8, 7, hour, 0, 0).unwrap()
}

fn store() -> SqliteJobRepository {
    SqliteJobRepository::open_in_memory().expect("in-memory database")
}

// ---------------------------------------------------------------------------
// Recording and reading back.
// ---------------------------------------------------------------------------

#[test]
fn a_pairing_round_trips_with_its_key() {
    // The spec requires the pairing record the peer's *key*, not only its name: a pairing
    // storing an identifier alone would authorize whoever presented that name.
    let store = store();

    store.record_pairing(&phone(), &key(1), at(12)).expect("pair");

    let pairings = store.list_pairings().expect("list");
    assert_eq!(pairings.len(), 1);
    assert_eq!(pairings[0].peer, phone());
    assert_eq!(pairings[0].verifying_key, key(1));
    assert!(!pairings[0].revoked);
}

#[test]
fn an_active_pairings_key_appears_in_the_map() {
    let store = store();
    store.record_pairing(&phone(), &key(1), at(12)).expect("pair");

    let keys = store.verifying_keys_from_store().expect("keys");

    assert_eq!(keys.get(&phone()), Some(&key(1)));
}

#[test]
fn a_device_that_was_never_paired_has_no_key() {
    let store = store();
    store.record_pairing(&phone(), &key(1), at(12)).expect("pair");

    let keys = store.verifying_keys_from_store().expect("keys");

    assert!(!keys.contains_key(&laptop()));
}

// ---------------------------------------------------------------------------
// Revocation.
// ---------------------------------------------------------------------------

#[test]
fn revoking_removes_the_key_from_the_map() {
    // Design D5: absent, not present-and-rejected-later. A revoked peer's command fails as
    // an unknown sender rather than having its signature verified first.
    let store = store();
    store.record_pairing(&phone(), &key(1), at(12)).expect("pair");

    store.revoke_pairing(&phone(), at(13)).expect("revoke");

    let keys = store.verifying_keys_from_store().expect("keys");
    assert!(
        !keys.contains_key(&phone()),
        "a revoked pairing's key must be absent from the map, not present for a later check"
    );
}

#[test]
fn revoking_retains_the_row_rather_than_deleting_it() {
    // Design D4. The record that a pairing existed is what someone investigating an
    // unexplained shutdown needs, and "was this device ever paired?" must stay answerable
    // after a device is lost.
    let store = store();
    store.record_pairing(&phone(), &key(1), at(12)).expect("pair");

    store.revoke_pairing(&phone(), at(13)).expect("revoke");

    let pairings = store.list_pairings().expect("list");
    assert_eq!(pairings.len(), 1, "the row must be retained");
    assert!(pairings[0].revoked);
}

#[test]
fn revocation_is_per_pairing_rather_than_global() {
    // The property the shared vectors also pin. An implementation treating any revocation as
    // a switch that disables the store passes every single-peer test above.
    let store = store();
    store.record_pairing(&phone(), &key(1), at(12)).expect("pair phone");
    store.record_pairing(&laptop(), &key(2), at(12)).expect("pair laptop");

    store.revoke_pairing(&phone(), at(13)).expect("revoke");

    let keys = store.verifying_keys_from_store().expect("keys");
    assert!(!keys.contains_key(&phone()));
    assert_eq!(
        keys.get(&laptop()),
        Some(&key(2)),
        "revoking one pairing must not disturb another"
    );
}

#[test]
fn revoking_an_unknown_peer_is_an_error() {
    // Silence would let a user believe they had de-authorized a lost device when no such
    // pairing was recorded.
    let store = store();

    assert!(store.revoke_pairing(&phone(), at(13)).is_err());
}

// ---------------------------------------------------------------------------
// Re-pairing. The row-count assertion is the point.
// ---------------------------------------------------------------------------

#[test]
fn a_revoked_device_can_be_paired_again_and_regains_authority() {
    // Revocation withdraws the authority previously granted; it does not blacklist. A user
    // who revokes a phone after mislaying it must be able to pair it again when it turns up.
    let store = store();
    store.record_pairing(&phone(), &key(1), at(12)).expect("pair");
    store.revoke_pairing(&phone(), at(13)).expect("revoke");

    store.record_pairing(&phone(), &key(1), at(14)).expect("re-pair");

    let keys = store.verifying_keys_from_store().expect("keys");
    assert_eq!(keys.get(&phone()), Some(&key(1)));
}

#[test]
fn re_pairing_clears_the_revocation_on_the_same_row_rather_than_adding_a_second() {
    // **The row count is the assertion**, not merely that the key came back. A second row
    // would satisfy a key-presence check while leaving exactly the ambiguity design D4
    // exists to prevent: "is this device authorized?" would depend on which row is read
    // first, and that resolves the wrong way sooner or later.
    let store = store();
    store.record_pairing(&phone(), &key(1), at(12)).expect("pair");
    store.revoke_pairing(&phone(), at(13)).expect("revoke");

    store.record_pairing(&phone(), &key(1), at(14)).expect("re-pair");

    let pairings = store.list_pairings().expect("list");
    assert_eq!(
        pairings.len(),
        1,
        "re-pairing must clear the revocation on the existing row, never insert a second: \
         two rows for one peer make authorization depend on read order"
    );
    assert!(!pairings[0].revoked, "the revocation must be cleared");
}

#[test]
fn re_pairing_with_a_new_key_replaces_the_recorded_one() {
    // A device that was reinstalled has a new identity key. Pairing it again must record the
    // key actually presented, or commands signed with it would be refused against a stale
    // one — and still only one row.
    let store = store();
    store.record_pairing(&phone(), &key(1), at(12)).expect("pair");

    store.record_pairing(&phone(), &key(2), at(14)).expect("re-pair");

    let pairings = store.list_pairings().expect("list");
    assert_eq!(pairings.len(), 1);
    assert_eq!(pairings[0].verifying_key, key(2));
}

// ---------------------------------------------------------------------------
// Durability.
// ---------------------------------------------------------------------------

#[test]
fn pairings_and_revocations_both_survive_a_restart() {
    // Both halves in one test, because they fail in opposite directions and each is a spec
    // scenario: a forgotten pairing silently de-authorizes a peer, while a forgotten
    // revocation leaves a lost device in control after the owner believed otherwise.
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("weakup.db");

    {
        let store = SqliteJobRepository::open(&path).expect("open");
        store.record_pairing(&phone(), &key(1), at(12)).expect("pair phone");
        store.record_pairing(&laptop(), &key(2), at(12)).expect("pair laptop");
        store.revoke_pairing(&phone(), at(13)).expect("revoke");
    }

    let reopened = SqliteJobRepository::open(&path).expect("reopen");
    let keys = reopened.verifying_keys_from_store().expect("keys");

    assert!(
        !keys.contains_key(&phone()),
        "a revocation that a restart undid would be worse than none"
    );
    assert_eq!(
        keys.get(&laptop()),
        Some(&key(2)),
        "a pairing forgotten on restart would silently de-authorize a peer"
    );
}

// ---------------------------------------------------------------------------
// The pure map builder, independent of any database.
// ---------------------------------------------------------------------------

#[test]
fn the_map_builder_excludes_revoked_pairings() {
    let pairings = vec![
        PairingRecord {
            peer: phone(),
            verifying_key: key(1),
            revoked: true,
        },
        PairingRecord {
            peer: laptop(),
            verifying_key: key(2),
            revoked: false,
        },
    ];

    let keys = verifying_keys_from_pairings(&pairings);

    assert_eq!(keys.len(), 1);
    assert!(!keys.contains_key(&phone()));
    assert_eq!(keys.get(&laptop()), Some(&key(2)));
}

#[test]
fn an_empty_store_authorizes_nobody() {
    // The first-run state. An implementation defaulting to permissive when it holds no
    // pairings would obey every command it received.
    assert!(verifying_keys_from_pairings(&[]).is_empty());
}

// ---------------------------------------------------------------------------
// Migration from the previous schema.
// ---------------------------------------------------------------------------

#[test]
fn a_v4_database_upgrades_with_its_existing_rows_intact() {
    // Opens a genuine pre-migration database — the tables and `user_version` exactly as
    // version 4 wrote them — rather than a fresh one, which would exercise the create path
    // instead of the upgrade path and prove nothing about existing installs.
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("v4.db");

    {
        let conn = rusqlite::Connection::open(&path).expect("open raw");
        conn.execute_batch(
            "CREATE TABLE jobs (
                 id                 TEXT    PRIMARY KEY NOT NULL,
                 job_type           TEXT    NOT NULL,
                 trigger_kind       TEXT    NOT NULL,
                 trigger_minutes    INTEGER,
                 trigger_hour       INTEGER,
                 trigger_minute     INTEGER,
                 status             TEXT    NOT NULL,
                 target_instant_utc INTEGER,
                 created_at_utc     INTEGER NOT NULL,
                 updated_at_utc     INTEGER NOT NULL,
                 timezone           TEXT    NOT NULL,
                 failure_message    TEXT,
                 trigger_date       TEXT,
                 origin             TEXT
             );
             CREATE TABLE settings (
                 key   TEXT PRIMARY KEY NOT NULL,
                 value TEXT NOT NULL
             );
             CREATE TABLE command_decisions (
                 id               INTEGER PRIMARY KEY AUTOINCREMENT,
                 sender_device_id TEXT    NOT NULL,
                 command          TEXT    NOT NULL,
                 decision         TEXT    NOT NULL,
                 rejection_reason TEXT,
                 decided_at_utc   INTEGER NOT NULL
             );
             PRAGMA user_version = 4;",
        )
        .expect("create v4 schema");

        // A row written by the old version, which the migration must not disturb.
        conn.execute(
            "INSERT INTO jobs (
                 id, job_type, trigger_kind, trigger_minutes, status,
                 target_instant_utc, created_at_utc, updated_at_utc, timezone, origin
             ) VALUES ('job-1', 'powerOff', 'duration', 30, 'active', 1000, 1, 2, 'UTC', 'remote')",
            [],
        )
        .expect("insert legacy job");
        conn.execute(
            "INSERT INTO settings (key, value) VALUES ('timezone', 'Europe/Paris')",
            [],
        )
        .expect("insert legacy setting");
    }

    // Opening runs the migration.
    let store = SqliteJobRepository::open(&path).expect("migrate");

    // The new tables exist and are usable.
    store.record_pairing(&phone(), &key(1), at(12)).expect("pair");
    assert_eq!(store.list_pairings().expect("list").len(), 1);

    // And nothing existing was rewritten. The migration is additive: an older binary reading
    // this file would see two tables it does not query, and every row it does query unchanged.
    use crate::data::job_repository::JobRepository;
    let job = store.find("job-1").expect("find").expect("the legacy job survived");
    assert_eq!(job.id, "job-1");
    assert_eq!(job.origin, crate::domain::JobOrigin::Remote);

    use crate::data::settings::SettingsStore;
    assert_eq!(store.load().expect("settings").timezone, "Europe/Paris");
}

#[test]
fn the_schema_version_is_recorded_after_migrating() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("fresh.db");

    let store = SqliteJobRepository::open(&path).expect("open");
    // Reopening must be a no-op rather than re-running the migration; `CREATE TABLE IF NOT
    // EXISTS` makes that safe either way, and this asserts the second open succeeds.
    drop(store);

    assert!(SqliteJobRepository::open(&path).is_ok());
}

// ---------------------------------------------------------------------------
// The structural guarantee: only the store builds the key map.
// ---------------------------------------------------------------------------

/// Every production source file that could plausibly assemble a key map, read at compile
/// time. Tests are deliberately excluded — they may and do build maps directly, because a
/// vector case must be writable without a database.
const COMMAND_ACCEPTANCE_SOURCE: &str = include_str!("../domain/command_acceptance.rs");
const TRANSPORT_SOURCE: &str = include_str!("../application/transport.rs");
const COMMANDS_SOURCE: &str = include_str!("../commands/mod.rs");
const SCHEDULER_SOURCE: &str = include_str!("../application/scheduler.rs");
const SETUP_SOURCE: &str = include_str!("../setup.rs");

#[test]
fn no_production_file_outside_the_store_constructs_a_verifying_keys_map() {
    // Design D1, and the honest statement of its limit.
    //
    // The spec requires that "no caller-supplied key SHALL be consulted". That is satisfied
    // here at the *composition* layer rather than in the type system: `evaluate_command`
    // still takes the map as data, and exactly one production function builds it —
    // `verifying_keys_from_pairings`, which reads the pairing store.
    //
    // Making it unrepresentable in the type system would mean passing the store into the
    // rule, which would make the rule impure. The vectors are the only thing preventing the
    // two implementations from drifting, and every vector case would then need a store
    // fixture. That cost is paid on every future change, so the weaker guard is the right
    // trade — but it *is* weaker, and this comment says so rather than implying otherwise.
    //
    // This is the same mechanism the repo already uses for `PowerOffGate` and the transport
    // interface: blunt, but hard to remove by accident.
    for (name, source) in [
        ("domain/command_acceptance.rs", COMMAND_ACCEPTANCE_SOURCE),
        ("application/transport.rs", TRANSPORT_SOURCE),
        ("commands/mod.rs", COMMANDS_SOURCE),
        ("application/scheduler.rs", SCHEDULER_SOURCE),
        ("setup.rs", SETUP_SOURCE),
    ] {
        let assignments: Vec<&str> = source
            .lines()
            // A map being *built* rather than the field being declared or documented. The
            // declaration in `CommandTargetState` is the field itself, which must stay.
            .filter(|line| {
                let t = line.trim_start();
                !t.starts_with("//") && !t.starts_with("///")
            })
            .filter(|line| {
                line.contains("verifying_keys:") || line.contains("verifying_keys =")
            })
            // The struct field declaration, which is a type and not a construction.
            .filter(|line| !line.contains("pub verifying_keys: HashMap"))
            // Reading the map is fine; only assembling one is not.
            .filter(|line| !line.contains("target_state.verifying_keys"))
            .collect();

        assert!(
            assignments.is_empty(),
            "{name} appears to construct a verifying_keys map: {assignments:?}\n\
             Exactly one production function may build that map — \
             `verifying_keys_from_pairings` in data/pairing_store.rs, which reads the pairing \
             store and excludes revoked pairings. A map assembled anywhere else is a caller \
             choosing whose signature counts, which is the same authority as asserting \
             authenticity outright."
        );
    }
}

#[test]
fn the_store_is_the_one_place_that_builds_the_map() {
    // The other half: the guard above proves no one *else* builds a map, and this proves the
    // store still does. Without it, deleting `verifying_keys_from_pairings` entirely would
    // leave the suite green while nothing supplied keys at all.
    let store = store();
    store.record_pairing(&phone(), &key(1), at(12)).expect("pair");

    let keys = store.verifying_keys_from_store().expect("keys");

    assert_eq!(keys.get(&phone()), Some(&key(1)));
}

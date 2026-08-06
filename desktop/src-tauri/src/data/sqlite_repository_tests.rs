use chrono::{DateTime, NaiveDate, TimeZone, Utc};

use crate::core::AppError;
use crate::data::job_repository::JobRepository;
use crate::data::sqlite_repository::SqliteJobRepository;
use crate::domain::{Job, JobOrigin, JobStatus, JobType, TriggerSpec};

fn utc(y: i32, m: u32, d: u32, h: u32, min: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(y, m, d, h, min, 0).unwrap()
}

fn repo() -> SqliteJobRepository {
    SqliteJobRepository::open_in_memory().expect("open in-memory database")
}

fn job(id: &str, job_type: JobType, trigger: TriggerSpec, status: JobStatus) -> Job {
    let target = match trigger {
        TriggerSpec::Indefinite => None,
        _ => Some(utc(2026, 7, 30, 23, 30)),
    };
    Job {
        id: id.into(),
        job_type,
        trigger,
        status,
        target_instant_utc: target,
        created_at_utc: utc(2026, 7, 30, 10, 0),
        updated_at_utc: utc(2026, 7, 30, 10, 0),
        timezone: "Asia/Ho_Chi_Minh".into(),
        failure_message: None,
        origin: JobOrigin::default(),
    }
}

fn power_off_job(id: &str, status: JobStatus) -> Job {
    job(
        id,
        JobType::PowerOff,
        TriggerSpec::at_time(23, 30),
        status,
    )
}

/// The invariant task 6.5 exists to protect: exactly one job of a type holds the
/// active slot, never zero and never two.
fn assert_exactly_one_active(repo: &SqliteJobRepository, job_type: JobType, expected_id: &str) {
    let holders: Vec<Job> = repo
        .list_occupying_active_slot()
        .expect("list active slot")
        .into_iter()
        .filter(|j| j.job_type == job_type)
        .collect();

    assert_eq!(
        holders.len(),
        1,
        "expected exactly one {job_type:?} job in the active slot, found {}: {:?}",
        holders.len(),
        holders.iter().map(|j| (&j.id, j.status)).collect::<Vec<_>>()
    );
    assert_eq!(holders[0].id, expected_id);
}

// ---- 6.6 round-trip ----

#[test]
fn a_pending_job_round_trips_with_its_target_instant_intact() {
    let repo = repo();
    let original = power_off_job("job-1", JobStatus::Active);
    repo.insert(&original).expect("insert");

    let loaded = repo.find("job-1").expect("find").expect("job exists");

    // Field-by-field rather than `assert_eq!` on the whole struct: `Job`'s
    // equality is derived here, but a future id-only PartialEq (as the Dart side
    // has) would make a whole-struct comparison pass while every other column
    // was wrong.
    assert_eq!(loaded.id, original.id);
    assert_eq!(loaded.job_type, original.job_type);
    assert_eq!(loaded.trigger, original.trigger);
    assert_eq!(loaded.status, original.status);
    assert_eq!(loaded.target_instant_utc, original.target_instant_utc);
    assert_eq!(loaded.created_at_utc, original.created_at_utc);
    assert_eq!(loaded.updated_at_utc, original.updated_at_utc);
    assert_eq!(loaded.timezone, original.timezone);
    assert_eq!(loaded.failure_message, original.failure_message);
}

#[test]
fn every_trigger_kind_round_trips() {
    let repo = repo();
    let cases = [
        ("indefinite", JobType::KeepAwake, TriggerSpec::Indefinite),
        (
            "duration",
            JobType::PowerOff,
            TriggerSpec::Duration { minutes: 120 },
        ),
        (
            "absolute",
            JobType::PowerOff,
            TriggerSpec::at_time(0, 0),
        ),
        (
            "absolute-dated",
            JobType::PowerOff,
            TriggerSpec::on_date(NaiveDate::from_ymd_opt(2026, 8, 10).unwrap(), 22, 30),
        ),
    ];

    for (id, job_type, trigger) in cases {
        let original = job(id, job_type, trigger, JobStatus::Active);
        repo.insert(&original).expect("insert");
        let loaded = repo.find(id).expect("find").expect("job exists");
        assert_eq!(loaded.trigger, trigger, "trigger for {id}");
        assert_eq!(
            loaded.target_instant_utc, original.target_instant_utc,
            "target for {id}"
        );
    }
}

#[test]
fn every_status_round_trips_through_the_database() {
    let repo = repo();
    for status in [
        JobStatus::Active,
        JobStatus::Paused,
        JobStatus::Completed,
        JobStatus::Cancelled,
        JobStatus::Failed,
        JobStatus::Overdue,
        JobStatus::Degraded,
    ] {
        let id = format!("job-{}", status.as_str());
        repo.insert(&power_off_job(&id, status)).expect("insert");
        let loaded = repo.find(&id).expect("find").expect("job exists");
        assert_eq!(loaded.status, status);
    }
}

#[test]
fn the_target_instant_survives_a_reopen_to_the_millisecond() {
    // Guards the storage encoding itself. A target written as a local-time string
    // or truncated to seconds would still pass an in-process round-trip.
    let repo = repo();
    let precise = Utc.timestamp_millis_opt(1_785_000_123_456).single().unwrap();
    let mut original = power_off_job("job-1", JobStatus::Active);
    original.target_instant_utc = Some(precise);
    repo.insert(&original).expect("insert");

    let loaded = repo.find("job-1").expect("find").expect("job exists");
    assert_eq!(loaded.target_instant_utc, Some(precise));
    assert_eq!(
        loaded.target_instant_utc.unwrap().timestamp_millis(),
        1_785_000_123_456
    );
}

#[test]
fn an_indefinite_keep_awake_job_round_trips_with_no_target() {
    let repo = repo();
    let original = job(
        "job-1",
        JobType::KeepAwake,
        TriggerSpec::Indefinite,
        JobStatus::Active,
    );
    repo.insert(&original).expect("insert");

    let loaded = repo.find("job-1").expect("find").expect("job exists");
    assert_eq!(loaded.trigger, TriggerSpec::Indefinite);
    assert!(
        loaded.target_instant_utc.is_none(),
        "an indefinite job must not acquire a target instant in storage"
    );
}

// ---- 6.5 the replacement transaction ----

#[test]
fn replacement_never_leaves_zero_or_two_active_jobs_of_a_type() {
    let repo = repo();
    repo.insert(&power_off_job("first", JobStatus::Active))
        .expect("insert first");
    assert_exactly_one_active(&repo, JobType::PowerOff, "first");

    // Replace repeatedly: two competing power-off timers would mean two
    // shutdowns armed, and zero would mean the job the user just created is
    // silently absent.
    for id in ["second", "third", "fourth"] {
        repo.replace_active_job_of_type(JobType::PowerOff, &power_off_job(id, JobStatus::Active))
            .expect("replace");
        assert_exactly_one_active(&repo, JobType::PowerOff, id);
    }

    // The displaced jobs are retained as cancelled, not deleted.
    let all = repo.list_all().expect("list all");
    assert_eq!(all.len(), 4);
    for id in ["first", "second", "third"] {
        let displaced = all.iter().find(|j| j.id == id).expect("displaced job kept");
        assert_eq!(displaced.status, JobStatus::Cancelled, "for {id}");
    }
}

#[test]
fn replacement_displaces_a_paused_job_too() {
    // A paused job still holds the slot (`occupies_active_slot`), so leaving it
    // behind would give the type two holders the moment the user resumed it.
    let repo = repo();
    repo.insert(&power_off_job("paused-one", JobStatus::Paused))
        .expect("insert");
    assert_exactly_one_active(&repo, JobType::PowerOff, "paused-one");

    repo.replace_active_job_of_type(
        JobType::PowerOff,
        &power_off_job("new-one", JobStatus::Active),
    )
    .expect("replace");

    assert_exactly_one_active(&repo, JobType::PowerOff, "new-one");
    assert_eq!(
        repo.find("paused-one").unwrap().unwrap().status,
        JobStatus::Cancelled
    );
}

#[test]
fn replacement_leaves_the_other_job_type_untouched() {
    // Keep-awake and power-off hold independent slots; the user can run both at
    // once, which is the whole point of the two-feature design.
    let repo = repo();
    let keep_awake = job(
        "keep-awake",
        JobType::KeepAwake,
        TriggerSpec::Indefinite,
        JobStatus::Active,
    );
    repo.insert(&keep_awake).expect("insert keep-awake");
    repo.insert(&power_off_job("power-off", JobStatus::Active))
        .expect("insert power-off");

    repo.replace_active_job_of_type(
        JobType::PowerOff,
        &power_off_job("power-off-2", JobStatus::Active),
    )
    .expect("replace");

    assert_exactly_one_active(&repo, JobType::KeepAwake, "keep-awake");
    assert_exactly_one_active(&repo, JobType::PowerOff, "power-off-2");
    assert_eq!(
        repo.find("keep-awake").unwrap().unwrap().status,
        JobStatus::Active
    );
}

#[test]
fn replacement_into_an_empty_slot_still_yields_exactly_one() {
    let repo = repo();
    repo.replace_active_job_of_type(JobType::PowerOff, &power_off_job("only", JobStatus::Active))
        .expect("replace");
    assert_exactly_one_active(&repo, JobType::PowerOff, "only");
}

#[test]
fn a_failed_insert_inside_the_transaction_rolls_back_the_cancellation() {
    // The atomicity claim, tested by making the insert half fail: reusing an
    // existing primary key. Without a transaction the first job would already be
    // cancelled, leaving the type with no active job at all.
    let repo = repo();
    repo.insert(&power_off_job("existing", JobStatus::Active))
        .expect("insert");

    let duplicate = power_off_job("existing", JobStatus::Active);
    let result = repo.replace_active_job_of_type(JobType::PowerOff, &duplicate);
    assert!(result.is_err(), "duplicate id must not insert");

    // The original is still active — the cancellation was rolled back.
    assert_exactly_one_active(&repo, JobType::PowerOff, "existing");
    assert_eq!(
        repo.find("existing").unwrap().unwrap().status,
        JobStatus::Active,
        "the cancellation must not survive a failed insert"
    );
}

// ---- 6.3 CRUD ----

#[test]
fn list_occupying_active_slot_excludes_terminal_states() {
    let repo = repo();
    repo.insert(&power_off_job("active", JobStatus::Active))
        .unwrap();
    for status in [
        JobStatus::Completed,
        JobStatus::Cancelled,
        JobStatus::Failed,
        JobStatus::Overdue,
        JobStatus::Degraded,
    ] {
        let id = format!("job-{}", status.as_str());
        repo.insert(&job(
            &id,
            JobType::KeepAwake,
            TriggerSpec::Indefinite,
            status,
        ))
        .unwrap();
    }

    let holders = repo.list_occupying_active_slot().unwrap();
    assert_eq!(holders.len(), 1);
    assert_eq!(holders[0].id, "active");
}

#[test]
fn update_status_records_and_clears_the_failure_message() {
    let repo = repo();
    repo.insert(&power_off_job("job-1", JobStatus::Active))
        .unwrap();

    repo.update_status("job-1", JobStatus::Failed, Some("privileges denied"))
        .expect("set failed");
    let failed = repo.find("job-1").unwrap().unwrap();
    assert_eq!(failed.status, JobStatus::Failed);
    assert_eq!(failed.failure_message.as_deref(), Some("privileges denied"));

    // Moving out of Failed must not leave a stale reason attached.
    repo.update_status("job-1", JobStatus::Cancelled, None)
        .expect("clear");
    let cleared = repo.find("job-1").unwrap().unwrap();
    assert_eq!(cleared.status, JobStatus::Cancelled);
    assert!(cleared.failure_message.is_none());
}

#[test]
fn update_target_replaces_the_target_instant() {
    let repo = repo();
    repo.insert(&power_off_job("job-1", JobStatus::Active))
        .unwrap();

    let retargeted = utc(2026, 8, 1, 6, 15);
    repo.update_target("job-1", retargeted).expect("retarget");

    assert_eq!(
        repo.find("job-1").unwrap().unwrap().target_instant_utc,
        Some(retargeted)
    );
}

#[test]
fn mutating_a_missing_job_is_an_error_rather_than_a_silent_no_op() {
    // SQLite reports zero affected rows instead of failing. Swallowing that would
    // let the scheduler believe it recorded a state change that never happened —
    // including a power-off that failed.
    let repo = repo();
    for result in [
        repo.update_status("ghost", JobStatus::Failed, Some("boom")),
        repo.update_target("ghost", utc(2026, 8, 1, 6, 15)),
        repo.delete("ghost"),
    ] {
        match result {
            Err(AppError::Storage { .. }) => {}
            other => panic!("expected a storage error, got {other:?}"),
        }
    }
}

#[test]
fn delete_removes_the_job() {
    let repo = repo();
    repo.insert(&power_off_job("job-1", JobStatus::Active))
        .unwrap();
    repo.delete("job-1").expect("delete");

    assert!(repo.find("job-1").unwrap().is_none());
    assert!(repo.list_all().unwrap().is_empty());
}

#[test]
fn find_returns_none_for_an_unknown_id() {
    assert!(repo().find("nope").expect("query succeeds").is_none());
}

#[test]
fn list_all_orders_by_most_recently_updated_first() {
    let repo = repo();
    for (id, updated) in [
        ("oldest", utc(2026, 7, 30, 8, 0)),
        ("newest", utc(2026, 7, 30, 12, 0)),
        ("middle", utc(2026, 7, 30, 10, 0)),
    ] {
        let mut j = power_off_job(id, JobStatus::Active);
        j.updated_at_utc = updated;
        repo.insert(&j).unwrap();
    }

    let ids: Vec<String> = repo.list_all().unwrap().into_iter().map(|j| j.id).collect();
    assert_eq!(ids, vec!["newest", "middle", "oldest"]);
}

#[test]
fn reopening_the_same_file_keeps_the_jobs() {
    // Persistence across a restart is the feature; an in-memory-only check would
    // never exercise the schema actually written to disk.
    let dir = std::env::temp_dir().join(format!("weakup-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let path = dir.join("jobs.sqlite3");
    let _ = std::fs::remove_file(&path);

    {
        let repo = SqliteJobRepository::open(&path).expect("open");
        repo.insert(&power_off_job("job-1", JobStatus::Active))
            .expect("insert");
    }

    let reopened = SqliteJobRepository::open(&path).expect("reopen");
    let loaded = reopened.find("job-1").expect("find").expect("job survived");
    assert_eq!(loaded.target_instant_utc, Some(utc(2026, 7, 30, 23, 30)));

    // Migration is idempotent, so a second open must not duplicate or wipe rows.
    assert_eq!(reopened.list_all().unwrap().len(), 1);

    std::fs::remove_file(&path).ok();
    std::fs::remove_dir(&dir).ok();
}

#[test]
fn a_row_whose_trigger_and_target_disagree_is_rejected_on_read() {
    // Defends the one invariant the type system cannot: an indefinite trigger has
    // no target, and every other kind must have one. A row violating it would
    // otherwise load as a job that silently never fires.
    let repo = repo();
    let indefinite = job(
        "job-1",
        JobType::KeepAwake,
        TriggerSpec::Indefinite,
        JobStatus::Active,
    );
    repo.insert(&indefinite).unwrap();

    // Corrupt it the way a bad migration or a hand-edited file would.
    repo.update_target("job-1", utc(2026, 7, 30, 23, 30))
        .expect("write a target onto an indefinite job");

    match repo.find("job-1") {
        Err(AppError::Storage { message }) => {
            assert!(
                message.contains("inconsistent"),
                "unexpected message: {message}"
            );
        }
        other => panic!("expected a storage error, got {other:?}"),
    }
}

mod settings_tests {
    use crate::data::settings::{Language, Settings, SettingsStore, Theme};
    use crate::data::SqliteJobRepository;

    #[test]
    fn a_fresh_database_returns_the_defaults() {
        let repo = SqliteJobRepository::open_in_memory().unwrap();

        assert_eq!(repo.load().unwrap(), Settings::default());
    }

    #[test]
    fn settings_survive_a_round_trip() {
        let repo = SqliteJobRepository::open_in_memory().unwrap();
        let settings = Settings {
            timezone: "Asia/Ho_Chi_Minh".to_string(),
            notifications_enabled: false,
            theme: Theme::Dark,
            language: Language::Vi,
        };

        repo.save(&settings).unwrap();

        assert_eq!(repo.load().unwrap(), settings);
    }

    #[test]
    fn saving_twice_updates_rather_than_failing_on_the_primary_key() {
        let repo = SqliteJobRepository::open_in_memory().unwrap();

        repo.save(&Settings {
            timezone: "UTC".to_string(),
            notifications_enabled: true,
            theme: Theme::Light,
            language: Language::En,
        })
        .unwrap();
        repo.save(&Settings {
            timezone: "Europe/London".to_string(),
            notifications_enabled: false,
            theme: Theme::Dark,
            language: Language::Vi,
        })
        .unwrap();

        let loaded = repo.load().unwrap();
        assert_eq!(loaded.timezone, "Europe/London");
        assert!(!loaded.notifications_enabled);
        // The presentational settings must overwrite too. Were they append-only, the theme
        // the user just left would win on the next launch.
        assert_eq!(loaded.theme, Theme::Dark);
        assert_eq!(loaded.language, Language::Vi);
    }

    #[test]
    fn settings_persist_across_reopening_the_same_file() {
        // The point of storing them at all. An in-memory database cannot show this.
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("weakup.db");
        let settings = Settings {
            timezone: "Asia/Ho_Chi_Minh".to_string(),
            notifications_enabled: false,
            theme: Theme::Dark,
            language: Language::Vi,
        };

        SqliteJobRepository::open(&path).unwrap().save(&settings).unwrap();

        let reopened = SqliteJobRepository::open(&path).unwrap();
        assert_eq!(reopened.load().unwrap(), settings);
    }

    #[test]
    fn an_unknown_key_from_a_newer_version_does_not_break_loading() {
        // Downgrading must not wipe the settings a newer build wrote.
        let repo = SqliteJobRepository::open_in_memory().unwrap();
        repo.save(&Settings::default()).unwrap();

        repo.execute_raw_for_test(
            "INSERT INTO settings (key, value) VALUES ('from_the_future', 'yes')",
        )
        .unwrap();

        let loaded = repo.load().unwrap();
        assert_eq!(loaded.timezone, Settings::default().timezone);
    }
}

mod migration {
    use super::*;

    /// Writes a database file in exactly the shape schema version 2 produced, with one
    /// absolute-time job in it, and returns the path.
    ///
    /// Written as raw SQL rather than by an older build because that is the only way to
    /// pin the *old* shape — using the current code to create the fixture would make the
    /// test tautological the moment the schema changed again.
    fn v2_database_with_one_job(dir: &std::path::Path) -> std::path::PathBuf {
        let path = dir.join("v2.sqlite");
        let conn = rusqlite::Connection::open(&path).expect("create v2 file");
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
                 failure_message    TEXT
             );
             CREATE TABLE settings (
                 key   TEXT PRIMARY KEY NOT NULL,
                 value TEXT NOT NULL
             );
             PRAGMA user_version = 2;",
        )
        .expect("create v2 schema");

        conn.execute(
            "INSERT INTO jobs (
                 id, job_type, trigger_kind, trigger_minutes, trigger_hour,
                 trigger_minute, status, target_instant_utc, created_at_utc,
                 updated_at_utc, timezone, failure_message
             ) VALUES ('old-job', 'powerOff', 'absoluteTime', NULL, 23, 30, 'active',
                       ?1, ?2, ?3, 'Asia/Ho_Chi_Minh', NULL)",
            rusqlite::params![
                utc(2026, 7, 30, 16, 30).timestamp_millis(),
                utc(2026, 7, 30, 10, 0).timestamp_millis(),
                utc(2026, 7, 30, 10, 0).timestamp_millis(),
            ],
        )
        .expect("insert a v2 job");

        path
    }

    #[test]
    fn a_v2_job_reads_back_as_undated_after_migrating() {
        // The compatibility claim in one test: a job written before the column existed
        // means "a time of day", and must keep meaning that. If the migration
        // backfilled a date, this job would stop rolling forward and would instead be
        // refused the next time it was resumed.
        let dir = tempfile::tempdir().expect("temp dir");
        let path = v2_database_with_one_job(dir.path());

        let repo = SqliteJobRepository::open(&path).expect("migrate and open");
        let job = repo.find("old-job").expect("find").expect("job survived");

        assert_eq!(job.trigger, TriggerSpec::at_time(23, 30));
        assert_eq!(job.target_instant_utc, Some(utc(2026, 7, 30, 16, 30)));
    }

    #[test]
    fn a_migrated_database_can_store_a_dated_job() {
        // Migrating must leave the file fully usable, not merely readable.
        let dir = tempfile::tempdir().expect("temp dir");
        let path = v2_database_with_one_job(dir.path());
        let repo = SqliteJobRepository::open(&path).expect("migrate and open");

        let dated = TriggerSpec::on_date(NaiveDate::from_ymd_opt(2026, 8, 10).unwrap(), 22, 30);
        repo.insert(&job("new-job", JobType::PowerOff, dated, JobStatus::Paused))
            .expect("insert into the migrated file");

        assert_eq!(
            repo.find("new-job").unwrap().unwrap().trigger,
            dated,
            "the appended column must round-trip in a migrated file too"
        );
    }

    #[test]
    fn migrating_is_idempotent() {
        // Every launch runs `migrate`. A second run must not fail on the already-added
        // column, which is what a bare ALTER TABLE would do.
        let dir = tempfile::tempdir().expect("temp dir");
        let path = v2_database_with_one_job(dir.path());

        SqliteJobRepository::open(&path).expect("first open migrates");
        drop(SqliteJobRepository::open(&path).expect("second open must not re-migrate"));

        let repo = SqliteJobRepository::open(&path).expect("third open");
        assert!(repo.find("old-job").unwrap().is_some());
    }

    /// Writes a database file in exactly the shape schema version 3 produced — jobs with
    /// `trigger_date` but no `origin`, and no `command_decisions` table — with one job in it.
    ///
    /// Raw SQL for the same reason the v2 fixture uses it: creating the fixture with the
    /// current code would make the test tautological the moment the schema changed again.
    fn v3_database_with_one_job(dir: &std::path::Path) -> std::path::PathBuf {
        let path = dir.join("v3.sqlite");
        let conn = rusqlite::Connection::open(&path).expect("create v3 file");
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
                 trigger_date       TEXT
             );
             CREATE TABLE settings (
                 key   TEXT PRIMARY KEY NOT NULL,
                 value TEXT NOT NULL
             );
             PRAGMA user_version = 3;",
        )
        .expect("create v3 schema");

        conn.execute(
            "INSERT INTO jobs (
                 id, job_type, trigger_kind, trigger_minutes, trigger_hour,
                 trigger_minute, status, target_instant_utc, created_at_utc,
                 updated_at_utc, timezone, failure_message, trigger_date
             ) VALUES ('v3-job', 'powerOff', 'absoluteTime', NULL, 23, 30, 'active',
                       ?1, ?2, ?3, 'Asia/Ho_Chi_Minh', NULL, NULL)",
            rusqlite::params![
                utc(2026, 7, 30, 16, 30).timestamp_millis(),
                utc(2026, 7, 30, 10, 0).timestamp_millis(),
                utc(2026, 7, 30, 10, 0).timestamp_millis(),
            ],
        )
        .expect("insert a v3 job");

        path
    }

    #[test]
    fn a_v3_job_reads_back_as_locally_scheduled_after_migrating() {
        // Every row written before the origin column existed was scheduled at the machine,
        // so reading a null origin as `Local` is a faithful reading of old data rather than
        // a default standing in for information that was lost.
        let dir = tempfile::tempdir().expect("temp dir");
        let path = v3_database_with_one_job(dir.path());

        let repo = SqliteJobRepository::open(&path).expect("migrate and open");
        let job = repo.find("v3-job").expect("find").expect("job survived");

        assert_eq!(job.origin, JobOrigin::Local);
        // And the previous migration's column is still intact — the two coexist.
        assert_eq!(job.trigger, TriggerSpec::at_time(23, 30));
    }

    #[test]
    fn a_migrated_v3_database_stores_and_recovers_a_remote_origin() {
        // Migrating must leave the file fully usable, not merely readable: the whole point
        // of the column is that a remote job written after the migration reads back remote.
        let dir = tempfile::tempdir().expect("temp dir");
        let path = v3_database_with_one_job(dir.path());
        let repo = SqliteJobRepository::open(&path).expect("migrate and open");

        let remote = Job {
            origin: JobOrigin::Remote,
            ..power_off_job("remote-job", JobStatus::Paused)
        };
        repo.insert(&remote).expect("insert into the migrated file");

        assert_eq!(
            repo.find("remote-job").unwrap().unwrap().origin,
            JobOrigin::Remote,
            "the appended column must round-trip in a migrated file too"
        );
    }

    #[test]
    fn migrating_from_v3_creates_the_command_decision_table() {
        use crate::data::job_repository::CommandDecisionLog;

        // The table arrives with the same migration, so an upgraded install can record a
        // decision immediately rather than on some later launch.
        let dir = tempfile::tempdir().expect("temp dir");
        let path = v3_database_with_one_job(dir.path());
        let repo = SqliteJobRepository::open(&path).expect("migrate and open");

        repo.append_command_decision(&crate::data::CommandDecisionRecord {
            sender: crate::domain::DeviceId::new("phone-a").unwrap(),
            command: crate::domain::RemoteCommand::PowerOff,
            acceptance: crate::domain::CommandAcceptance::Accepted,
            decided_at_utc: utc(2026, 8, 6, 12, 0),
        })
        .expect("the table must exist after migrating");

        assert_eq!(repo.recent_command_decisions(10).unwrap().len(), 1);
    }

    #[test]
    fn migrating_from_v3_is_idempotent() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = v3_database_with_one_job(dir.path());

        SqliteJobRepository::open(&path).expect("first open migrates");
        drop(SqliteJobRepository::open(&path).expect("second open must not re-migrate"));

        let repo = SqliteJobRepository::open(&path).expect("third open");
        assert_eq!(
            repo.find("v3-job").unwrap().unwrap().origin,
            JobOrigin::Local
        );
    }
}

mod origin {
    use super::*;
    use crate::application::{grace_period_seconds, REMOTE_GRACE_PERIOD_SECONDS};

    fn job_with_origin(id: &str, origin: JobOrigin) -> Job {
        Job {
            origin,
            ..power_off_job(id, JobStatus::Active)
        }
    }

    #[test]
    fn a_remote_job_round_trips_as_remote() {
        // Before the origin column existed this came back as `Local`, which is the bug this
        // closes: the read hardcoded a default rather than reading what was written.
        let repo = repo();
        repo.insert(&job_with_origin("remote-job", JobOrigin::Remote))
            .expect("insert a remote job");

        let found = repo.find("remote-job").unwrap().expect("job exists");
        assert_eq!(found.origin, JobOrigin::Remote);
    }

    #[test]
    fn a_local_job_round_trips_as_local() {
        let repo = repo();
        repo.insert(&job_with_origin("local-job", JobOrigin::Local))
            .expect("insert a local job");

        let found = repo.find("local-job").unwrap().expect("job exists");
        assert_eq!(found.origin, JobOrigin::Local);
    }

    #[test]
    fn the_origin_survives_every_read_path() {
        // `find` and the two list queries select the same JOB_COLUMNS, but a future edit
        // could give one of them its own column list and only this test would notice.
        let repo = repo();
        repo.insert(&job_with_origin("remote-job", JobOrigin::Remote))
            .expect("insert");

        for job in repo.list_all().unwrap() {
            assert_eq!(job.origin, JobOrigin::Remote, "list_all lost the origin");
        }
        for job in repo.list_occupying_active_slot().unwrap() {
            assert_eq!(
                job.origin,
                JobOrigin::Remote,
                "list_occupying_active_slot lost the origin"
            );
        }
    }

    #[test]
    fn the_origin_is_preserved_across_a_status_update() {
        // `update_status` names the columns it writes and must not disturb this one. A
        // remote job paused and resumed must not quietly become local.
        let repo = repo();
        repo.insert(&job_with_origin("remote-job", JobOrigin::Remote))
            .expect("insert");

        repo.update_status("remote-job", JobStatus::Paused, None)
            .expect("pause");
        assert_eq!(
            repo.find("remote-job").unwrap().unwrap().origin,
            JobOrigin::Remote
        );

        repo.update_target("remote-job", utc(2026, 7, 31, 23, 30))
            .expect("retarget");
        assert_eq!(
            repo.find("remote-job").unwrap().unwrap().origin,
            JobOrigin::Remote
        );
    }

    #[test]
    fn a_remote_job_recovered_from_storage_is_given_the_remote_countdown() {
        // This is the actual bug the origin column closes, stated as the consequence rather
        // than as a storage detail. With the origin hardcoded on read, a remote power-off
        // that survived a restart was handed 60 seconds instead of 300 — a safety countdown
        // silently shortened for exactly the person who did not ask for the shutdown.
        let repo = repo();
        repo.insert(&job_with_origin("remote-job", JobOrigin::Remote))
            .expect("insert");

        let recovered = repo.find("remote-job").unwrap().expect("job exists");

        assert_eq!(
            grace_period_seconds(recovered.origin),
            REMOTE_GRACE_PERIOD_SECONDS,
            "a remote job recovered from storage must keep the longer countdown"
        );
    }

    #[test]
    fn an_unrecognised_stored_origin_is_a_storage_error() {
        // Matching the file's other "row holds an unknown ..." errors. Silently reading it
        // as local would shorten the countdown, which is the failure this column exists to
        // prevent.
        let repo = repo();
        repo.insert(&job_with_origin("odd-job", JobOrigin::Local))
            .expect("insert");
        repo.execute_raw_for_test("UPDATE jobs SET origin = 'sideways' WHERE id = 'odd-job'")
            .expect("write an unknown origin");

        let error = repo.find("odd-job").expect_err("must not be readable");
        assert!(
            matches!(&error, AppError::Storage { message } if message.contains("unknown job origin")),
            "got: {error:?}"
        );
    }
}

mod command_decisions {
    use super::*;
    use crate::data::job_repository::CommandDecisionLog;
    use crate::data::CommandDecisionRecord;
    use crate::domain::{CommandAcceptance, DeviceId, RejectionReason, RemoteCommand};

    fn device(id: &str) -> DeviceId {
        DeviceId::new(id).expect("non-empty device id")
    }

    fn record(
        sender: &str,
        command: RemoteCommand,
        acceptance: CommandAcceptance,
        minute: u32,
    ) -> CommandDecisionRecord {
        CommandDecisionRecord {
            sender: device(sender),
            command,
            acceptance,
            decided_at_utc: utc(2026, 8, 6, 12, minute),
        }
    }

    #[test]
    fn an_accepted_decision_is_recorded_with_its_sender_and_instant() {
        // Asserted on the stored row rather than on a return value: the point of the record
        // is that it can be read back afterwards to attribute a shutdown.
        let repo = repo();
        repo.append_command_decision(&record(
            "phone-a",
            RemoteCommand::PowerOff,
            CommandAcceptance::Accepted,
            0,
        ))
        .expect("append");

        let stored = repo.recent_command_decisions(10).expect("read back");
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].sender, device("phone-a"));
        assert_eq!(stored[0].command, RemoteCommand::PowerOff);
        assert_eq!(stored[0].acceptance, CommandAcceptance::Accepted);
        assert_eq!(stored[0].decided_at_utc, utc(2026, 8, 6, 12, 0));
        assert_eq!(stored[0].rejection_reason(), None);
    }

    #[test]
    fn a_refusal_is_recorded_with_its_single_reason() {
        // A refused command creates no job, so if the refusal were not written here it
        // would leave no trace anywhere that anything was attempted.
        //
        // A fresh database per reason, so each assertion sees exactly one row and a reason
        // that failed to write cannot be masked by another that did.
        for reason in [
            RejectionReason::AuthenticityUnverified,
            RejectionReason::ReplayedNonce,
            RejectionReason::FutureDated,
            RejectionReason::Stale,
            RejectionReason::NotPermitted,
        ] {
            let fresh = repo();
            fresh
                .append_command_decision(&record(
                    "phone-a",
                    RemoteCommand::PowerOff,
                    CommandAcceptance::Rejected(reason),
                    0,
                ))
                .expect("append a refusal");

            let stored = fresh.recent_command_decisions(10).expect("read back");
            assert_eq!(stored.len(), 1, "{reason:?} was not recorded");
            assert_eq!(stored[0].rejection_reason(), Some(reason));
            assert_eq!(stored[0].decision_str(), "rejected");
        }
    }

    #[test]
    fn several_refusals_from_one_device_are_all_retained() {
        // A series of refusals is the visible signature of an attack in progress, so the
        // record must keep every one rather than collapsing them into a latest-only row.
        let repo = repo();
        for minute in 0..5 {
            repo.append_command_decision(&record(
                "phone-a",
                RemoteCommand::PowerOff,
                CommandAcceptance::Rejected(RejectionReason::AuthenticityUnverified),
                minute,
            ))
            .expect("append");
        }

        let stored = repo.recent_command_decisions(10).expect("read back");
        assert_eq!(stored.len(), 5, "refusals must not overwrite one another");
        assert!(stored.iter().all(|r| r.sender == device("phone-a")));
        // Newest first, so the most recent attempt is what a reader sees.
        assert_eq!(stored[0].decided_at_utc, utc(2026, 8, 6, 12, 4));
        assert_eq!(stored[4].decided_at_utc, utc(2026, 8, 6, 12, 0));
    }

    #[test]
    fn reads_are_capped_by_the_requested_limit() {
        let repo = repo();
        for minute in 0..10 {
            repo.append_command_decision(&record(
                "phone-a",
                RemoteCommand::CancelJob,
                CommandAcceptance::Accepted,
                minute,
            ))
            .expect("append");
        }

        assert_eq!(repo.recent_command_decisions(3).unwrap().len(), 3);
        assert_eq!(repo.recent_command_decisions(100).unwrap().len(), 10);
    }

    #[test]
    fn decisions_from_different_devices_stay_attributable_to_each() {
        // Attribution is the whole purpose: "which device shut my machine down" must have
        // an answer, so two senders must not be confusable.
        let repo = repo();
        repo.append_command_decision(&record(
            "phone-a",
            RemoteCommand::PowerOff,
            CommandAcceptance::Accepted,
            0,
        ))
        .expect("append");
        repo.append_command_decision(&record(
            "laptop-b",
            RemoteCommand::PowerOff,
            CommandAcceptance::Rejected(RejectionReason::NotPermitted),
            1,
        ))
        .expect("append");

        let stored = repo.recent_command_decisions(10).expect("read back");
        assert_eq!(stored[0].sender, device("laptop-b"));
        assert_eq!(
            stored[0].rejection_reason(),
            Some(RejectionReason::NotPermitted)
        );
        assert_eq!(stored[1].sender, device("phone-a"));
        assert_eq!(stored[1].rejection_reason(), None);
    }

    #[test]
    fn a_row_that_cannot_explain_itself_is_a_storage_error() {
        // A rejection with no reason, or an acceptance carrying one, means the record can no
        // longer explain what happened — which is the only thing it exists to do. Caught on
        // read rather than returned as a half-decision.
        //
        // A separate database per case: the first bad row makes every later read fail, so
        // sharing one would let the second assertion pass on the first row's error.
        let reasonless = repo();
        reasonless
            .execute_raw_for_test(
                "INSERT INTO command_decisions
                 (sender_device_id, command, decision, rejection_reason, decided_at_utc)
             VALUES ('phone-a', 'powerOff', 'rejected', NULL, 0)",
            )
            .expect("write a reasonless refusal");
        assert!(matches!(
            reasonless.recent_command_decisions(10),
            Err(AppError::Storage { .. })
        ));

        let accepted_with_reason = repo();
        accepted_with_reason
            .execute_raw_for_test(
                "INSERT INTO command_decisions
                 (sender_device_id, command, decision, rejection_reason, decided_at_utc)
             VALUES ('phone-a', 'powerOff', 'accepted', 'stale', 0)",
            )
            .expect("write an accepted row with a reason");
        assert!(matches!(
            accepted_with_reason.recent_command_decisions(10),
            Err(AppError::Storage { .. })
        ));
    }

    #[test]
    fn the_record_is_readable_with_no_network_because_it_is_a_local_table() {
        // Nothing in this path opens a socket. Stated as a test so a future change that
        // routed reads through a relay would have to delete an assertion that says not to.
        let repo = repo();
        repo.append_command_decision(&record(
            "phone-a",
            RemoteCommand::PowerOff,
            CommandAcceptance::Accepted,
            0,
        ))
        .expect("append");

        assert_eq!(repo.recent_command_decisions(1).unwrap().len(), 1);
    }
}

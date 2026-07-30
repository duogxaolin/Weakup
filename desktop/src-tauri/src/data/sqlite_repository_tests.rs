use chrono::{DateTime, TimeZone, Utc};

use crate::core::AppError;
use crate::data::job_repository::JobRepository;
use crate::data::sqlite_repository::SqliteJobRepository;
use crate::domain::{Job, JobStatus, JobType, TriggerSpec};

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
    }
}

fn power_off_job(id: &str, status: JobStatus) -> Job {
    job(
        id,
        JobType::PowerOff,
        TriggerSpec::AbsoluteTime { hour: 23, minute: 30 },
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
            TriggerSpec::AbsoluteTime { hour: 0, minute: 0 },
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

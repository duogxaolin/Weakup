//! Tests for the keep-awake coordinator.
//!
//! The property under test throughout: when no job wants the screen awake, no OS
//! assertion is held. A leak here means the user's display never sleeps again and
//! nothing in the UI explains why.

use std::sync::Arc;

use crate::core::AppError;
use crate::platform::keep_awake::controller::KeepAwakeController;
use crate::platform::keep_awake::coordinator::{AssertionChange, KeepAwakeCoordinator};
use crate::platform::keep_awake::fake::{FakeKeepAwakeController, KeepAwakeEvent};

fn coordinator() -> (KeepAwakeCoordinator, Arc<FakeKeepAwakeController>) {
    let fake = Arc::new(FakeKeepAwakeController::working());
    (KeepAwakeCoordinator::new(fake.clone()), fake)
}

#[test]
fn the_first_job_acquires_and_the_last_one_releases() {
    let (coord, fake) = coordinator();

    assert_eq!(coord.add("job-a").unwrap(), AssertionChange::Acquired);
    assert!(fake.is_held());

    assert_eq!(coord.remove("job-a").unwrap(), AssertionChange::Released);
    fake.assert_not_held();
    assert_eq!(fake.events(), vec![KeepAwakeEvent::Acquired, KeepAwakeEvent::Released]);
}

#[test]
fn a_second_job_does_not_take_a_second_assertion() {
    let (coord, fake) = coordinator();

    coord.add("job-a").unwrap();
    assert_eq!(coord.add("job-b").unwrap(), AssertionChange::Unchanged);

    assert_eq!(fake.acquire_count(), 1);
    assert_eq!(coord.holder_count(), 2);
}

#[test]
fn the_assertion_survives_until_the_last_job_goes() {
    let (coord, fake) = coordinator();

    coord.add("job-a").unwrap();
    coord.add("job-b").unwrap();

    // Releasing on the first removal would drop the screen assertion while job-b is
    // still running — the user's screen sleeps mid-job.
    assert_eq!(coord.remove("job-a").unwrap(), AssertionChange::Unchanged);
    assert!(fake.is_held(), "job-b still wants the screen awake");
    assert_eq!(fake.release_count(), 0);

    assert_eq!(coord.remove("job-b").unwrap(), AssertionChange::Released);
    fake.assert_not_held();
}

#[test]
fn adding_the_same_job_twice_is_one_hold_and_one_removal_ends_it() {
    // The scheduler re-reconciles on startup and after every edit, so the same id
    // arrives repeatedly. If a repeat counted as a second holder, one cancel would
    // leave the set non-empty forever and the assertion would never be released.
    let (coord, fake) = coordinator();

    coord.add("job-a").unwrap();
    assert_eq!(coord.add("job-a").unwrap(), AssertionChange::Unchanged);
    assert_eq!(coord.add("job-a").unwrap(), AssertionChange::Unchanged);
    assert_eq!(coord.holder_count(), 1);
    assert_eq!(fake.acquire_count(), 1);

    assert_eq!(coord.remove("job-a").unwrap(), AssertionChange::Released);
    fake.assert_not_held();
}

#[test]
fn removing_a_job_that_never_held_changes_nothing() {
    let (coord, fake) = coordinator();

    assert_eq!(coord.remove("ghost").unwrap(), AssertionChange::Unchanged);
    assert!(fake.events().is_empty());

    coord.add("job-a").unwrap();
    assert_eq!(coord.remove("ghost").unwrap(), AssertionChange::Unchanged);
    assert!(fake.is_held(), "an unrelated removal must not drop job-a's hold");
    assert_eq!(fake.release_count(), 0);
}

#[test]
fn cancel_complete_and_pause_all_end_the_assertion() {
    // They are the same operation here by design: three separate call sites would
    // be three chances to forget the release. This asserts each of the three job
    // lifecycle endings leaves nothing held.
    for ending in ["cancelled", "completed", "paused"] {
        let (coord, fake) = coordinator();
        coord.add("job-a").unwrap();
        assert!(fake.is_held());

        coord.remove("job-a").unwrap();
        assert!(
            !fake.is_held(),
            "an assertion outlived a {ending} job — the display would never sleep"
        );
    }
}

#[test]
fn a_refused_acquire_leaves_no_holder_registered() {
    // Otherwise the set claims an assertion is held that is not, and because only
    // the empty-to-non-empty transition acquires, no later add would ever retry.
    let fake = Arc::new(FakeKeepAwakeController::failing_to_acquire(
        AppError::KeepAwakeFailed {
            message: "the OS refused".to_string(),
        },
    ));
    let coord = KeepAwakeCoordinator::new(fake.clone());

    assert!(coord.add("job-a").is_err());
    assert_eq!(coord.holder_count(), 0);
    assert!(!fake.is_held());

    // A later attempt genuinely tries again.
    fake.set_acquire_error(None);
    assert_eq!(coord.add("job-a").unwrap(), AssertionChange::Acquired);
    assert!(fake.is_held());
}

#[test]
fn a_failed_release_still_clears_the_holders() {
    // Reporting the error is right; keeping the job registered is not. Re-adding it
    // would not un-stick an OS assertion, and it would stop the app from ever
    // attempting the release again.
    let fake = Arc::new(FakeKeepAwakeController::failing_to_release(
        AppError::KeepAwakeFailed {
            message: "release failed".to_string(),
        },
    ));
    let coord = KeepAwakeCoordinator::new(fake.clone());

    coord.add("job-a").unwrap();
    assert!(coord.remove("job-a").is_err());
    assert_eq!(coord.holder_count(), 0);
    assert_eq!(fake.events().last(), Some(&KeepAwakeEvent::ReleaseFailed));
}

#[test]
fn reconcile_drops_a_job_that_vanished_while_the_app_was_closed() {
    let (coord, fake) = coordinator();

    coord.add("job-a").unwrap();
    coord.add("job-b").unwrap();

    // The database now says only job-b is active — job-a was deleted elsewhere.
    assert_eq!(coord.reconcile(["job-b"]).unwrap(), AssertionChange::Unchanged);
    assert_eq!(coord.holders(), vec!["job-b".to_string()]);
    assert!(fake.is_held());

    // And now nothing is active.
    assert_eq!(
        coord.reconcile(Vec::<String>::new()).unwrap(),
        AssertionChange::Released
    );
    fake.assert_not_held();
}

#[test]
fn reconcile_from_empty_to_active_acquires_once() {
    let (coord, fake) = coordinator();

    assert_eq!(
        coord.reconcile(["job-a", "job-b", "job-c"]).unwrap(),
        AssertionChange::Acquired
    );
    assert_eq!(fake.acquire_count(), 1);
    assert_eq!(coord.holder_count(), 3);
}

#[test]
fn reconcile_to_the_same_set_does_not_touch_the_os() {
    let (coord, fake) = coordinator();
    coord.reconcile(["job-a"]).unwrap();
    let before = fake.events().len();

    coord.reconcile(["job-a"]).unwrap();
    coord.reconcile(["job-a"]).unwrap();

    assert_eq!(fake.events().len(), before, "no redundant OS calls");
}

#[test]
fn reconcile_with_an_empty_set_on_a_fresh_coordinator_does_nothing() {
    // Startup with no active jobs must not call release on an assertion that was
    // never taken.
    let (coord, fake) = coordinator();

    assert_eq!(
        coord.reconcile(Vec::<String>::new()).unwrap(),
        AssertionChange::Unchanged
    );
    assert!(fake.events().is_empty());
}

#[test]
fn an_unavailable_mechanism_is_reported_as_unavailable_not_as_a_failure() {
    // Task 8.6. The user needs to know keep-awake cannot work here at all, which is
    // a different sentence from "it failed this time".
    let fake = Arc::new(FakeKeepAwakeController::unavailable());
    let coord = KeepAwakeCoordinator::new(fake.clone());

    assert!(!coord.is_available());
    assert!(coord.unavailable_reason().is_some());

    match coord.add("job-a") {
        Err(AppError::KeepAwakeUnavailable { .. }) => {}
        other => panic!("expected unavailable, got {other:?}"),
    }
    assert_eq!(coord.holder_count(), 0);
}

#[test]
fn many_threads_adding_and_removing_leave_nothing_held() {
    // The scheduler drives this from tokio tasks. Interleaved add/remove must not
    // end with a held assertion and an empty holder set.
    let fake = Arc::new(FakeKeepAwakeController::working());
    let coord = Arc::new(KeepAwakeCoordinator::new(fake.clone()));

    let mut handles = Vec::new();
    for i in 0..8 {
        let coord = coord.clone();
        handles.push(std::thread::spawn(move || {
            let id = format!("job-{i}");
            for _ in 0..50 {
                coord.add(&id).unwrap();
                coord.remove(&id).unwrap();
            }
        }));
    }
    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(coord.holder_count(), 0);
    fake.assert_not_held();
    assert_eq!(
        fake.acquire_count(),
        fake.release_count(),
        "every acquire must be matched by a release"
    );
}

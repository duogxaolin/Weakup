use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use chrono_tz::Tz;

use crate::core::AppError;
use crate::domain::job_enums::JobType;
use crate::domain::trigger_resolver::{
    ReconcileOutcome, TriggerResolver, POWER_OFF_OVERTOLERANCE_MINUTES,
};
use crate::domain::trigger_spec::TriggerSpec;

fn utc(y: i32, m: u32, d: u32, h: u32, min: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(y, m, d, h, min, 0).unwrap()
}

fn date(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

fn ny() -> Tz {
    "America/New_York".parse().unwrap()
}

fn saigon() -> Tz {
    "Asia/Ho_Chi_Minh".parse().unwrap()
}

// ---------------------------------------------------------------- validation

#[test]
fn power_off_rejects_indefinite() {
    let err = TriggerResolver::validate(&TriggerSpec::Indefinite, JobType::PowerOff)
        .expect_err("power-off must reject indefinite");
    assert!(matches!(err, AppError::Validation { .. }));
    assert!(err.user_message().contains("duration or a specific time"));
}

#[test]
fn keep_awake_accepts_indefinite() {
    assert!(TriggerResolver::validate(&TriggerSpec::Indefinite, JobType::KeepAwake).is_ok());
}

#[test]
fn zero_duration_rejected() {
    let err = TriggerResolver::validate(&TriggerSpec::Duration { minutes: 0 }, JobType::PowerOff)
        .expect_err("zero must be rejected");
    assert!(err.user_message().contains("positive"));
}

#[test]
fn negative_duration_rejected() {
    assert!(
        TriggerResolver::validate(&TriggerSpec::Duration { minutes: -5 }, JobType::PowerOff)
            .is_err()
    );
}

#[test]
fn duration_at_the_maximum_is_accepted_and_one_past_it_is_not() {
    assert!(
        TriggerResolver::validate(&TriggerSpec::Duration { minutes: 1440 }, JobType::PowerOff)
            .is_ok(),
        "1440 minutes is exactly 24h and must be accepted"
    );
    let err =
        TriggerResolver::validate(&TriggerSpec::Duration { minutes: 1441 }, JobType::PowerOff)
            .expect_err("1441 must be rejected");
    assert!(err.user_message().contains("24 hours"));
}

#[test]
fn out_of_range_wall_clock_times_are_rejected() {
    assert!(TriggerResolver::validate(
        &TriggerSpec::at_time(24, 0),
        JobType::PowerOff
    )
    .is_err());
    assert!(TriggerResolver::validate(
        &TriggerSpec::at_time(12, 60),
        JobType::PowerOff
    )
    .is_err());
    assert!(TriggerResolver::validate(
        &TriggerSpec::at_time(23, 59),
        JobType::PowerOff
    )
    .is_ok());
}

// ---------------------------------------------------------------- resolution

#[test]
fn indefinite_resolves_to_no_target_instant() {
    let resolved =
        TriggerResolver::resolve(&TriggerSpec::Indefinite, saigon(), utc(2026, 7, 30, 7, 0))
            .expect("resolve");
    assert_eq!(resolved, None);
}

#[test]
fn duration_resolves_to_now_plus_duration() {
    let now = utc(2026, 7, 30, 7, 0);
    let resolved =
        TriggerResolver::resolve(&TriggerSpec::Duration { minutes: 120 }, saigon(), now)
            .expect("resolve")
            .expect("has target");
    assert_eq!(resolved, utc(2026, 7, 30, 9, 0));
}

#[test]
fn duration_result_is_independent_of_timezone() {
    // A duration is anchored at creation, so the same now must give the same target
    // in any zone. This is the property that makes duration jobs immune to timezone
    // changes.
    let now = utc(2026, 7, 30, 7, 0);
    let in_saigon = TriggerResolver::resolve(&TriggerSpec::Duration { minutes: 90 }, saigon(), now)
        .expect("resolve");
    let in_ny =
        TriggerResolver::resolve(&TriggerSpec::Duration { minutes: 90 }, ny(), now).expect("resolve");
    assert_eq!(in_saigon, in_ny);
}

#[test]
fn future_wall_time_today_resolves_to_today() {
    // 14:00 local Saigon = 07:00 UTC. Target 20:00 local = 13:00 UTC same day.
    let now = utc(2026, 7, 30, 7, 0);
    let resolved = TriggerResolver::resolve(
        &TriggerSpec::at_time(20, 0),
        saigon(),
        now,
    )
    .expect("resolve")
    .expect("has target");
    assert_eq!(resolved, utc(2026, 7, 30, 13, 0));
}

#[test]
fn past_wall_time_today_rolls_to_tomorrow() {
    // 21:00 local Saigon = 14:00 UTC. Target 20:00 local already passed, so it must
    // roll to 20:00 local tomorrow = 13:00 UTC on the 31st.
    let now = utc(2026, 7, 30, 14, 0);
    let resolved = TriggerResolver::resolve(
        &TriggerSpec::at_time(20, 0),
        saigon(),
        now,
    )
    .expect("resolve")
    .expect("has target");
    assert_eq!(resolved, utc(2026, 7, 31, 13, 0));
}

#[test]
fn wall_time_exactly_equal_to_now_rolls_to_tomorrow() {
    // The target must be strictly in the future; a job created at exactly 20:00 for
    // 20:00 should not fire instantly.
    let now = utc(2026, 7, 30, 13, 0); // 20:00 Saigon
    let resolved = TriggerResolver::resolve(
        &TriggerSpec::at_time(20, 0),
        saigon(),
        now,
    )
    .expect("resolve")
    .expect("has target");
    assert_eq!(resolved, utc(2026, 7, 31, 13, 0));
}

#[test]
fn spring_forward_gap_resolves_to_the_jump_instant() {
    // America/New_York 2026-03-08: 02:00 -> 03:00 local, so 02:30 never exists.
    // Verified empirically: 03:00 EDT == 07:00 UTC.
    let now = utc(2026, 3, 8, 6, 0); // 01:00 EST, before the transition
    let resolved = TriggerResolver::resolve(
        &TriggerSpec::at_time(2, 30),
        ny(),
        now,
    )
    .expect("resolve")
    .expect("has target");
    assert_eq!(
        resolved,
        utc(2026, 3, 8, 7, 0),
        "a nonexistent wall time must resolve to the instant the clock jumps to"
    );
}

#[test]
fn fall_back_overlap_resolves_to_the_earlier_occurrence() {
    // America/New_York 2026-11-01: 02:00 -> 01:00 local, so 01:30 occurs twice.
    // Verified empirically: earliest == 05:30 UTC, latest == 06:30 UTC.
    let now = utc(2026, 11, 1, 4, 0); // 00:00 EDT, before the transition
    let resolved = TriggerResolver::resolve(
        &TriggerSpec::at_time(1, 30),
        ny(),
        now,
    )
    .expect("resolve")
    .expect("has target");
    assert_eq!(
        resolved,
        utc(2026, 11, 1, 5, 30),
        "an ambiguous wall time must resolve to the first occurrence, not the second"
    );
}

// -------------------------------------------------- resolution, dated absolute

#[test]
fn a_dated_wall_time_resolves_to_that_exact_date() {
    // Eleven days out. The undated form would have resolved to today or tomorrow.
    let now = utc(2026, 7, 30, 7, 0);
    let resolved = TriggerResolver::resolve(
        &TriggerSpec::on_date(date(2026, 8, 10), 20, 0),
        saigon(),
        now,
    )
    .expect("resolve")
    .expect("has target");
    assert_eq!(resolved, utc(2026, 8, 10, 13, 0));
}

#[test]
fn a_dated_wall_time_in_the_past_is_rejected_rather_than_rolled_forward() {
    // The counterpart of `past_wall_time_today_rolls_to_tomorrow`: identical now,
    // zone, and time-of-day, differing only by carrying today's date. The undated
    // form rolls to tomorrow; this must refuse instead, because moving an
    // irreversible power-off to a day the user never chose is worse than refusing.
    let now = utc(2026, 7, 30, 14, 0); // 21:00 Saigon
    let error = TriggerResolver::resolve(
        &TriggerSpec::on_date(date(2026, 7, 30), 20, 0),
        saigon(),
        now,
    )
    .expect_err("a passed dated instant must not resolve");

    assert!(
        error.user_message().contains("already passed"),
        "the message must say why: {}",
        error.user_message()
    );
}

#[test]
fn a_dated_wall_time_equal_to_now_is_rejected() {
    // Strictly in the future, as for the undated form. A target equal to now would
    // fire the instant it was created.
    let now = utc(2026, 7, 30, 13, 0); // exactly 20:00 Saigon
    assert!(TriggerResolver::resolve(
        &TriggerSpec::on_date(date(2026, 7, 30), 20, 0),
        saigon(),
        now,
    )
    .is_err());
}

#[test]
fn a_dated_wall_time_one_minute_after_now_is_accepted() {
    // Pins the boundary from the other side, so the comparison cannot drift from
    // `<=` to `<` without a failure here.
    let now = utc(2026, 7, 30, 12, 59);
    let resolved = TriggerResolver::resolve(
        &TriggerSpec::on_date(date(2026, 7, 30), 20, 0),
        saigon(),
        now,
    )
    .expect("resolve")
    .expect("has target");
    assert_eq!(resolved, utc(2026, 7, 30, 13, 0));
}

#[test]
fn a_dated_wall_time_resolves_the_spring_forward_gap_identically() {
    // Supplying a date must not open a second DST path. Same expectation as the
    // undated case, but reached with `now` a week earlier so the date is what
    // selects the day rather than the clock.
    let now = utc(2026, 3, 1, 6, 0);
    let resolved = TriggerResolver::resolve(
        &TriggerSpec::on_date(date(2026, 3, 8), 2, 30),
        ny(),
        now,
    )
    .expect("resolve")
    .expect("has target");
    assert_eq!(resolved, utc(2026, 3, 8, 7, 0));
}

#[test]
fn a_dated_wall_time_takes_the_earlier_of_an_ambiguous_pair() {
    let now = utc(2026, 10, 25, 4, 0);
    let resolved = TriggerResolver::resolve(
        &TriggerSpec::on_date(date(2026, 11, 1), 1, 30),
        ny(),
        now,
    )
    .expect("resolve")
    .expect("has target");
    assert_eq!(resolved, utc(2026, 11, 1, 5, 30));
}

#[test]
fn validation_does_not_ask_whether_a_dated_instant_has_passed() {
    // It has no clock and no timezone to answer with, so it must not try. A date
    // long past is still structurally valid; `resolve` is what refuses it.
    assert!(TriggerResolver::validate(
        &TriggerSpec::on_date(date(2020, 1, 1), 20, 0),
        JobType::PowerOff,
    )
    .is_ok());
}

#[test]
fn a_date_does_not_weaken_the_time_of_day_bounds() {
    assert!(TriggerResolver::validate(
        &TriggerSpec::on_date(date(2026, 8, 10), 24, 0),
        JobType::PowerOff,
    )
    .is_err());
    assert!(TriggerResolver::validate(
        &TriggerSpec::on_date(date(2026, 8, 10), 12, 60),
        JobType::PowerOff,
    )
    .is_err());
}

// ------------------------------------------------------------- reconciliation

#[test]
fn future_target_is_still_pending() {
    let now = utc(2026, 7, 30, 10, 0);
    assert_eq!(
        TriggerResolver::reconcile(JobType::PowerOff, Some(utc(2026, 7, 30, 12, 0)), now),
        ReconcileOutcome::StillPending
    );
}

#[test]
fn indefinite_keep_awake_is_never_reconciled_away() {
    let now = utc(2026, 7, 30, 10, 0);
    assert_eq!(
        TriggerResolver::reconcile(JobType::KeepAwake, None, now),
        ReconcileOutcome::StillPending
    );
}

#[test]
fn overdue_keep_awake_completes() {
    let now = utc(2026, 7, 30, 10, 0);
    assert_eq!(
        TriggerResolver::reconcile(JobType::KeepAwake, Some(utc(2026, 7, 30, 9, 0)), now),
        ReconcileOutcome::Completed
    );
}

#[test]
fn keep_awake_completes_however_long_it_has_been_overdue() {
    // No tolerance applies to keep-awake: completing late is harmless.
    let now = utc(2026, 7, 30, 10, 0);
    assert_eq!(
        TriggerResolver::reconcile(JobType::KeepAwake, Some(utc(2026, 7, 20, 9, 0)), now),
        ReconcileOutcome::Completed
    );
}

#[test]
fn power_off_overdue_within_tolerance_proceeds_to_grace_period() {
    let now = utc(2026, 7, 30, 10, 0);
    assert_eq!(
        TriggerResolver::reconcile(JobType::PowerOff, Some(utc(2026, 7, 30, 9, 55)), now),
        ReconcileOutcome::ProceedToGracePeriod
    );
}

#[test]
fn power_off_overdue_beyond_tolerance_is_refused() {
    // The safety-critical case: a machine that slept past its target must not shut
    // down on wake.
    let now = utc(2026, 7, 30, 10, 0);
    assert_eq!(
        TriggerResolver::reconcile(JobType::PowerOff, Some(utc(2026, 7, 30, 9, 30)), now),
        ReconcileOutcome::Overdue
    );
}

#[test]
fn power_off_tolerance_boundary_is_inclusive() {
    // Exactly 15 minutes overdue still executes; one minute more does not. The
    // shared vectors encode this same choice, so it is asserted explicitly rather
    // than left to whichever comparison operator happened to be used.
    let now = utc(2026, 7, 30, 10, 0);

    let exactly_at_boundary =
        now - chrono::Duration::minutes(POWER_OFF_OVERTOLERANCE_MINUTES);
    assert_eq!(
        TriggerResolver::reconcile(JobType::PowerOff, Some(exactly_at_boundary), now),
        ReconcileOutcome::ProceedToGracePeriod,
        "15 minutes overdue is within tolerance"
    );

    let one_past_boundary =
        now - chrono::Duration::minutes(POWER_OFF_OVERTOLERANCE_MINUTES + 1);
    assert_eq!(
        TriggerResolver::reconcile(JobType::PowerOff, Some(one_past_boundary), now),
        ReconcileOutcome::Overdue,
        "16 minutes overdue is beyond tolerance"
    );
}

#[test]
fn power_off_overdue_by_hours_is_refused() {
    let now = utc(2026, 7, 30, 10, 0);
    assert_eq!(
        TriggerResolver::reconcile(JobType::PowerOff, Some(utc(2026, 7, 29, 22, 0)), now),
        ReconcileOutcome::Overdue
    );
}

//! How long a device has been silent, and what that means.
//!
//! A device that has not reported recently is not simply "offline". There is a middle
//! state, because a phone the operating system has suspended is neither reachable nor
//! gone, and reporting it as either one misleads the user: shown as online, a command
//! sent to it appears to be ignored; shown as offline, the user concludes the device has
//! stopped working.
//!
//! Pure, in the same sense [`crate::domain::TriggerResolver`] is pure: `now` is passed
//! in rather than read from a clock, so every boundary is reachable from a test on any
//! machine and the shared vectors in `shared/testvectors/presence.json` are
//! deterministic. The Dart implementation in `mobile/lib/domain/presence.dart` mirrors
//! this file, and those vectors are what keep the two from drifting.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

/// Silence up to and including this is still `Online`.
///
/// 90 seconds tolerates one missed report on a 60-second heartbeat plus jitter, without
/// requiring a second missed one. Not user-configurable: a tunable presence window would
/// let a device be configured to look online indefinitely.
pub const ONLINE_THRESHOLD_SECONDS: i64 = 90;

/// Silence beyond this is `Offline`; up to and including it is `Stale`.
///
/// 15 minutes matches the magnitude of [`POWER_OFF_OVERTOLERANCE_MINUTES`], reusing a
/// timescale the codebase already reasons about rather than introducing a third. It is a
/// separate constant regardless, because it answers a different question and coupling the
/// two would make one unchangeable without the other.
///
/// [`POWER_OFF_OVERTOLERANCE_MINUTES`]: crate::domain::POWER_OFF_OVERTOLERANCE_MINUTES
pub const OFFLINE_THRESHOLD_SECONDS: i64 = 900;

/// The online window must be strictly shorter than the offline one.
///
/// Checked at compile time rather than in a test: equal or inverted thresholds would
/// erase the `Stale` state entirely — the one thing the three-state model exists to
/// provide — and the mistake should not be buildable, not merely caught by a suite
/// someone might not run.
const _: () = assert!(
    ONLINE_THRESHOLD_SECONDS < OFFLINE_THRESHOLD_SECONDS,
    "an online window at or past the offline one would erase the stale state"
);

/// How reachable a device is, as three states rather than a boolean.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PresenceState {
    /// Reported within the online window; a command sent now should arrive.
    Online,
    /// Silent past the online window but within the offline one. Typically a phone the
    /// OS has backgrounded: it may come back on its own.
    Stale,
    /// Silent past the offline window, or never seen at all.
    Offline,
}

impl PresenceState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Online => "online",
            Self::Stale => "stale",
            Self::Offline => "offline",
        }
    }

    pub fn from_str_value(value: &str) -> Option<Self> {
        match value {
            "online" => Some(Self::Online),
            "stale" => Some(Self::Stale),
            "offline" => Some(Self::Offline),
            _ => None,
        }
    }
}

/// A presence decision and the silence it was decided from.
///
/// The elapsed time travels with the state rather than being left to the caller to
/// subtract. The UI needs it — "silent for one minute" and "silent for a day" are both
/// `Offline` and mean quite different things — and a second subtraction outside this
/// function is where the two implementations would drift by an off-by-one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresenceEvaluation {
    pub state: PresenceState,
    /// `None` only when the device has never reported: there is no age to give.
    /// Never negative — see the clamp in [`evaluate_presence`].
    pub elapsed: Option<Duration>,
}

/// Derives a device's presence from its last report and the current instant.
///
/// A `last_seen` in the *future* — a peer whose clock is skewed — clamps to zero elapsed
/// and reports `Online`. Treating it as an error would make the peer's clock skew look
/// like a local failure, and there is nothing the local user could do about it either
/// way. Two vector cases pin this, at five seconds and at a month of skew.
///
/// Both boundaries are inclusive-online: silence of exactly [`ONLINE_THRESHOLD_SECONDS`]
/// is `Online`, and of exactly [`OFFLINE_THRESHOLD_SECONDS`] is `Stale`. That is a
/// decision rather than an accident of which comparison operator was typed, which is why
/// each boundary is pinned from both sides in the shared vectors.
pub fn evaluate_presence(
    last_seen: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> PresenceEvaluation {
    let Some(last_seen) = last_seen else {
        // Never reported. The absent value must not stand in for `now`, which would
        // report a device that has never been seen as online.
        return PresenceEvaluation {
            state: PresenceState::Offline,
            elapsed: None,
        };
    };

    let elapsed = now.signed_duration_since(last_seen).max(Duration::zero());
    let seconds = elapsed.num_seconds();

    let state = if seconds <= ONLINE_THRESHOLD_SECONDS {
        PresenceState::Online
    } else if seconds <= OFFLINE_THRESHOLD_SECONDS {
        PresenceState::Stale
    } else {
        PresenceState::Offline
    };

    PresenceEvaluation {
        state,
        elapsed: Some(elapsed),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn utc(h: u32, m: u32, s: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 7, 30, h, m, s).unwrap()
    }

    /// Evaluates a device that last reported `seconds` before a fixed `now`. Negative
    /// values put the last report in the future.
    fn after(seconds: i64) -> PresenceEvaluation {
        let now = utc(10, 30, 0);
        evaluate_presence(Some(now - Duration::seconds(seconds)), now)
    }

    #[test]
    fn a_device_that_just_reported_is_online() {
        assert_eq!(after(5).state, PresenceState::Online);
    }

    #[test]
    fn exactly_the_online_threshold_is_still_online() {
        // The boundary is inclusive-online by decision, not by whichever comparison
        // operator happened to be typed. Asserted from both sides so a `<` slipping in
        // for a `<=` fails here rather than only in the cross-language vectors.
        assert_eq!(after(ONLINE_THRESHOLD_SECONDS).state, PresenceState::Online);
        assert_eq!(
            after(ONLINE_THRESHOLD_SECONDS + 1).state,
            PresenceState::Stale
        );
    }

    #[test]
    fn exactly_the_offline_threshold_is_still_stale() {
        assert_eq!(after(OFFLINE_THRESHOLD_SECONDS).state, PresenceState::Stale);
        assert_eq!(
            after(OFFLINE_THRESHOLD_SECONDS + 1).state,
            PresenceState::Offline
        );
    }

    #[test]
    fn the_thresholds_are_the_values_the_dart_side_uses() {
        // The ordering is asserted at compile time beside the constants, where a
        // regression cannot be built. What a test can still catch is a value drifting to
        // something both sides do not share — named here so this suite fails before the
        // cross-language one does, and points at the right file.
        assert_eq!(ONLINE_THRESHOLD_SECONDS, 90);
        assert_eq!(OFFLINE_THRESHOLD_SECONDS, 900);
    }

    #[test]
    fn a_device_that_never_reported_is_offline_with_no_age() {
        // The absent value must not be treated as the current instant, which would show
        // a device that has never been seen as online.
        let evaluation = evaluate_presence(None, utc(10, 30, 0));

        assert_eq!(evaluation.state, PresenceState::Offline);
        assert_eq!(evaluation.elapsed, None);
    }

    #[test]
    fn a_last_seen_in_the_future_clamps_to_zero_rather_than_failing() {
        // A peer with a skewed clock. Reporting an error here would make someone else's
        // clock look like a local failure, and negative elapsed time would print as a
        // countdown in the UI.
        let now = utc(10, 30, 0);
        let evaluation = evaluate_presence(Some(now + Duration::seconds(5)), now);

        assert_eq!(evaluation.state, PresenceState::Online);
        assert_eq!(evaluation.elapsed, Some(Duration::zero()));
    }

    #[test]
    fn a_last_seen_far_in_the_future_clamps_the_same_way() {
        // A month of skew, not five seconds: a clamp with a small tolerance would pass
        // the previous test and fail this one.
        let now = utc(10, 30, 0);
        let evaluation = evaluate_presence(Some(now + Duration::days(31)), now);

        assert_eq!(evaluation.state, PresenceState::Online);
        assert_eq!(evaluation.elapsed, Some(Duration::zero()));
    }

    #[test]
    fn the_elapsed_time_is_reported_for_every_state_that_has_one() {
        // Including `Online`, so a caller does not have to branch on the state to learn
        // how long it has been.
        for seconds in [0, 5, ONLINE_THRESHOLD_SECONDS + 1, 86_400] {
            let evaluation = after(seconds);
            assert_eq!(
                evaluation.elapsed.map(|elapsed| elapsed.num_seconds()),
                Some(seconds),
                "elapsed must survive for a device silent {seconds}s"
            );
        }
    }

    #[test]
    fn elapsed_time_is_never_negative() {
        for offset in [-1, -60, -86_400] {
            let evaluation = after(offset);
            assert!(
                evaluation.elapsed.expect("a reported device has an age") >= Duration::zero(),
                "a skewed peer clock must not produce a negative age"
            );
        }
    }

    #[test]
    fn state_strings_round_trip_and_match_the_dart_wire_names() {
        // These exact strings appear in the shared vectors, so they are a cross-language
        // contract rather than an internal detail.
        assert_eq!(PresenceState::Online.as_str(), "online");
        assert_eq!(PresenceState::Stale.as_str(), "stale");
        assert_eq!(PresenceState::Offline.as_str(), "offline");

        for state in [
            PresenceState::Online,
            PresenceState::Stale,
            PresenceState::Offline,
        ] {
            assert_eq!(PresenceState::from_str_value(state.as_str()), Some(state));
        }
        assert_eq!(PresenceState::from_str_value("nonsense"), None);
    }

    #[test]
    fn the_state_serialises_camel_case_like_the_other_domain_enums() {
        let json = serde_json::to_string(&PresenceState::Offline).expect("serialize");
        assert_eq!(json, "\"offline\"");
    }
}

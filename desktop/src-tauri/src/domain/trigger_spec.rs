use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// Maximum accepted duration, in minutes (24 hours).
pub const MAX_DURATION_MINUTES: i64 = 1440;

/// What condition fires a job.
///
/// The `kind` tag values match the Dart implementation's wire names, because the
/// shared test vectors are parsed by both.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TriggerSpec {
    /// Runs until cancelled manually. Valid only for `JobType::KeepAwake`.
    Indefinite,

    /// Fires `minutes` after activation. Must be in 1..=1440.
    Duration { minutes: i64 },

    /// Fires at a wall-clock time in the device's timezone.
    ///
    /// `date` decides which of two semantics applies, and they differ in one
    /// important way:
    ///
    /// - `None` — a *time of day*, resolved to its next occurrence. If that time has
    ///   already passed today it rolls to tomorrow.
    /// - `Some(d)` — a *one-off instant* on exactly that local date. It does **not**
    ///   roll forward; a dated instant that has passed is refused, because moving an
    ///   irreversible power-off to a day the user never chose is worse than refusing.
    ///
    /// The serde attributes are load-bearing rather than tidy. `default` lets every
    /// pre-existing payload keep deserializing, and `skip_serializing_if` keeps the
    /// undated form serializing byte-identically — which is what allows all the
    /// existing shared vector cases to pass unmodified.
    AbsoluteTime {
        hour: u32,
        minute: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        date: Option<NaiveDate>,
    },
}

impl TriggerSpec {
    /// An undated absolute time: the next occurrence of `hour:minute`.
    ///
    /// This and [`Self::on_date`] exist so that adding a further field to the variant
    /// costs two edits here rather than an edit at every construction site.
    pub fn at_time(hour: u32, minute: u32) -> Self {
        Self::AbsoluteTime {
            hour,
            minute,
            date: None,
        }
    }

    /// A dated absolute time: `hour:minute` on exactly `date`, never rolled forward.
    pub fn on_date(date: NaiveDate, hour: u32, minute: u32) -> Self {
        Self::AbsoluteTime {
            hour,
            minute,
            date: Some(date),
        }
    }

    pub fn kind_str(&self) -> &'static str {
        match self {
            Self::Indefinite => "indefinite",
            Self::Duration { .. } => "duration",
            Self::AbsoluteTime { .. } => "absoluteTime",
        }
    }

    /// Whether this trigger yields a concrete target instant. `Indefinite` does not,
    /// which is why power-off rejects it.
    pub fn has_target_instant(&self) -> bool {
        !matches!(self, Self::Indefinite)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("a real date")
    }

    #[test]
    fn only_indefinite_lacks_a_target_instant() {
        assert!(!TriggerSpec::Indefinite.has_target_instant());
        assert!(TriggerSpec::Duration { minutes: 30 }.has_target_instant());
        assert!(TriggerSpec::at_time(23, 30).has_target_instant());
        assert!(TriggerSpec::on_date(date(2026, 8, 10), 23, 30).has_target_instant());
    }

    #[test]
    fn kind_names_match_the_shared_vector_wire_format() {
        assert_eq!(TriggerSpec::Indefinite.kind_str(), "indefinite");
        assert_eq!(TriggerSpec::Duration { minutes: 1 }.kind_str(), "duration");
        assert_eq!(TriggerSpec::at_time(0, 0).kind_str(), "absoluteTime");
        // A date must not introduce a fourth kind: the databases and both vector
        // parsers treat the kind as a three-value domain.
        assert_eq!(
            TriggerSpec::on_date(date(2026, 8, 10), 0, 0).kind_str(),
            "absoluteTime"
        );
    }

    #[test]
    fn triggers_round_trip_through_serde() {
        for trigger in [
            TriggerSpec::Indefinite,
            TriggerSpec::Duration { minutes: 120 },
            TriggerSpec::at_time(6, 5),
            TriggerSpec::on_date(date(2026, 8, 10), 6, 5),
        ] {
            let json = serde_json::to_string(&trigger).expect("serialize");
            let parsed: TriggerSpec = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(trigger, parsed);
        }
    }

    #[test]
    fn an_undated_absolute_time_serializes_with_no_date_key() {
        // The whole basis of the backward-compatibility claim. If `date: null` were
        // emitted instead, every stored payload and every shared vector expectation
        // would have changed shape while the tests here still passed.
        let json = serde_json::to_string(&TriggerSpec::at_time(22, 30)).expect("serialize");
        assert_eq!(json, r#"{"kind":"absoluteTime","hour":22,"minute":30}"#);
    }

    #[test]
    fn a_dated_absolute_time_serializes_the_date_as_a_plain_calendar_day() {
        // `YYYY-MM-DD` with no time and no offset, because a wall-clock date carries
        // neither. The Dart side parses this same string.
        let json =
            serde_json::to_string(&TriggerSpec::on_date(date(2026, 8, 10), 22, 30)).expect("serialize");
        assert_eq!(
            json,
            r#"{"kind":"absoluteTime","hour":22,"minute":30,"date":"2026-08-10"}"#
        );
    }

    #[test]
    fn a_payload_without_a_date_still_deserializes() {
        // Rows written before this field existed, and every undated UI submission.
        let parsed: TriggerSpec =
            serde_json::from_str(r#"{"kind":"absoluteTime","hour":22,"minute":30}"#)
                .expect("deserialize");
        assert_eq!(parsed, TriggerSpec::at_time(22, 30));
    }

    #[test]
    fn a_malformed_date_is_refused_at_the_parse_boundary() {
        // Well-formedness is a parse concern, not a validation one: `NaiveDate` cannot
        // represent February 30th, so the resolver never has to consider it. This is
        // also why the shared vectors carry no malformed-date case — such a case would
        // make the vector *file* unparseable rather than exercise a rule.
        for bad in [
            r#"{"kind":"absoluteTime","hour":1,"minute":0,"date":"2026-02-30"}"#,
            r#"{"kind":"absoluteTime","hour":1,"minute":0,"date":"2026-13-01"}"#,
            r#"{"kind":"absoluteTime","hour":1,"minute":0,"date":"2026-08-00"}"#,
            r#"{"kind":"absoluteTime","hour":1,"minute":0,"date":"10/08/2026"}"#,
        ] {
            assert!(
                serde_json::from_str::<TriggerSpec>(bad).is_err(),
                "{bad} should not parse"
            );
        }
    }
}

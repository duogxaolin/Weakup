//! Whether a grant offered to establish a pairing may still be redeemed.
//!
//! Pairing is the only act that confers authority to command a device — not being signed in
//! to the same account, not being registered to the same owner. So this is the decision
//! that gates everything the acceptance rules later assume, and a grant that outlives its
//! usefulness is a grant someone else can use.
//!
//! # Why the out-of-band lifetime is *longer* in wall-clock terms
//!
//! This is the value most likely to be "corrected" by a later contributor who has read the
//! spec and not this comment, so the reasoning is recorded in full.
//!
//! The spec says out-of-band evidence is weaker: possession of a mailbox proves less than
//! possession of the machine. That is true, and yet
//! [`OUT_OF_BAND_GRANT_LIFETIME_SECONDS`] is twice
//! [`AT_MACHINE_GRANT_LIFETIME_SECONDS`]. The resolution is that the two windows are not
//! measuring the same thing. A code shown on screen is typed within seconds; 300 seconds is
//! already generous and exists only to absorb a user who walks between rooms. A code sent to
//! a mailbox must survive mail delivery latency, which is under nobody's control and
//! routinely exceeds a minute. Setting out-of-band to 300 seconds would fail honest users
//! often enough that they would retry repeatedly, and a flow that habitually fails trains
//! people to expect failure.
//!
//! The weaker evidence is compensated where it actually matters: the grant is single-use, it
//! is delivered only to an address the owner has already proven control of and never to one
//! supplied in the request to pair, and it authorises *pairing* rather than a shutdown. An
//! intercepted grant still cannot power anything off without the person also completing a
//! pairing the owner can see and revoke.
//!
//! Stated as a trade-off rather than hidden: if mail delivery turns out to be fast in
//! practice, the right change is to shorten the out-of-band lifetime. Both constants live
//! here so that is a two-line edit plus vector updates.
//!
//! The Dart implementation in `mobile/lib/domain/pairing_grant.dart` mirrors this file,
//! including the rejection messages verbatim.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// How a grant reached the person redeeming it.
///
/// The delivery path is what selects the lifetime, so it is part of the grant rather than a
/// separate argument a caller could get wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GrantDelivery {
    /// Shown at the target device itself. Possession of the machine is direct evidence of
    /// the authority being granted.
    AtMachine,
    /// Delivered out of band to the account owner's established address.
    OutOfBand,
}

/// Lifetime of a grant shown at the target machine. See the module comment.
pub const AT_MACHINE_GRANT_LIFETIME_SECONDS: i64 = 300;

/// Lifetime of a grant delivered out of band. Longer in wall-clock terms, for the reason the
/// module comment sets out at length — it absorbs mail delivery latency, not weaker evidence.
pub const OUT_OF_BAND_GRANT_LIFETIME_SECONDS: i64 = 600;

/// Neither lifetime may be open-ended, and both must be long enough to type a code.
///
/// Checked at compile time so a contributor who zeroes one out to "disable expiry" gets a
/// build error. A grant that never expires is a grant an observer can use tomorrow.
const _: () = assert!(
    AT_MACHINE_GRANT_LIFETIME_SECONDS > 0 && OUT_OF_BAND_GRANT_LIFETIME_SECONDS > 0,
    "a grant with no lifetime never expires, and an observed grant would stay usable"
);

impl GrantDelivery {
    /// How long a grant delivered this way stays redeemable.
    ///
    /// The single place the mapping lives, so the lifetime enforced and the lifetime
    /// documented cannot drift apart.
    pub fn lifetime_seconds(self) -> i64 {
        match self {
            Self::AtMachine => AT_MACHINE_GRANT_LIFETIME_SECONDS,
            Self::OutOfBand => OUT_OF_BAND_GRANT_LIFETIME_SECONDS,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::AtMachine => "atMachine",
            Self::OutOfBand => "outOfBand",
        }
    }

    pub fn from_str_value(value: &str) -> Option<Self> {
        match value {
            "atMachine" => Some(Self::AtMachine),
            "outOfBand" => Some(Self::OutOfBand),
            _ => None,
        }
    }
}

/// Why a presented grant was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GrantRejection {
    /// The grant's lifetime has elapsed.
    Expired,
    /// The grant has been redeemed before. Single use is what makes a grant observed in
    /// transit worthless after the fact.
    AlreadyUsed,
    /// The target never issued this grant — a mistyped or invented code.
    NoSuchGrant,
}

impl GrantRejection {
    /// Prose for the person redeeming the grant.
    ///
    /// **Byte-identical to the strings in `mobile/lib/domain/pairing_grant.dart`.**
    pub fn user_message(self) -> String {
        match self {
            Self::Expired => {
                "That pairing code has expired. Start pairing again to get a new one."
                    .to_string()
            }
            Self::AlreadyUsed => {
                "That pairing code has already been used. Each code works only once."
                    .to_string()
            }
            Self::NoSuchGrant => {
                "That pairing code was not recognised. Check it and try again."
                    .to_string()
            }
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Expired => "expired",
            Self::AlreadyUsed => "alreadyUsed",
            Self::NoSuchGrant => "noSuchGrant",
        }
    }

    pub fn from_str_value(value: &str) -> Option<Self> {
        match value {
            "expired" => Some(Self::Expired),
            "alreadyUsed" => Some(Self::AlreadyUsed),
            "noSuchGrant" => Some(Self::NoSuchGrant),
            _ => None,
        }
    }
}

/// Valid, or refused with exactly one reason. The same shape, and the same argument, as
/// [`CommandAcceptance`].
///
/// [`CommandAcceptance`]: crate::domain::CommandAcceptance
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrantValidity {
    Valid,
    Invalid(GrantRejection),
}

impl GrantValidity {
    pub fn is_valid(self) -> bool {
        matches!(self, Self::Valid)
    }

    pub fn rejection(self) -> Option<GrantRejection> {
        match self {
            Self::Valid => None,
            Self::Invalid(reason) => Some(reason),
        }
    }
}

/// A grant as presented for redemption.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairingGrant {
    /// How it reached the person redeeming it, which selects its lifetime.
    pub delivery: GrantDelivery,
    /// When the target issued it.
    pub issued_at: DateTime<Utc>,
    /// Whether it has been redeemed before.
    pub redeemed: bool,
    /// Whether the target recognises it as one it issued.
    pub recognised: bool,
}

/// Decides whether this grant may still be redeemed.
///
/// **The order of these checks is part of the contract**, and the shared vectors pin it:
/// recognition first, then [`AlreadyUsed`], then [`Expired`].
///
/// Recognition comes first because a target has no reason to trust the claimed fields of a
/// grant it never issued — computing an age from an attacker-supplied `issued_at` and
/// reporting "expired" would answer a question about data the target does not stand behind.
///
/// [`AlreadyUsed`] precedes [`Expired`] so that an already-redeemed grant reports that fact
/// whether or not it has also expired. The two indicate different situations to the person
/// holding the code: one means try again, the other means this code is spent, and a user
/// told the wrong one goes looking for a new code they already have.
///
/// [`AlreadyUsed`]: GrantRejection::AlreadyUsed
/// [`Expired`]: GrantRejection::Expired
pub fn evaluate_grant(grant: &PairingGrant, now: DateTime<Utc>) -> GrantValidity {
    if !grant.recognised {
        return GrantValidity::Invalid(GrantRejection::NoSuchGrant);
    }

    if grant.redeemed {
        return GrantValidity::Invalid(GrantRejection::AlreadyUsed);
    }

    // Inclusive-accept: an age of exactly the lifetime is still valid, pinned from both
    // sides in the vectors so it cannot be decided by whichever operator was typed.
    let age_seconds = (now - grant.issued_at).num_seconds();
    if age_seconds > grant.delivery.lifetime_seconds() {
        return GrantValidity::Invalid(GrantRejection::Expired);
    }

    GrantValidity::Valid
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn issued() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 8, 6, 12, 0, 0).unwrap()
    }

    fn grant(delivery: GrantDelivery) -> PairingGrant {
        PairingGrant {
            delivery,
            issued_at: issued(),
            redeemed: false,
            recognised: true,
        }
    }

    #[test]
    fn a_grant_at_exactly_its_lifetime_is_still_valid() {
        // Both boundaries are inclusive-accept, and each is asserted against the constant
        // rather than a literal so changing the constant moves the test with it.
        for delivery in [GrantDelivery::AtMachine, GrantDelivery::OutOfBand] {
            let at_boundary =
                issued() + chrono::Duration::seconds(delivery.lifetime_seconds());
            assert_eq!(
                evaluate_grant(&grant(delivery), at_boundary),
                GrantValidity::Valid,
                "{delivery:?} at exactly its lifetime must be valid"
            );
        }
    }

    #[test]
    fn a_grant_one_second_past_its_lifetime_has_expired() {
        for delivery in [GrantDelivery::AtMachine, GrantDelivery::OutOfBand] {
            let past =
                issued() + chrono::Duration::seconds(delivery.lifetime_seconds() + 1);
            assert_eq!(
                evaluate_grant(&grant(delivery), past),
                GrantValidity::Invalid(GrantRejection::Expired),
                "{delivery:?} one second past its lifetime must be expired"
            );
        }
    }

    #[test]
    fn an_age_between_the_two_lifetimes_splits_by_delivery_path() {
        // The case that proves the two lifetimes are actually distinct. An implementation
        // using one lifetime for both passes every other test in this file.
        let between = issued()
            + chrono::Duration::seconds(
                (AT_MACHINE_GRANT_LIFETIME_SECONDS + OUT_OF_BAND_GRANT_LIFETIME_SECONDS) / 2,
            );

        assert_eq!(
            evaluate_grant(&grant(GrantDelivery::AtMachine), between),
            GrantValidity::Invalid(GrantRejection::Expired)
        );
        assert_eq!(
            evaluate_grant(&grant(GrantDelivery::OutOfBand), between),
            GrantValidity::Valid
        );
    }

    #[test]
    fn the_two_lifetimes_are_not_the_same_value() {
        // Guards the split test above: if a later edit made them equal, that test would
        // still compile and could pass vacuously depending on the midpoint.
        assert_ne!(
            AT_MACHINE_GRANT_LIFETIME_SECONDS,
            OUT_OF_BAND_GRANT_LIFETIME_SECONDS
        );
    }

    #[test]
    fn an_already_used_grant_reports_that_rather_than_expiry() {
        let used = PairingGrant {
            redeemed: true,
            ..grant(GrantDelivery::AtMachine)
        };

        // Before expiry.
        assert_eq!(
            evaluate_grant(&used, issued() + chrono::Duration::seconds(60)),
            GrantValidity::Invalid(GrantRejection::AlreadyUsed)
        );

        // And after it — the precedence pair. Both reasons hold; single use is the one
        // worth reporting, because it is the one that will not change by waiting.
        assert_eq!(
            evaluate_grant(
                &used,
                issued()
                    + chrono::Duration::seconds(OUT_OF_BAND_GRANT_LIFETIME_SECONDS * 2)
            ),
            GrantValidity::Invalid(GrantRejection::AlreadyUsed)
        );
    }

    #[test]
    fn an_unrecognised_grant_is_distinguished_from_an_expired_one() {
        let unknown = PairingGrant {
            recognised: false,
            ..grant(GrantDelivery::AtMachine)
        };

        assert_eq!(
            evaluate_grant(&unknown, issued() + chrono::Duration::seconds(1)),
            GrantValidity::Invalid(GrantRejection::NoSuchGrant)
        );

        // Recognition is settled before any age is computed from fields the target does not
        // stand behind.
        assert_eq!(
            evaluate_grant(
                &unknown,
                issued()
                    + chrono::Duration::seconds(OUT_OF_BAND_GRANT_LIFETIME_SECONDS * 2)
            ),
            GrantValidity::Invalid(GrantRejection::NoSuchGrant)
        );
    }

    #[test]
    fn a_decision_carries_a_reason_exactly_when_it_is_a_refusal() {
        assert_eq!(GrantValidity::Valid.rejection(), None);
        assert!(GrantValidity::Valid.is_valid());

        let invalid = GrantValidity::Invalid(GrantRejection::Expired);
        assert_eq!(invalid.rejection(), Some(GrantRejection::Expired));
        assert!(!invalid.is_valid());
    }

    #[test]
    fn every_rejection_has_a_distinct_message_a_person_can_act_on() {
        let messages: Vec<String> = [
            GrantRejection::Expired,
            GrantRejection::AlreadyUsed,
            GrantRejection::NoSuchGrant,
        ]
        .into_iter()
        .map(GrantRejection::user_message)
        .collect();

        for message in &messages {
            assert!(!message.trim().is_empty());
            assert!(
                message.chars().any(char::is_lowercase),
                "messages are prose, not identifiers: {message}"
            );
        }

        let unique: std::collections::HashSet<&String> = messages.iter().collect();
        assert_eq!(unique.len(), messages.len(), "two reasons share a message");
    }

    #[test]
    fn wire_names_round_trip_and_match_the_dart_ones() {
        // These exact strings appear in the shared vectors, so they are a cross-language
        // contract rather than an internal detail.
        assert_eq!(GrantDelivery::AtMachine.as_str(), "atMachine");
        assert_eq!(GrantDelivery::OutOfBand.as_str(), "outOfBand");
        for delivery in [GrantDelivery::AtMachine, GrantDelivery::OutOfBand] {
            assert_eq!(
                GrantDelivery::from_str_value(delivery.as_str()),
                Some(delivery)
            );
        }
        assert_eq!(GrantDelivery::from_str_value("nonsense"), None);

        assert_eq!(GrantRejection::Expired.as_str(), "expired");
        assert_eq!(GrantRejection::AlreadyUsed.as_str(), "alreadyUsed");
        assert_eq!(GrantRejection::NoSuchGrant.as_str(), "noSuchGrant");
        for reason in [
            GrantRejection::Expired,
            GrantRejection::AlreadyUsed,
            GrantRejection::NoSuchGrant,
        ] {
            assert_eq!(GrantRejection::from_str_value(reason.as_str()), Some(reason));
        }
        assert_eq!(GrantRejection::from_str_value("nonsense"), None);
    }
}

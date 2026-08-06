//! Whether an arriving command is *real*, as distinct from whether it is *permitted*.
//!
//! Authorization answers permission and keeps its own reason set. This module answers the
//! prior question — is this command genuinely from a paired device, recent enough to obey,
//! and one we have not already acted on — and it runs first.
//!
//! # Why this is one decision rather than three checks a caller composes
//!
//! The composition *is* the security property. A caller that checks authenticity and
//! forgets replay has written a working system with a silent hole, and nothing in the type
//! system objects. Composing once, inside the vector-covered function, means the ordering
//! and the completeness are pinned by tests rather than by each call site's discipline.
//!
//! This wraps [`authorize`] rather than replacing it. The seam is deliberate: permission is
//! a policy that will change as features are added; authenticity is not.
//!
//! # This module decides; it does not act
//!
//! Like `remote_command.rs`, this file must never reach the countdown gate or the executor,
//! and that is enforced rather than asked for: a source-text test in
//! `command_acceptance_tests.rs` fails the build if this file so much as names them. An
//! accepted command is carried out by creating a *job*, which the target's existing
//! scheduler then runs by its existing rules — including the mandatory cancellable
//! countdown a remote-origin job earns. Establishing that a command is genuine must not
//! shorten, skip, or bypass that countdown: an authentic, fresh, unreplayed, permitted
//! command is the *normal* case, and the countdown exists for exactly that case.
//!
//! The Dart implementation in `mobile/lib/domain/command_acceptance.dart` mirrors this
//! file, including the rejection messages verbatim.
//! `shared/testvectors/command_acceptance.json` is what keeps the two from drifting.

use std::collections::HashSet;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::command_envelope::CommandEnvelope;
use crate::domain::remote_command::{authorize, RemoteCommandContext, RemoteCommandDecision};

/// How old a command may be and still be obeyed.
///
/// Two minutes survives a phone that was backgrounded mid-send, a relay retry, and ordinary
/// mobile latency, without leaving a captured command usable for a meaningful period.
///
/// Deliberately **not** the 90 seconds of `ONLINE_THRESHOLD_SECONDS`: presence asks "is this
/// device still there", which tolerates a missed heartbeat, whereas freshness asks "was this
/// command issued just now". One constant for both would couple two unrelated questions and
/// make either unchangeable without the other.
pub const FRESHNESS_WINDOW_SECONDS: i64 = 120;

/// How far ahead of the target's clock a command may claim to have been created.
///
/// Small on purpose. Every second of tolerance is a second added to the window in which a
/// captured command remains obeyable, so this trades against the same property freshness
/// protects. Thirty seconds is enough for unsynchronised consumer clocks and not enough to
/// be useful to an attacker.
pub const FUTURE_TOLERANCE_SECONDS: i64 = 30;

/// The shortest time a nonce may be remembered for.
///
/// Not a free parameter. A nonce may be forgotten only once the command bearing it can no
/// longer pass the freshness check — otherwise forgetting reopens the replay window the
/// nonce existed to close. Hence the assertion below, which derives the floor rather than
/// trusting this number.
///
/// Note what this is *not*: it is not a promise about how long the decision record is kept.
/// That record has its own lifetime and a different purpose — attribution rather than
/// replay defence.
pub const NONCE_RETENTION_SECONDS: i64 = 600;

/// Forgetting a nonce sooner than a command bearing it could still be accepted would reopen
/// the replay window the nonce exists to close.
///
/// Checked at compile time rather than in a test, in the same style as the assertion that
/// the remote countdown exceeds the local one: a contributor who shortens retention or
/// lengthens the freshness window gets a build error rather than a silent hole.
const _: () = assert!(
    NONCE_RETENTION_SECONDS > FRESHNESS_WINDOW_SECONDS + FUTURE_TOLERANCE_SECONDS,
    "nonce retention must outlast the window in which a command can still be accepted; \
     shortening it reopens the replay window the nonce exists to close"
);

/// Why an arriving command was refused.
///
/// Closed rather than a string for two reasons: adding a sixth reason becomes a compile
/// error at every match site, and the shared vectors can name reasons exactly rather than
/// matching on prose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RejectionReason {
    /// The command's signature did not verify against a pairing this target holds. An
    /// intermediary's assertion that a command is genuine counts for nothing here.
    AuthenticityUnverified,
    /// This target has already acted on a command bearing this nonce.
    ReplayedNonce,
    /// The command claims to have been created further ahead than the clock tolerance
    /// allows. Distinct from staleness: a sender clock running ahead is a different fault
    /// from a command that sat too long.
    FutureDated,
    /// The command was created longer ago than the freshness window allows.
    Stale,
    /// The command is authentic, fresh, and new, but the authorization rule refuses it.
    /// The specific permission reason is deliberately not disclosed here — see
    /// [`evaluate_command`].
    NotPermitted,
}

impl RejectionReason {
    /// Prose for the person who asked, in the style of [`AppError::user_message`] and
    /// [`DenialReason::user_message`].
    ///
    /// Specific per variant rather than a generic failure: a refusal the user cannot act on
    /// is indistinguishable from a bug. **These strings are byte-identical to the ones in
    /// `mobile/lib/domain/command_acceptance.dart`** — a user who sees a refusal on their
    /// phone and again on their laptop must not be told two different things.
    ///
    /// [`AppError::user_message`]: crate::core::AppError::user_message
    /// [`DenialReason::user_message`]: crate::domain::DenialReason::user_message
    pub fn user_message(self) -> String {
        match self {
            Self::AuthenticityUnverified => {
                "That command could not be confirmed as coming from a paired device. It \
                 was ignored."
                    .to_string()
            }
            Self::ReplayedNonce => {
                "That command had already been carried out once. Repeating it was \
                 refused."
                    .to_string()
            }
            Self::FutureDated => {
                "That command is dated too far in the future. Check the clock on the \
                 device that sent it."
                    .to_string()
            }
            Self::Stale => {
                "That command took too long to arrive and was refused. Send it again."
                    .to_string()
            }
            Self::NotPermitted => {
                "That command is not permitted on the target device.".to_string()
            }
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::AuthenticityUnverified => "authenticityUnverified",
            Self::ReplayedNonce => "replayedNonce",
            Self::FutureDated => "futureDated",
            Self::Stale => "stale",
            Self::NotPermitted => "notPermitted",
        }
    }

    pub fn from_str_value(value: &str) -> Option<Self> {
        match value {
            "authenticityUnverified" => Some(Self::AuthenticityUnverified),
            "replayedNonce" => Some(Self::ReplayedNonce),
            "futureDated" => Some(Self::FutureDated),
            "stale" => Some(Self::Stale),
            "notPermitted" => Some(Self::NotPermitted),
            _ => None,
        }
    }
}

/// Accepted, or refused with exactly one reason.
///
/// The same shape as [`RemoteCommandDecision`], and for the same reason: a
/// `(bool, Option<RejectionReason>)` pair would admit accepted-with-a-reason and
/// refused-without-one, and something would eventually construct one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandAcceptance {
    Accepted,
    Rejected(RejectionReason),
}

impl CommandAcceptance {
    pub fn is_accepted(self) -> bool {
        matches!(self, Self::Accepted)
    }

    /// The refusal reason, or `None` when accepted.
    pub fn rejection_reason(self) -> Option<RejectionReason> {
        match self {
            Self::Accepted => None,
            Self::Rejected(reason) => Some(reason),
        }
    }
}

/// Everything about the *target* that [`evaluate_command`] needs, named.
///
/// The nonce store is an input rather than something this rule looks up or prunes. Choosing
/// a pruning policy needs operational data this change does not have;
/// [`NONCE_RETENTION_SECONDS`] states the floor below which pruning is unsafe, and the
/// change that owns the store picks a policy above it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandTargetState {
    /// Nonces this target has already acted on.
    pub seen_nonces: HashSet<String>,
    /// Whether the *target* can shut itself down.
    pub target_can_power_off: bool,
    /// Whether the *target* can act as a remote-control target at all.
    pub target_is_remote_target: bool,
    /// Whether the sending device is paired with this target. Derived from a pairing the
    /// target itself holds and has verified the command against — never from an account
    /// session, an owner record, or a relay's assertion.
    pub is_paired: bool,
    /// Whether the target's owner has turned remote control on, at the target.
    pub remote_control_enabled: bool,
}

/// Decides whether this target will act on this command.
///
/// **The order of these checks is part of the contract**, and the shared vectors pin it:
/// [`AuthenticityUnverified`] → [`ReplayedNonce`] → [`FutureDated`] → [`Stale`] →
/// [`NotPermitted`]. Several reasons can hold at once, and an unspecified order would let
/// both implementations return "a" correct refusal while disagreeing case by case.
///
/// The order is not arbitrary. Authenticity is first because the spec requires it: every
/// reason after the first leaks something about the target's state, so a caller must not be
/// able to probe permission state, freshness windows, or which nonces this target has seen
/// by sending unauthenticated commands. Replay precedes the two time reasons deliberately —
/// a replayed command is evidence of an attack whereas a stale one is more often a bad
/// network, and reporting the more serious finding when both hold means the record shows an
/// attack as an attack. [`NotPermitted`] is last because it leaks the most, and is reached
/// only by a command already authentic, fresh, and new.
///
/// Permission is delegated to [`authorize`] rather than reimplemented, and its specific
/// reason is deliberately collapsed into [`NotPermitted`] here. The two rules answer
/// different questions and keep separate reason sets.
///
/// [`AuthenticityUnverified`]: RejectionReason::AuthenticityUnverified
/// [`ReplayedNonce`]: RejectionReason::ReplayedNonce
/// [`FutureDated`]: RejectionReason::FutureDated
/// [`Stale`]: RejectionReason::Stale
/// [`NotPermitted`]: RejectionReason::NotPermitted
pub fn evaluate_command(
    envelope: &CommandEnvelope,
    target_state: &CommandTargetState,
    now: DateTime<Utc>,
) -> CommandAcceptance {
    if !envelope.signature_verified {
        return CommandAcceptance::Rejected(RejectionReason::AuthenticityUnverified);
    }

    if target_state.seen_nonces.contains(&envelope.nonce) {
        return CommandAcceptance::Rejected(RejectionReason::ReplayedNonce);
    }

    // Positive when the command is older than `now`, negative when it claims the future.
    // One subtraction serves both bounds, so the two cannot drift apart.
    let age_seconds = (now - envelope.created_at).num_seconds();

    // Both boundaries are inclusive-accept: an age of exactly the window is accepted, and a
    // future offset of exactly the tolerance is accepted. A decision, not an accident of
    // which comparison operator was typed, which is why each is pinned from both sides.
    if age_seconds < -FUTURE_TOLERANCE_SECONDS {
        return CommandAcceptance::Rejected(RejectionReason::FutureDated);
    }

    if age_seconds > FRESHNESS_WINDOW_SECONDS {
        return CommandAcceptance::Rejected(RejectionReason::Stale);
    }

    let context = RemoteCommandContext {
        command: envelope.command,
        target_can_power_off: target_state.target_can_power_off,
        target_is_remote_target: target_state.target_is_remote_target,
        is_paired: target_state.is_paired,
        remote_control_enabled: target_state.remote_control_enabled,
    };

    match authorize(&context) {
        RemoteCommandDecision::Allowed => CommandAcceptance::Accepted,
        RemoteCommandDecision::Denied(_) => {
            CommandAcceptance::Rejected(RejectionReason::NotPermitted)
        }
    }
}

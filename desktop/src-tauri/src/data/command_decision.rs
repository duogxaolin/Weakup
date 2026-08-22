//! The record of what this device decided about an arriving remote command.
//!
//! A power-off cannot be undone. Without a record, a user whose machine shut down has no way
//! to determine afterwards which device caused it, and a refused attempt leaves no trace that
//! anything was attempted at all. This is what makes an irreversible action attributable.
//!
//! Refusals are recorded as well as acceptances, and that is the more important half: a
//! series of refusals is the visible signature of an attack in progress, and a record that
//! only existed when the command succeeded would be blind to the case it is most needed for.
//!
//! Local by requirement — never sent to a relay. That is partly a privacy position, since a
//! record of when commands arrive is also a record of when the owner is at their machine, and
//! partly availability: it must be readable at the target when no network is reachable.

use chrono::{DateTime, Utc};

use crate::domain::{CommandAcceptance, DeviceId, RejectionReason, RemoteCommand};

/// One decision reached about one arriving command.
///
/// The decision and its reason are carried as a [`CommandAcceptance`] rather than as a
/// string plus an optional reason, so a record cannot be constructed claiming acceptance
/// with a refusal reason attached.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandDecisionRecord {
    /// The device that sent the command.
    pub sender: DeviceId,
    /// What it asked for.
    pub command: RemoteCommand,
    /// What was decided, including the single reason when refused.
    pub acceptance: CommandAcceptance,
    /// When the decision was reached.
    pub decided_at_utc: DateTime<Utc>,
}

impl CommandDecisionRecord {
    /// The stored form of the decision: `accepted` or `rejected`.
    ///
    /// Separate from the reason so a reader can count refusals without interpreting them.
    pub fn decision_str(&self) -> &'static str {
        match self.acceptance {
            CommandAcceptance::Accepted => "accepted",
            CommandAcceptance::Rejected(_) => "rejected",
        }
    }

    /// The stored refusal reason, or `None` when accepted.
    pub fn rejection_reason(&self) -> Option<RejectionReason> {
        self.acceptance.rejection_reason()
    }
}

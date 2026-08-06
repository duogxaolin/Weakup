//! What a command looks like when it arrives from another device.
//!
//! The Dart implementation in `mobile/lib/domain/command_envelope.dart` mirrors this file,
//! carrying the same field set and the same documented omission.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::device_id::DeviceId;
use crate::domain::remote_command::RemoteCommand;

/// A command as received, with everything the acceptance rule needs to judge it.
///
/// # What this struct deliberately does not carry
///
/// There is **no `relay_attested`, no `server_verified`, and no `trusted_source`** — no
/// field of any kind that an intermediary could populate to influence the decision. That
/// omission is the design, not an oversight.
///
/// The architecture this change exists to honour is that the *sending device* signs and the
/// relay only forwards. A relay must be able to drop or delay a command and must never be
/// able to invent one, because that is what keeps a compromised relay from being a shutdown
/// for every user of the service. A struct with a field a relay could set would satisfy
/// that requirement in prose while contradicting it in shape — and a field that exists
/// eventually gets read. Omitting it makes the requirement true by construction rather than
/// by review, the same argument [`PowerOffGate`] makes for taking no duration parameter.
///
/// # Why `signature_verified` is a bool and not a signature
///
/// The rules take a verification *outcome* the caller computed, not a signature they check.
/// Verifying one needs a key, a key store, an algorithm, and a platform; none of those can
/// be pure, none can be identical across Rust and Dart, and vectors carrying real key
/// material would test each language's crypto library rather than the shared decision.
///
/// The field is named for what it asserts rather than for what a caller wants, so a caller
/// hardcoding it reads as obviously wrong at the call site. That is a mitigation and not a
/// guarantee: **nothing here prevents a caller passing `true` unconditionally**, and no test
/// in this change can catch it because the caller does not exist yet. The change that
/// introduces the transport must verify signatures against a key held only by the paired
/// devices and must carry an end-to-end test that an invalid signature is rejected. Until
/// then the authenticity guarantee is specified, not enforced.
///
/// [`PowerOffGate`]: crate::application::PowerOffGate
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandEnvelope {
    /// The device that produced this command.
    pub sender: DeviceId,
    /// What is being asked for.
    pub command: RemoteCommand,
    /// The instant the *sending* device says it created this command.
    ///
    /// A claim rather than a fact — the target cannot verify another device's clock — which
    /// is why it is bounded on both sides: too old is stale, too far ahead is future-dated.
    pub created_at: DateTime<Utc>,
    /// What distinguishes this command from every other one from the same device.
    ///
    /// A signature alone does not prevent replay: a captured command stays valid, and
    /// resent immediately it is still fresh. Freshness and this value are both required and
    /// neither substitutes for the other.
    pub nonce: String,
    /// Whether the caller verified this command's signature against a pairing the target
    /// holds. See the note above — this is an input, not something these rules compute.
    pub signature_verified: bool,
}

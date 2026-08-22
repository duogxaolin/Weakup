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
/// # Why this carries a signature and not a verdict
///
/// The field here is `signature` — the actual bytes — and there is deliberately **no
/// `signature_verified` boolean**. An earlier version of this struct had one, and its own
/// doc comment recorded the problem: nothing prevented a caller passing `true`
/// unconditionally, so the authenticity guarantee was specified rather than enforced.
///
/// Replacing the boolean rather than adding alongside it is the entire point. If both
/// existed, every call site would face a choice between doing the work and asserting the
/// answer, and some call site would eventually assert — a transport, most likely, on the
/// day it was written against a rule that accepted either. Deleting the field makes the
/// shortcut unrepresentable. That is the same argument the omission above makes, and the
/// same one [`PowerOffGate`] makes by taking no duration parameter.
///
/// A caller can now only present evidence. [`evaluate_command`] encodes the signed content
/// canonically, looks up the verifying key the target holds for the claimed sender, and
/// checks the arithmetic itself.
///
/// What this does *not* yet establish is where the signing key lives. A verified signature
/// proves the command came from whoever holds that key; nothing here guarantees it is held
/// only by the paired devices, because key storage belongs to the change that has a device
/// to run on.
///
/// [`PowerOffGate`]: crate::application::PowerOffGate
/// [`evaluate_command`]: crate::domain::evaluate_command
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
    ///
    /// Covered by the signature, so a relay cannot rewrite it to revive a stale command.
    pub created_at: DateTime<Utc>,
    /// What distinguishes this command from every other one from the same device.
    ///
    /// A signature alone does not prevent replay: a captured command stays valid, and
    /// resent immediately it is still fresh. Freshness and this value are both required and
    /// neither substitutes for the other.
    ///
    /// Covered by the signature, so a relay cannot change it to make a replay look new.
    pub nonce: String,
    /// The sending device's Ed25519 signature over the canonical encoding of this command.
    ///
    /// Evidence, not a verdict. These bytes are attacker-controlled in the threat model —
    /// a relay can put anything here — which is why verification returns `false` for
    /// malformed input rather than failing in any way a caller must handle.
    ///
    /// The bytes signed are produced by
    /// [`encode_signing_payload`](crate::domain::encode_signing_payload) and cover the
    /// sender, the command, the creation instant, and the nonce. A field outside that
    /// encoding is a field an intermediary could change undetected.
    pub signature: Vec<u8>,
}

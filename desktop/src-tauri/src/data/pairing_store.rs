//! The pairings this device holds, and the key map the acceptance rule checks against.
//!
//! `remote-device-pairing` specifies pairing as the only act that confers authority to
//! command a device, and specifies revocation. Until this module existed there was nowhere
//! to write either, so both were specified-but-unimplemented: pairing was a decision function
//! with nothing behind it.
//!
//! # Revocation is a flag, not a deletion
//!
//! Revoking sets `revoked_at`; it does not remove the row (design D4). Deleting would lose
//! the record that a pairing ever existed — which is exactly what someone investigating an
//! unexplained shutdown wants to see — and would make "was this device ever paired?"
//! unanswerable, a question that matters most after a device is lost.
//!
//! Re-pairing a revoked device clears `revoked_at` on the **same row** rather than inserting
//! a second. Two rows for one peer would make "is this device authorized?" depend on which
//! one is read first, and that kind of ambiguity eventually resolves the wrong way. The peer
//! id is the table's primary key, so a second row is not merely discouraged but impossible.
//!
//! # A revoked pairing's key is absent, not present-and-rejected
//!
//! [`verifying_keys_from_pairings`] filters revoked rows out of the map, so a command from a
//! revoked peer fails as an *unknown sender* (design D5). Including the key and refusing at a
//! later pairing check would verify the signature of a device the user explicitly
//! de-authorized, then refuse it for a different reason — reporting the wrong thing and doing
//! cryptographic work on a revoked device's behalf. Absent is the honest representation of
//! revoked.
//!
//! The Dart implementation in `mobile/lib/data/pairing_store.dart` mirrors this file, and
//! `shared/testvectors/pairing_store.json` pins the decisions both sides must agree on.

use std::collections::HashMap;

use chrono::{DateTime, Utc};

use crate::core::AppResult;
use crate::domain::{DeviceId, VerifyingKey};

/// One peer this device is paired with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairingRecord {
    pub peer: DeviceId,
    /// The key recorded when this peer was paired. Subsequent commands from it are checked
    /// against exactly this, never against a key supplied alongside a command.
    pub verifying_key: VerifyingKey,
    /// Whether the pairing has been revoked. A revoked pairing is retained for the record
    /// but confers no authority.
    pub revoked: bool,
}

/// Builds the key map [`evaluate_command`] checks signatures against.
///
/// **Revoked pairings are excluded**, per design D5 — see the module comment for why absent
/// rather than present-and-rejected-later is the honest representation.
///
/// This is the *only* function in production code that constructs a `verifying_keys` map. The
/// rule itself still takes the map as data, which keeps it pure and keeps every vector case
/// writable without a store fixture; what changed is that no caller invents one.
/// `pairing_store_tests.rs` asserts that structurally.
///
/// [`evaluate_command`]: crate::domain::evaluate_command
pub fn verifying_keys_from_pairings(
    pairings: &[PairingRecord],
) -> HashMap<DeviceId, VerifyingKey> {
    pairings
        .iter()
        .filter(|record| !record.revoked)
        .map(|record| (record.peer.clone(), record.verifying_key.clone()))
        .collect()
}

/// Where pairings are kept.
///
/// A trait so the acceptance path can be exercised without a database, in the same way
/// `SecretStore` makes the identity path testable without a keychain.
pub trait PairingStore {
    /// Records a pairing with `peer`, or re-establishes a revoked one.
    ///
    /// Re-pairing **clears the revocation on the existing row** rather than adding another,
    /// per design D4. Revoking withdraws the authority previously granted; it does not
    /// blacklist the device, and a user who revokes a phone after mislaying it must be able
    /// to pair it again when it turns up.
    fn record_pairing(
        &self,
        peer: &DeviceId,
        verifying_key: &VerifyingKey,
        paired_at: DateTime<Utc>,
    ) -> AppResult<()>;

    /// Every pairing, revoked ones included. For showing the user what this device has ever
    /// been paired with.
    fn list_pairings(&self) -> AppResult<Vec<PairingRecord>>;

    /// Withdraws `peer`'s authority, with effect for every command evaluated afterwards.
    ///
    /// A local act: it takes effect without reference to any relay or network, because a
    /// device must be de-authorizable when nothing is reachable — which is exactly when
    /// someone is most likely to be doing it.
    fn revoke_pairing(&self, peer: &DeviceId, revoked_at: DateTime<Utc>) -> AppResult<()>;

    /// The keys commands are actually checked against — active pairings only.
    fn verifying_keys_from_store(&self) -> AppResult<HashMap<DeviceId, VerifyingKey>> {
        Ok(verifying_keys_from_pairings(&self.list_pairings()?))
    }
}

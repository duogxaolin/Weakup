//! The seam a relay sits behind, and what a relay is required to be incapable of.
//!
//! # A transport moves bytes and decides nothing
//!
//! The interface below can register a device, report that it is present, list the account's
//! devices, send an envelope, and receive envelopes addressed here. That is the whole
//! surface. There is deliberately **no member by which an implementation could report that
//! a command is genuine** — no flag, no parameter, no method.
//!
//! This is what makes a hosted relay an acceptable place to run this service rather than a
//! concentration of risk. The guarantee is not that the operator behaves well; it is that
//! the operator's cooperation is not sufficient. A relay can drop a command, delay it,
//! duplicate it, or deliver two out of order. It cannot cause one to be obeyed, because the
//! target checks the signature itself against a key the relay never holds.
//!
//! A source-text test in `transport_tests.rs` fails the build if a member is ever added
//! whose name suggests it carries such a claim. That test lives in its own file rather than
//! in a `mod tests` here, because an inline module's own string literals would match its
//! own grep — a trap this codebase has hit before.
//!
//! # Liveness is data, not a verdict
//!
//! [`RemoteTransport::report_presence`] records an instant and
//! [`RemoteTransport::list_devices`] returns instants. Neither returns "online". Whether a
//! device counts as online is decided from that instant by the presence rule, at whichever
//! device is asking — otherwise two devices could ask the same relay about the same peer
//! and be told different things, and a network-dependent judgement would appear to the user
//! as a fact.
//!
//! # Why account identity is a separate interface
//!
//! [`AuthProvider`] is not part of [`RemoteTransport`]. Account identity decides which
//! devices a user can *see*; pairing decides which they can *command*, and the authorization
//! rules already state that same-account access is not sufficient. One interface carrying
//! both concerns invites an implementation in which a valid session becomes authority —
//! exactly what those rules forbid — and makes it awkward to replace either half alone.

use std::collections::HashMap;
use std::sync::Mutex;

use chrono::{DateTime, Utc};

use crate::core::AppResult;
use crate::domain::{CommandEnvelope, DeviceId};

/// What a transport reports about one device in the account.
///
/// Carries an instant rather than a state. See the module comment: the online-or-not
/// judgement belongs to the presence rule at the asking device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DevicePresenceRecord {
    /// Which device this is.
    pub device_id: DeviceId,
    /// A label the person recognises, for display only. It confers nothing.
    pub display_name: String,
    /// When this device last reported that it was present, if it ever has.
    pub last_reported_at: Option<DateTime<Utc>>,
}

/// The account a device belongs to.
///
/// Deliberately thin. It answers "whose devices should this one be shown", and nothing
/// about what may be commanded.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AccountId(String);

impl AccountId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Carries opaque envelopes between devices in one account, and reports liveness.
///
/// Every method moves data or reports an instant. None of them makes, or reports, a
/// judgement about whether a command should be obeyed — see the module comment for why that
/// absence is the design rather than an omission.
pub trait RemoteTransport {
    /// Makes this device known to the account, so other devices can list it.
    ///
    /// Being listed is not permission to command. Pairing is a separate act.
    fn register_device(&self, device_id: &DeviceId, display_name: &str) -> AppResult<()>;

    /// Records that this device is present, as of `at`.
    fn report_presence(&self, device_id: &DeviceId, at: DateTime<Utc>) -> AppResult<()>;

    /// The account's devices and when each last reported presence.
    fn list_devices(&self) -> AppResult<Vec<DevicePresenceRecord>>;

    /// Hands a signed envelope to the transport for delivery to `target`.
    ///
    /// The transport does not inspect the envelope and has no way to indicate an opinion
    /// about it. Success means the transport accepted it for delivery, not that the target
    /// obeyed it — and certainly not that it should.
    fn send_envelope(&self, target: &DeviceId, envelope: &CommandEnvelope) -> AppResult<()>;

    /// Takes the envelopes waiting for `device_id`, emptying the queue.
    ///
    /// The order is whatever the transport gives, and may repeat an envelope already
    /// delivered. The receiving device judges each one on its own merits, which is what
    /// makes those faults survivable.
    fn receive_envelopes(&self, device_id: &DeviceId) -> AppResult<Vec<CommandEnvelope>>;
}

/// Establishes which account this device belongs to.
///
/// Separate from [`RemoteTransport`] by design (see the module comment): neither may
/// substitute for the other, and replacing one must not require touching the other.
pub trait AuthProvider {
    /// The account this device is currently acting as, if any.
    fn current_account(&self) -> AppResult<Option<AccountId>>;
}

/// The faults a transport is assumed capable of, each switched on per test.
///
/// A fake that only behaves proves the happy path and nothing else. The spec requires the
/// rules survive a transport that misbehaves, and these are the four faults the design
/// depends on surviving: a dropped command is an ordinary mobile network, a delayed one is
/// a backgrounded app, a duplicate is a retry after an ambiguous timeout, and a reorder is
/// two commands racing through different paths.
///
/// Default is well-behaved, so a test that does not opt in reads plainly.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TransportFaults {
    /// Accept envelopes and deliver none of them.
    pub drop_everything: bool,
    /// Hold envelopes until [`FakeTransport::release_delayed`] is called.
    pub delay_delivery: bool,
    /// Deliver every envelope twice.
    pub duplicate_delivery: bool,
    /// Deliver in the reverse of the order sent.
    pub reorder_delivery: bool,
}

/// An in-memory [`RemoteTransport`] that can misbehave on demand.
///
/// This is the honest expression of a property the design claims: a relay may drop, delay,
/// duplicate, and reorder, but cannot forge. The fake does all four; what it has no way to
/// do — because the trait offers no such member — is make a command more likely to be
/// obeyed.
pub struct FakeTransport {
    state: Mutex<FakeTransportState>,
}

#[derive(Default)]
struct FakeTransportState {
    faults: TransportFaults,
    devices: Vec<DevicePresenceRecord>,
    /// Envelopes ready to be received, per target device.
    queued: HashMap<DeviceId, Vec<CommandEnvelope>>,
    /// Envelopes held back while `delay_delivery` is set.
    held: HashMap<DeviceId, Vec<CommandEnvelope>>,
}

impl FakeTransport {
    /// A well-behaved transport: nothing is dropped, delayed, duplicated, or reordered.
    pub fn new() -> Self {
        Self {
            state: Mutex::new(FakeTransportState::default()),
        }
    }

    /// A transport with the given faults switched on.
    pub fn with_faults(faults: TransportFaults) -> Self {
        Self {
            state: Mutex::new(FakeTransportState {
                faults,
                ..FakeTransportState::default()
            }),
        }
    }

    /// Releases everything held back by `delay_delivery`, as a late delivery would.
    ///
    /// The command's own creation instant is unchanged by the wait, which is exactly how a
    /// real delayed delivery behaves and why the freshness rule catches it.
    pub fn release_delayed(&self) {
        let mut state = self.state.lock().expect("fake transport lock");
        let held: Vec<(DeviceId, Vec<CommandEnvelope>)> = state.held.drain().collect();
        for (target, envelopes) in held {
            state.queued.entry(target).or_default().extend(envelopes);
        }
    }

    /// How many envelopes are waiting for `device_id`, for a test asserting a drop.
    pub fn queued_count(&self, device_id: &DeviceId) -> usize {
        let state = self.state.lock().expect("fake transport lock");
        state.queued.get(device_id).map_or(0, Vec::len)
    }
}

impl Default for FakeTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl RemoteTransport for FakeTransport {
    fn register_device(&self, device_id: &DeviceId, display_name: &str) -> AppResult<()> {
        let mut state = self.state.lock().expect("fake transport lock");
        if let Some(existing) = state
            .devices
            .iter_mut()
            .find(|record| &record.device_id == device_id)
        {
            existing.display_name = display_name.to_string();
        } else {
            state.devices.push(DevicePresenceRecord {
                device_id: device_id.clone(),
                display_name: display_name.to_string(),
                last_reported_at: None,
            });
        }
        Ok(())
    }

    fn report_presence(&self, device_id: &DeviceId, at: DateTime<Utc>) -> AppResult<()> {
        let mut state = self.state.lock().expect("fake transport lock");
        if let Some(existing) = state
            .devices
            .iter_mut()
            .find(|record| &record.device_id == device_id)
        {
            existing.last_reported_at = Some(at);
        } else {
            state.devices.push(DevicePresenceRecord {
                device_id: device_id.clone(),
                display_name: device_id.as_str().to_string(),
                last_reported_at: Some(at),
            });
        }
        Ok(())
    }

    fn list_devices(&self) -> AppResult<Vec<DevicePresenceRecord>> {
        let state = self.state.lock().expect("fake transport lock");
        Ok(state.devices.clone())
    }

    fn send_envelope(&self, target: &DeviceId, envelope: &CommandEnvelope) -> AppResult<()> {
        let mut state = self.state.lock().expect("fake transport lock");
        let faults = state.faults;

        if faults.drop_everything {
            // Accepted and discarded, exactly as a relay losing a message would behave. The
            // sender is told nothing is wrong, which is the point: it cannot tell.
            return Ok(());
        }

        let destination = if faults.delay_delivery {
            state.held.entry(target.clone()).or_default()
        } else {
            state.queued.entry(target.clone()).or_default()
        };

        destination.push(envelope.clone());
        if faults.duplicate_delivery {
            destination.push(envelope.clone());
        }

        Ok(())
    }

    fn receive_envelopes(&self, device_id: &DeviceId) -> AppResult<Vec<CommandEnvelope>> {
        let mut state = self.state.lock().expect("fake transport lock");
        let reorder = state.faults.reorder_delivery;

        let mut envelopes = state.queued.remove(device_id).unwrap_or_default();
        if reorder {
            envelopes.reverse();
        }

        Ok(envelopes)
    }
}

/// An [`AuthProvider`] that returns a fixed account.
///
/// There is no OAuth and no real identity behind this. It exists so that the seam is
/// exercised and so the transport can be written against an interface rather than against
/// a sign-in library.
pub struct FakeAuthProvider {
    account: Option<AccountId>,
}

impl FakeAuthProvider {
    /// A provider reporting the given account.
    pub fn new(account: AccountId) -> Self {
        Self {
            account: Some(account),
        }
    }

    /// A provider reporting no account, as a signed-out device would.
    pub fn signed_out() -> Self {
        Self { account: None }
    }
}

impl AuthProvider for FakeAuthProvider {
    fn current_account(&self) -> AppResult<Option<AccountId>> {
        Ok(self.account.clone())
    }
}

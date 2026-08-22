//! The remote surface's state: identity, pairings, outstanding codes, and the seams.
//!
//! Held apart from [`AppState`](crate::commands::state::AppState) because the remote
//! feature is optional in a way the scheduler is not. A machine with no keychain, or one
//! whose identity could not be generated, still schedules and still counts down; it simply
//! cannot pair. Folding these into `AppState` would have meant either making the whole app
//! fail on a locked credential store or scattering `Option` checks through the job commands,
//! which never needed them.
//!
//! # What is deliberately absent
//!
//! No [`PowerOffExecutor`](crate::platform::power_off::PowerOffExecutor), and no route to
//! one. The remote surface creates *jobs*; the countdown gate holds the only executor and
//! takes no duration argument. That is what makes the 300-second remote countdown
//! non-bypassable rather than merely usual — see `grace_period.rs`.
//!
//! # Why the issuer is behind a mutex and the store is not
//!
//! [`PairingCodeIssuer`] is mutated by issuing and redeeming, so it needs exclusive access.
//! The pairing store takes `&self` and serialises internally at the SQLite connection, so a
//! second lock here would add a way to deadlock without adding a guarantee.

use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};

use crate::application::pairing_flow::PairingCodeIssuer;
use crate::application::transport::{AuthProvider, RemoteTransport};
use crate::core::{AppError, AppResult};
use crate::data::PairingStore;
use crate::domain::DeviceIdentity;

/// How often stale pairing codes are swept from the issuer.
///
/// Sweeping is driven by *use* rather than by a timer: every command that touches the
/// issuer sweeps first, and this is the minimum gap between two sweeps so that a burst of
/// calls does not walk the map repeatedly.
///
/// # Why this exists at all
///
/// [`PairingCodeIssuer`] holds outstanding codes in memory and nothing called
/// [`PairingCodeIssuer::forget_stale`] until this module did. Left unswept the map grows
/// for the life of the process — which for this app is weeks, since it lives in the tray
/// and is not quit. Small per entry, unbounded over time.
///
/// The sweep is placed here, at the surface, rather than inside the issuer, because the
/// issuer has no clock: `forget_stale` takes `now` precisely so the decision about *when*
/// belongs to a caller that has one.
const CODE_SWEEP_INTERVAL_SECONDS: i64 = 60;

/// The sweep must not run so rarely that a burst of codes accumulates between sweeps, nor
/// so often that it walks the map on every keystroke.
const _: () = assert!(
    CODE_SWEEP_INTERVAL_SECONDS > 0 && CODE_SWEEP_INTERVAL_SECONDS <= 300,
    "a sweep interval outside this range either lets the outstanding-code map grow \
     unbounded or walks it needlessly often"
);

/// Everything the pairing and device commands need, when the feature is available.
pub struct RemoteSurface {
    /// This device's signing identity. Its private half never leaves it.
    identity: Arc<DeviceIdentity>,
    /// A display name for this device, shown to peers. Confers nothing.
    display_name: String,
    /// Where pairings are recorded and revoked.
    pairings: Arc<dyn PairingStore + Send + Sync>,
    /// Outstanding pairing codes, and the sweep state that keeps the map bounded.
    issuer: Mutex<IssuerState>,
    /// The relay, when one is configured. Absent means "no network side"; listing devices
    /// then reports nothing rather than pretending.
    transport: Option<Arc<dyn RemoteTransport + Send + Sync>>,
    /// The account seam. Separate from the transport by design — see `transport.rs`.
    auth: Arc<dyn AuthProvider + Send + Sync>,
    /// Forgets the account session.
    ///
    /// A closure rather than a method on [`AuthProvider`], and that is deliberate. The
    /// trait answers "which account is this device acting as" and nothing more; widening it
    /// with a mutating member would put session *management* behind the same interface the
    /// transport is written against, and the whole point of keeping the two apart is that a
    /// valid session must never become authority.
    ///
    /// The closure captures the concrete provider, so signing out reaches exactly the
    /// credential-store entries that provider owns — and nothing else. There is no pairing
    /// store in scope for it to touch, which is what makes "signing out leaves pairings
    /// intact" true by construction rather than by remembering.
    sign_out: Box<dyn Fn() -> AppResult<()> + Send + Sync>,
}

/// The issuer alongside when it was last swept.
struct IssuerState {
    issuer: PairingCodeIssuer,
    last_swept_at: Option<DateTime<Utc>>,
}

impl RemoteSurface {
    pub fn new(
        identity: Arc<DeviceIdentity>,
        display_name: impl Into<String>,
        pairings: Arc<dyn PairingStore + Send + Sync>,
        transport: Option<Arc<dyn RemoteTransport + Send + Sync>>,
        auth: Arc<dyn AuthProvider + Send + Sync>,
        sign_out: Box<dyn Fn() -> AppResult<()> + Send + Sync>,
    ) -> Self {
        Self {
            identity,
            display_name: display_name.into(),
            pairings,
            issuer: Mutex::new(IssuerState {
                issuer: PairingCodeIssuer::new(),
                last_swept_at: None,
            }),
            transport,
            auth,
            sign_out,
        }
    }

    pub fn identity(&self) -> &Arc<DeviceIdentity> {
        &self.identity
    }

    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    pub fn pairings(&self) -> &Arc<dyn PairingStore + Send + Sync> {
        &self.pairings
    }

    pub fn transport(&self) -> Option<&Arc<dyn RemoteTransport + Send + Sync>> {
        self.transport.as_ref()
    }

    pub fn auth(&self) -> &Arc<dyn AuthProvider + Send + Sync> {
        &self.auth
    }

    /// Forgets the account session, touching no pairing.
    pub fn sign_out(&self) -> AppResult<()> {
        (self.sign_out)()
    }

    /// Runs `body` against the issuer, sweeping stale codes first if enough time has passed.
    ///
    /// The sweep happens *before* `body` rather than after, which is the ordering that
    /// matters: sweeping afterwards could discard the code `body` had just issued if the
    /// clock had moved, and a code the user is reading off the screen must not vanish.
    ///
    /// [`PairingCodeIssuer::forget_stale`] keeps a redeemed code for a second lifetime past
    /// expiry on purpose, so this sweep never turns an `AlreadyUsed` report into a
    /// `NoSuchGrant` one — those tell the person two different things.
    pub fn with_issuer<T>(
        &self,
        now: DateTime<Utc>,
        body: impl FnOnce(&mut PairingCodeIssuer) -> T,
    ) -> AppResult<T> {
        let mut state = self.issuer.lock().map_err(|_| AppError::Storage {
            message: "the pairing code issuer lock was poisoned".to_string(),
        })?;

        let due = match state.last_swept_at {
            None => true,
            Some(last) => (now - last).num_seconds() >= CODE_SWEEP_INTERVAL_SECONDS,
        };
        if due {
            state.issuer.forget_stale(now);
            state.last_swept_at = Some(now);
        }

        Ok(body(&mut state.issuer))
    }

    /// How many codes are outstanding, for a surface that tells the user one is live.
    pub fn outstanding_code_count(&self, now: DateTime<Utc>) -> AppResult<usize> {
        self.with_issuer(now, |issuer| issuer.outstanding_count())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::transport::{AccountId, FakeAuthProvider, FakeTransport};
    use crate::data::PairingRecord;
    use crate::domain::{DeviceId, GrantDelivery, VerifyingKey};
    use std::collections::HashMap;

    /// A store that records nothing, for tests about the issuer alone.
    struct EmptyStore;

    impl PairingStore for EmptyStore {
        fn record_pairing(
            &self,
            _peer: &DeviceId,
            _key: &VerifyingKey,
            _at: DateTime<Utc>,
        ) -> AppResult<()> {
            Ok(())
        }

        fn list_pairings(&self) -> AppResult<Vec<PairingRecord>> {
            Ok(Vec::new())
        }

        fn revoke_pairing(&self, _peer: &DeviceId, _at: DateTime<Utc>) -> AppResult<()> {
            Ok(())
        }

        fn verifying_keys_from_store(&self) -> AppResult<HashMap<DeviceId, VerifyingKey>> {
            Ok(HashMap::new())
        }
    }

    fn surface() -> RemoteSurface {
        let identity = Arc::new(
            DeviceIdentity::from_signing_key_bytes(&[7u8; 32]).expect("a valid seed"),
        );
        RemoteSurface::new(
            identity,
            "Test desktop",
            Arc::new(EmptyStore),
            Some(Arc::new(FakeTransport::new())),
            Arc::new(FakeAuthProvider::new(AccountId::new("someone@example.com"))),
            Box::new(|| Ok(())),
        )
    }

    fn at(seconds: i64) -> DateTime<Utc> {
        use chrono::TimeZone;
        Utc.with_ymd_and_hms(2026, 8, 6, 12, 0, 0).unwrap() + chrono::Duration::seconds(seconds)
    }

    #[test]
    fn an_outstanding_code_is_swept_once_it_can_no_longer_be_reported_on() {
        // The loose end this module exists to close: nothing called `forget_stale`, so the
        // outstanding map grew for the life of the process. This app lives in the tray for
        // weeks, so "grows slowly" means "grows".
        let surface = surface();

        surface
            .with_issuer(at(0), |issuer| {
                issuer.issue(GrantDelivery::AtMachine, at(0)).expect("issued")
            })
            .expect("the lock is not poisoned");

        assert_eq!(surface.outstanding_code_count(at(0)).unwrap(), 1);

        // A code shown at the machine lives 300 seconds and is retained for a second
        // lifetime past that so `Expired` stays reportable. At 601 seconds both are past.
        assert_eq!(
            surface.outstanding_code_count(at(601)).unwrap(),
            0,
            "the issuer was never swept, so the map grows without bound"
        );
    }

    #[test]
    fn a_live_code_is_never_swept_out_from_under_the_person_reading_it() {
        // The failure mode a careless sweep introduces: the user is looking at a code on
        // screen and it stops working. Worse than not sweeping at all, because it is
        // intermittent.
        let surface = surface();

        let code = surface
            .with_issuer(at(0), |issuer| {
                issuer.issue(GrantDelivery::AtMachine, at(0)).expect("issued")
            })
            .expect("lock");

        // Well past the sweep interval, well inside the grant's own lifetime.
        let validity = surface
            .with_issuer(at(120), |issuer| issuer.redeem(&code, at(120)))
            .expect("lock");

        assert!(
            validity.is_valid(),
            "a sweep discarded a code that was still within its lifetime"
        );
    }

    #[test]
    fn a_redeemed_code_stays_reportable_as_used_rather_than_unrecognised() {
        // `forget_stale` retains a spent code for a second lifetime deliberately. If the
        // sweep dropped it at redemption, a second presentation would report "no such
        // code" — which reads to the user as a typo rather than as a spent code, and they
        // would retype it until the grant really expired.
        let surface = surface();

        let code = surface
            .with_issuer(at(0), |issuer| {
                issuer.issue(GrantDelivery::AtMachine, at(0)).expect("issued")
            })
            .expect("lock");

        surface
            .with_issuer(at(10), |issuer| issuer.redeem(&code, at(10)))
            .expect("lock");

        let second = surface
            .with_issuer(at(120), |issuer| issuer.redeem(&code, at(120)))
            .expect("lock");

        assert_eq!(
            second,
            crate::domain::GrantValidity::Invalid(crate::domain::GrantRejection::AlreadyUsed),
            "a spent code must report as spent, not as unrecognised"
        );
    }
}

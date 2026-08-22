//! The pairing exchange: presenting a code at one device and redeeming it at the other.
//!
//! Pairing is the only act that confers authority to command a device. The rules that decide
//! whether a grant may still be redeemed already exist in
//! [`evaluate_grant`](crate::domain::evaluate_grant) and are pinned by shared vectors. This
//! module adds the two things that were missing around them: a code a person can read off one
//! screen and type into another, and the exchange that turns a redeemed code into a pairing
//! recorded at both ends.
//!
//! # What this module does *not* decide
//!
//! Expiry and single use are not reimplemented here. [`PairingCodeIssuer::redeem`] builds a
//! [`PairingGrant`] from what it knows — whether it issued this code, whether it has already
//! been redeemed, when it was issued — and hands the decision to `evaluate_grant`. A second
//! implementation of those rules would be a second source of truth, and the one that drifted
//! would be whichever was not vector-verified.
//!
//! # Why the alphabet excludes `0`/`O` and `1`/`I`/`l` (design D6)
//!
//! Not cosmetic. A user who mistypes a code gets [`GrantRejection::NoSuchGrant`], which reads
//! to them as indistinguishable from an expired code — so they retry, and keep retrying, until
//! the grant really has expired. Removing the glyphs that are routinely confused removes that
//! failure rather than documenting it. See [`PAIRING_CODE_ALPHABET`].
//!
//! Six characters from a 31-glyph alphabet is about 29.7 bits (31^6 ≈ 8.9e8). That is weak in
//! isolation and is
//! adequate here only because of three properties that hold together:
//!
//! - the grant expires — 300 seconds when shown at the machine;
//! - it can be redeemed once, after which the code is spent;
//! - it is accepted only by the device that issued it, which holds it in memory and never
//!   sends it anywhere.
//!
//! Weakening any one of those three makes the code length wrong, and the right response would
//! be to lengthen the code rather than to accept the loss quietly.
//!
//! # Both sides or neither (design D7)
//!
//! [`complete_pairing_at_requester`] records the issuer **only after** the issuer has confirmed
//! it recorded the requester, and if its own write then fails it withdraws the issuer's record
//! too. A half-recorded pairing is the worst outcome available: the requesting device believes
//! it is authorized, sends commands, and every one is refused for a reason the user cannot see
//! anywhere in either interface. An outright failure is strictly better, because it tells the
//! user to try again.
//!
//! The withdrawal is a revocation rather than a deletion, because that is what the store
//! offers and the reason is the same one design D4 gives: the record that an exchange was
//! attempted is worth keeping, and a revoked pairing confers nothing — its key is absent from
//! the map [`evaluate_command`](crate::domain::evaluate_command) checks against.
//!
//! The Dart implementation in `mobile/lib/application/pairing_flow.dart` mirrors this file.

use std::collections::HashMap;

use chrono::{DateTime, Utc};

use crate::core::{AppError, AppResult};
use crate::data::PairingStore;
use crate::domain::{
    evaluate_grant, DeviceId, GrantDelivery, GrantRejection, GrantValidity, PairingGrant,
    VerifyingKey,
};

// ---------------------------------------------------------------------------
// The code itself
// ---------------------------------------------------------------------------

/// The glyphs a pairing code may contain, per design D6.
///
/// The 36 alphanumerics of the Latin alphabet less the four that are routinely misread:
/// `0`/`O` and `1`/`I`/`L`. Uppercase only, so a code read off a screen and typed in any case
/// normalises to exactly one string.
///
/// Counted rather than assumed: 26 letters + 10 digits = 36, less `O`, `I`, `L`, `0`, `1` = 31.
/// The remaining glyph count is asserted below, because an edit that drops one by accident
/// would silently shrink the space.
pub const PAIRING_CODE_ALPHABET: &[u8] = b"23456789ABCDEFGHJKMNPQRSTUVWXYZ";

/// How many characters a pairing code has. See the module comment for why six is enough only
/// alongside expiry, single use, and issuer-only acceptance.
pub const PAIRING_CODE_LENGTH: usize = 6;

/// The confusable glyphs that must never appear in a code, named so the reason survives.
///
/// `l` is lowercase deliberately: the alphabet is uppercase, so the glyph a user confuses with
/// `1` is the lowercase `l` they might type. Its uppercase form `L` is what is excluded from
/// the alphabet, and both are checked.
pub const CONFUSABLE_GLYPHS: &[char] = &['0', 'O', '1', 'I', 'l', 'L'];

/// The alphabet must not contain a glyph the exclusion list names.
///
/// Checked at compile time so a contributor who "restores" the full alphanumeric set for
/// entropy gets a build error rather than a subtle usability regression. `assert!` on a
/// constant is what clippy rejects, hence the `const _` form used elsewhere in this crate.
const _: () = {
    let alphabet = PAIRING_CODE_ALPHABET;
    let mut i = 0;
    while i < alphabet.len() {
        let byte = alphabet[i];
        assert!(
            byte != b'0' && byte != b'O' && byte != b'1' && byte != b'I' && byte != b'L',
            "the pairing alphabet contains a glyph users confuse with another; a mistyped \
             code is refused as unrecognised, which a user cannot tell apart from expiry"
        );
        i += 1;
    }
};

/// A code short enough to type but long enough to matter, and an alphabet large enough for
/// the arithmetic in the module comment to hold.
const _: () = assert!(
    PAIRING_CODE_LENGTH >= 6 && PAIRING_CODE_ALPHABET.len() >= 31,
    "six characters from a 31-glyph alphabet is the space the design reasoned about; \
     shortening either invalidates that reasoning"
);

/// A pairing code, as shown at the issuing device and typed at the other.
///
/// A newtype rather than a bare `String` so that a code and a device id cannot be passed in
/// each other's place, and so normalisation happens in exactly one place.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PairingCode(String);

impl PairingCode {
    /// Accepts a code a person typed, normalising case and ignoring spacing.
    ///
    /// Normalisation is deliberately generous about everything that does not change which
    /// code was meant — case, surrounding whitespace, and the internal spaces or hyphens a
    /// user adds when transcribing in groups. It is deliberately strict about glyphs outside
    /// the alphabet: those are refused here rather than being looked up and reported as
    /// unrecognised, so a genuinely mistyped character does not consume the attempt.
    ///
    /// Confusable glyphs are **not** silently corrected. Mapping `O` to `0` would be guessing
    /// at intent, and a code that is accepted as something other than what was typed is
    /// harder to reason about than one that is refused.
    pub fn parse(entered: &str) -> AppResult<Self> {
        let normalised: String = entered
            .chars()
            .filter(|c| !c.is_whitespace() && *c != '-')
            .flat_map(char::to_uppercase)
            .collect();

        if normalised.chars().count() != PAIRING_CODE_LENGTH {
            return Err(AppError::validation(format!(
                "A pairing code is {PAIRING_CODE_LENGTH} characters long."
            )));
        }

        if let Some(bad) = normalised
            .chars()
            .find(|c| !PAIRING_CODE_ALPHABET.contains(&(*c as u8)))
        {
            // Named rather than generic: "that is not a character a code contains" is
            // actionable, and it is a different situation from a code that was not issued.
            return Err(AppError::validation(format!(
                "A pairing code does not contain '{bad}'. Check the code shown on the other \
                 device and try again."
            )));
        }

        Ok(Self(normalised))
    }

    /// The code as it should be displayed and compared.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for PairingCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Draws a fresh code from the system CSPRNG.
///
/// # Why rejection sampling rather than `% alphabet.len()`
///
/// 256 is not a multiple of 31, so taking a random byte modulo the alphabet length would make
/// the first few glyphs measurably more likely than the rest — a bias that shrinks the
/// effective space of a code that has only ~33 bits to begin with. Bytes outside the largest
/// usable multiple are discarded instead.
///
/// A failure to read entropy is an error, never a fallback to something predictable. A code
/// drawn from a degraded source is one an attacker can reproduce, and the grant it guards is
/// what authorises commanding this machine.
pub fn generate_pairing_code() -> AppResult<PairingCode> {
    let alphabet_len = PAIRING_CODE_ALPHABET.len();
    // The largest multiple of the alphabet length that fits in a byte. Bytes at or above this
    // are rejected rather than folded, which is what keeps the distribution uniform.
    let ceiling = (256 / alphabet_len) * alphabet_len;

    let mut code = String::with_capacity(PAIRING_CODE_LENGTH);
    while code.len() < PAIRING_CODE_LENGTH {
        let mut byte = [0u8; 1];
        getrandom::fill(&mut byte).map_err(|e| AppError::Storage {
            message: format!("could not read system entropy to generate a pairing code: {e}"),
        })?;

        let value = byte[0] as usize;
        if value >= ceiling {
            continue;
        }
        code.push(PAIRING_CODE_ALPHABET[value % alphabet_len] as char);
    }

    Ok(PairingCode(code))
}

// ---------------------------------------------------------------------------
// The issuing side
// ---------------------------------------------------------------------------

/// What this device tells a peer about itself when pairing.
///
/// Public material only: an identifier and the verifying key peers record. There is no private
/// half here and no route to one — the identity that signs holds its key and never hands it
/// over.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairingIdentity {
    pub device_id: DeviceId,
    pub verifying_key: VerifyingKey,
}

/// One outstanding code and the grant state `evaluate_grant` needs to judge it.
#[derive(Debug, Clone)]
struct OutstandingGrant {
    delivery: GrantDelivery,
    issued_at: DateTime<Utc>,
    redeemed: bool,
}

/// Holds the codes this device has issued and not yet seen redeemed.
///
/// In memory rather than in the database, and that is the decision. A code is meaningful only
/// while the person is standing at the machine, its lifetime is 300 seconds, and a restart in
/// the middle of pairing is a restart the user will notice and repeat. Persisting it would
/// keep a redeemable secret on disk to no benefit.
///
/// It is also what makes "accepted only by the device that issued it" true rather than
/// aspirational: the code is never sent to a relay, so no other device can be asked about it.
#[derive(Debug, Default)]
pub struct PairingCodeIssuer {
    outstanding: HashMap<PairingCode, OutstandingGrant>,
}

impl PairingCodeIssuer {
    pub fn new() -> Self {
        Self {
            outstanding: HashMap::new(),
        }
    }

    /// Issues a code to show the person, valid for its delivery path's lifetime from `now`.
    ///
    /// The lifetime is not stored: it is derived from `delivery` by the domain rule, so this
    /// side has no opportunity to disagree with the rule about when a code dies.
    pub fn issue(
        &mut self,
        delivery: GrantDelivery,
        now: DateTime<Utc>,
    ) -> AppResult<PairingCode> {
        let code = generate_pairing_code()?;
        self.outstanding.insert(
            code.clone(),
            OutstandingGrant {
                delivery,
                issued_at: now,
                redeemed: false,
            },
        );
        Ok(code)
    }

    /// Judges a code presented back, and marks it spent if it was good.
    ///
    /// The verdict comes from [`evaluate_grant`]; this function's job is to describe the
    /// grant honestly — including `recognised: false` for a code it never issued, which is
    /// what keeps a mistyped code distinguishable from an expired one.
    ///
    /// Marking spent happens **here**, at the moment the code is accepted, rather than after
    /// the pairing is recorded. A code that could be redeemed twice while a first exchange
    /// was still in flight would not be single use, and single use is what makes an observed
    /// code worthless afterwards.
    pub fn redeem(&mut self, code: &PairingCode, now: DateTime<Utc>) -> GrantValidity {
        let Some(state) = self.outstanding.get(code) else {
            // Never issued, or issued by some other device. Either way this device does not
            // stand behind any `issued_at` for it, so no age is computed — which is exactly
            // the ordering `evaluate_grant` pins.
            return evaluate_grant(
                &PairingGrant {
                    delivery: GrantDelivery::AtMachine,
                    issued_at: now,
                    redeemed: false,
                    recognised: false,
                },
                now,
            );
        };

        let validity = evaluate_grant(
            &PairingGrant {
                delivery: state.delivery,
                issued_at: state.issued_at,
                redeemed: state.redeemed,
                recognised: true,
            },
            now,
        );

        if validity.is_valid() {
            if let Some(state) = self.outstanding.get_mut(code) {
                state.redeemed = true;
            }
        }

        validity
    }

    /// Forgets an expired or spent code, so the map does not grow without bound.
    ///
    /// Retaining a redeemed code until it is swept is deliberate: dropping it the instant it
    /// was used would make a second presentation report `NoSuchGrant` rather than
    /// `AlreadyUsed`, and those tell the person two different things. One means check the
    /// code; the other means the code is spent and a new one is needed.
    pub fn forget_stale(&mut self, now: DateTime<Utc>) {
        self.outstanding.retain(|_, state| {
            let age = (now - state.issued_at).num_seconds();
            // Kept for a second lifetime past expiry so `AlreadyUsed` and `Expired` remain
            // reportable for a while after the fact.
            age <= state.delivery.lifetime_seconds() * 2
        });
    }

    /// How many codes are outstanding. For tests and for a surface that shows the user
    /// whether a code is currently live.
    pub fn outstanding_count(&self) -> usize {
        self.outstanding.len()
    }
}

// ---------------------------------------------------------------------------
// The exchange
// ---------------------------------------------------------------------------

/// What happened to a pairing attempt.
///
/// A refusal names the grant rejection it came from, so the surface can show the person the
/// prose the domain already wrote rather than inventing its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PairingOutcome {
    /// Both devices recorded each other.
    Paired {
        /// The peer now recorded here.
        peer: DeviceId,
    },
    /// The code was refused. Nothing was recorded at either device.
    Refused(GrantRejection),
}

impl PairingOutcome {
    pub fn is_paired(&self) -> bool {
        matches!(self, Self::Paired { .. })
    }

    /// The prose to show the person, taken from the domain's own messages for a refusal.
    pub fn user_message(&self) -> String {
        match self {
            Self::Paired { peer } => format!("Paired with {peer}."),
            Self::Refused(reason) => reason.user_message(),
        }
    }
}

/// The issuing device's half: judge the code, and record the requester if it was good.
///
/// Returns this device's own public identity on success, which is what the requester needs in
/// order to record the other direction — and, crucially, is also the *confirmation* that this
/// side has recorded the requester. There is no way to obtain the identity without the record
/// having been written, so the requester cannot record a peer that has not reciprocated.
///
/// If the store write fails, this returns the error and records nothing. The requester never
/// receives an identity and so never records anything either.
pub fn accept_pairing_at_issuer(
    issuer: &mut PairingCodeIssuer,
    store: &dyn PairingStore,
    own_identity: &PairingIdentity,
    requester: &PairingIdentity,
    code: &PairingCode,
    now: DateTime<Utc>,
) -> AppResult<PairingResponse> {
    match issuer.redeem(code, now) {
        GrantValidity::Invalid(reason) => Ok(PairingResponse::Refused(reason)),
        GrantValidity::Valid => {
            // The requester's key is recorded from what the requester presented in person,
            // during an exchange the owner started at this machine. It is never taken from a
            // relay and never from a key travelling alongside a command.
            store.record_pairing(&requester.device_id, &requester.verifying_key, now)?;

            Ok(PairingResponse::Accepted {
                issuer_identity: Box::new(own_identity.clone()),
            })
        }
    }
}

/// What the issuing device sends back.
///
/// `Accepted` carrying the issuer's identity is the confirmation design D7 requires: it exists
/// only if the issuer's own record was written, so the requester recording on receipt of it
/// cannot produce a one-sided pairing in the issuer-failed direction.
///
/// The identity is boxed because a [`VerifyingKey`] is substantially larger than a rejection
/// reason, and an unboxed enum would carry the larger size everywhere including in the refusal
/// case that needs one byte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PairingResponse {
    Accepted {
        issuer_identity: Box<PairingIdentity>,
    },
    Refused(GrantRejection),
}

/// The requesting device's half: present the code, then record the issuer only once the
/// issuer has confirmed it recorded us.
///
/// # The failure this function exists to prevent
///
/// Design D7. The dangerous ordering is the tempting one — record the peer optimistically,
/// then tell it about us — because it leaves this device believing it is authorized while the
/// peer has no record of it. Every command it then sends is refused as coming from an unknown
/// sender, and there is nowhere in either interface where the user could see why.
///
/// So the order here is fixed: the issuer records first and says so, and only then does this
/// side write. And if this side's write fails after the issuer's succeeded, the issuer's
/// record is **withdrawn** by `undo` before the error is returned, because the other one-sided
/// state is just as bad in the opposite direction.
///
/// `undo` is a closure rather than a direct store call because the issuer's store is not
/// reachable from here in the real system — it is on the other device, behind whatever channel
/// carried the exchange. Making the caller supply it keeps this function honest about the fact
/// that the compensating step is a real round trip that can itself fail.
pub fn complete_pairing_at_requester<F>(
    store: &dyn PairingStore,
    response: PairingResponse,
    now: DateTime<Utc>,
    undo: F,
) -> AppResult<PairingOutcome>
where
    F: FnOnce() -> AppResult<()>,
{
    let issuer_identity = match response {
        PairingResponse::Refused(reason) => {
            // Nothing was recorded at the issuer, so there is nothing to undo and nothing to
            // write here. Both sides hold no pairing, which is the correct both-or-neither
            // outcome for a refusal.
            return Ok(PairingOutcome::Refused(reason));
        }
        PairingResponse::Accepted { issuer_identity } => issuer_identity,
    };

    match store.record_pairing(&issuer_identity.device_id, &issuer_identity.verifying_key, now) {
        Ok(()) => Ok(PairingOutcome::Paired {
            peer: issuer_identity.device_id,
        }),
        Err(local_failure) => {
            // The issuer has recorded us and we cannot record it. Withdraw its record rather
            // than leaving it believing a pairing exists that this device knows nothing about.
            match undo() {
                Ok(()) => Err(local_failure),
                Err(undo_failure) => {
                    // Both the write and its compensation failed. This is the one state that
                    // cannot be repaired from here, so it is reported in full rather than
                    // flattened into either error alone — the user needs to know a stale
                    // record may exist at the other device and can be revoked there.
                    Err(AppError::Storage {
                        message: format!(
                            "pairing could not be recorded on this device ({}), and withdrawing \
                             the record the other device had already made also failed ({}). \
                             The other device may still list this one as paired; revoke it there.",
                            local_failure.user_message(),
                            undo_failure.user_message()
                        ),
                    })
                }
            }
        }
    }
}

/// Withdraws a pairing recorded during an exchange that then failed.
///
/// A revocation rather than a deletion, for the reason design D4 gives: the store never
/// deletes, and a revoked pairing's key is absent from the map commands are checked against,
/// so the peer confers nothing. "Neither side believes it is paired" is satisfied by absence
/// of authority, which is what a revoked row is.
pub fn withdraw_pairing(
    store: &dyn PairingStore,
    peer: &DeviceId,
    at: DateTime<Utc>,
) -> AppResult<()> {
    store.revoke_pairing(peer, at)
}

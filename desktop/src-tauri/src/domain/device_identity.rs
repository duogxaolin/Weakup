//! This device's own signing identity: one keypair, generated once, private half sealed.
//!
//! Every other module in `domain/` answers a question about a command that arrived. This one
//! answers "who am I" — and holds the only thing in the app that can produce a signature
//! rather than check one.
//!
//! # The private key cannot be read back, by construction
//!
//! [`DeviceIdentity`] exposes exactly three things: [`device_id`], [`verifying_key`], and
//! [`sign`]. There is no `private_key()`, no `export()`, no `Serialize`, and the [`Debug`]
//! implementation is **written by hand** to print `<sealed>` rather than derived.
//!
//! This is not defensive habit. A key that can be read will eventually be read — into a log
//! line, an error message, a crash report, or a debugging aid added at 2am by someone trying
//! to work out why pairing failed. Making it unreachable removes the class of accident
//! rather than relying on every future contributor to avoid it, which is the same argument
//! `PowerOffGate` makes by holding the only executor and taking no duration parameter.
//!
//! `device_identity_tests.rs` greps this file's source text and fails the build if any of
//! those members appear. It lives in a separate file because an inline `mod tests` would
//! match its own string literals — a trap this codebase has hit before.
//!
//! # Never regenerate on failure
//!
//! [`load_or_generate`] generates a keypair only when the database holds no identity record.
//! If the record says an identity exists but the secret store cannot produce the key, that is
//! an **error**, not a reason to mint a new one.
//!
//! The distinction matters more than it looks. A regenerated identity has a different
//! [`DeviceId`], so every peer that paired with this device would keep trusting a key it no
//! longer holds: its commands would be refused as coming from an unknown sender, and every
//! pairing would have to be redone by hand at each peer. A locked keychain is recoverable and
//! usually temporary. A silently replaced identity is neither. Failing loudly is the better
//! of two bad outcomes, which is why `SecretStore` distinguishes "not found" from
//! "unavailable" at all.
//!
//! The Dart implementation in `mobile/lib/domain/device_identity.dart` mirrors this file, and
//! `shared/testvectors/device_id_derivation.json` pins the identifier both sides derive.
//!
//! [`device_id`]: DeviceIdentity::device_id
//! [`verifying_key`]: DeviceIdentity::verifying_key
//! [`sign`]: DeviceIdentity::sign

use ed25519_dalek::{Signer, SigningKey};
use sha2::{Digest, Sha256};

use crate::core::{AppError, AppResult};
use crate::domain::device_id::DeviceId;
use crate::domain::signature::VerifyingKey;

/// How many bytes of the digest become the identifier.
///
/// Truncated from SHA-256's 32 because the full digest is a 64-character string that will end
/// up in a UI. 128 bits is far beyond what a collision attack on a personal device set
/// requires — and a collision would have to be found against one *specific* paired key, not
/// merely between any two keys.
pub const DEVICE_ID_BYTES: usize = 16;

/// The name the private key is filed under in the platform's secret store.
pub const IDENTITY_SECRET_NAME: &str = "device-identity-signing-key";

/// The length of an Ed25519 private key seed, in bytes.
const SIGNING_KEY_BYTES: usize = 32;

/// A device's identifier, derived from its verifying key.
///
/// The first [`DEVICE_ID_BYTES`] bytes of SHA-256 over the 32 raw verifying-key bytes,
/// lowercase hex. The digest is taken over the key bytes themselves — not over their hex
/// form, and not over any prefix.
///
/// # Why derived rather than assigned
///
/// An independently assigned identifier can be claimed by anyone who learns it: an attacker
/// presents a key of their own under a paired device's name, and the target — which looks
/// keys up *by* name — would have to already know better to refuse it. Deriving the
/// identifier from the key makes the claim self-certifying. To use an identifier, a device
/// must hold the key it was derived from; nothing has to be trusted to enforce that, because
/// a mismatched pair simply does not hash to the claimed value.
///
/// Both implementations must agree byte for byte, or two paired devices would compute
/// different identifiers for the same key and be strangers to each other. That is pinned by
/// vectors rather than by parallel unit tests.
pub fn derive_device_id(verifying_key_bytes: &[u8]) -> DeviceId {
    let digest = Sha256::digest(verifying_key_bytes);

    let mut hex = String::with_capacity(DEVICE_ID_BYTES * 2);
    for byte in &digest[..DEVICE_ID_BYTES] {
        use std::fmt::Write as _;
        let _ = write!(hex, "{byte:02x}");
    }

    // Infallible in practice: the string is 32 hex characters and therefore never empty,
    // which is the only thing `DeviceId::new` refuses.
    DeviceId::new(hex).expect("a hex digest is never empty")
}

/// This device's signing identity.
///
/// # No `#[derive(Debug)]`, no `Serialize`, no accessor for the private half
///
/// Every one of those would be a route by which the key leaves this struct, and the
/// source-text test in `device_identity_tests.rs` fails the build if one appears. See the
/// module comment for why this is structural rather than a matter of discipline.
pub struct DeviceIdentity {
    /// The private half. Never returned, never printed, never serialised — the only
    /// operation performed with it is [`sign`](DeviceIdentity::sign).
    signing_key: SigningKey,
    device_id: DeviceId,
    verifying_key: VerifyingKey,
}

impl DeviceIdentity {
    /// Rebuilds an identity from its private key bytes.
    ///
    /// Private to the crate: the only callers are [`load_or_generate`] and the tests. Nothing
    /// outside this module has a reason to hold key bytes, and the parameter here is the last
    /// point at which they are visible as bytes at all.
    pub(crate) fn from_signing_key_bytes(bytes: &[u8]) -> AppResult<Self> {
        let seed: [u8; SIGNING_KEY_BYTES] =
            bytes.try_into().map_err(|_| AppError::Storage {
                message: "the stored signing key is not the expected length".to_string(),
            })?;

        let signing_key = SigningKey::from_bytes(&seed);
        let verifying_key_bytes = signing_key.verifying_key().to_bytes();
        let verifying_key = VerifyingKey::from_bytes(&verifying_key_bytes)
            .ok_or_else(|| AppError::Storage {
                message: "the stored signing key does not yield a valid verifying key"
                    .to_string(),
            })?;

        Ok(Self {
            signing_key,
            device_id: derive_device_id(&verifying_key_bytes),
            verifying_key,
        })
    }

    /// This device's identifier, derived from its verifying key.
    pub fn device_id(&self) -> &DeviceId {
        &self.device_id
    }

    /// The public half, which is not secret and is what peers record when they pair.
    pub fn verifying_key(&self) -> &VerifyingKey {
        &self.verifying_key
    }

    /// Signs `payload`, which must be the canonical encoding from
    /// [`encode_signing_payload`](crate::domain::encode_signing_payload).
    ///
    /// The signing happens *here*, inside the component that holds the key, rather than by
    /// handing the key to a caller that signs. That is the whole point: the capability
    /// offered is "produce a signature", never "have the key".
    pub fn sign(&self, payload: &[u8]) -> [u8; 64] {
        self.signing_key.sign(payload).to_bytes()
    }
}

/// Prints the device id and the word `<sealed>` — written by hand, never derived.
///
/// A derived `Debug` on a struct holding key bytes would render the private key into any log
/// line, panic message, or `{:?}` a future contributor writes. That is precisely the accident
/// this guards against, so the implementation is spelled out and the source-text test refuses
/// `derive(Debug` in this file.
impl std::fmt::Debug for DeviceIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeviceIdentity")
            .field("device_id", &self.device_id)
            .field("signing_key", &"<sealed>")
            .finish()
    }
}

/// The public half of an identity, as the database records it.
///
/// Holds no secret. Its purpose is to answer "who am I" without touching the secret store,
/// and — more importantly — to record that an identity *exists*. Without that record a failed
/// secret-store read is indistinguishable from a first run, and the system would regenerate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceIdentityRecord {
    pub device_id: DeviceId,
    pub verifying_key: VerifyingKey,
}

/// Where the public half of the identity is recorded, as [`load_or_generate`] needs it.
///
/// A trait rather than the concrete repository so the identity logic is testable without a
/// database, in the same way [`SecretStore`] makes it testable without a keychain.
///
/// [`SecretStore`]: crate::platform::secret_store::SecretStore
pub trait DeviceIdentityRecordStore {
    /// The recorded identity, or `None` on a first run.
    fn load_identity_record(&self) -> AppResult<Option<DeviceIdentityRecord>>;

    /// Records a newly generated identity. Writes public material only.
    fn save_identity_record(&self, record: &DeviceIdentityRecord) -> AppResult<()>;
}

/// Loads this device's identity, generating one only on a genuine first run.
///
/// # The three cases, and why the third is an error
///
/// | Record | Secret store | Result |
/// | --- | --- | --- |
/// | absent | — | generate, store both halves |
/// | present | key found | reuse |
/// | present | key absent **or** unreachable | **error** |
///
/// The third row is the one that matters. Generating a replacement there would give this
/// device a new [`DeviceId`], making it a stranger to every peer that had paired with it —
/// their pairings would keep naming a key it no longer holds, and every one would have to be
/// redone by hand. The storage problem is often temporary; the identity loss would not be.
///
/// Note that a *missing* entry is treated as an error too when the record says one should
/// exist, not only an unreachable store. A record without a key means something removed the
/// key — that is a fault to report, and regenerating would paper over it while destroying
/// the pairings.
pub fn load_or_generate(
    records: &dyn DeviceIdentityRecordStore,
    secrets: &dyn crate::platform::secret_store::SecretStore,
) -> AppResult<DeviceIdentity> {
    use crate::platform::secret_store::SecretLookup;

    match records.load_identity_record()? {
        // A later run. The key must be there; anything else is a fault.
        Some(record) => {
            // A store outage propagates as an error from here rather than being flattened
            // into "no key" — that is exactly the distinction `SecretStore` preserves.
            let stored = secrets.get(IDENTITY_SECRET_NAME)?;

            let SecretLookup::Found(bytes) = stored else {
                return Err(AppError::Storage {
                    message: format!(
                        "this device records an identity ({}) but its signing key is not in \
                         the secure store. Refusing to generate a replacement: a new identity \
                         would make this device unrecognisable to every device it is paired \
                         with, and those pairings would all have to be set up again.",
                        record.device_id
                    ),
                });
            };

            let identity = DeviceIdentity::from_signing_key_bytes(&bytes)?;

            // The recorded public half and the stored private half must describe the same
            // device. A mismatch means one of the two was replaced independently, and
            // proceeding would sign commands under an identifier peers do not associate with
            // this key.
            if identity.device_id() != &record.device_id {
                return Err(AppError::Storage {
                    message: format!(
                        "the signing key in the secure store belongs to device {} but this \
                         device is recorded as {}. Refusing to proceed rather than signing \
                         under an identity that does not match the key.",
                        identity.device_id(),
                        record.device_id
                    ),
                });
            }

            Ok(identity)
        }

        // A genuine first run: no record, so nothing can be lost by generating.
        None => {
            // The system CSPRNG, wrapped so a failure to read entropy is an error rather
            // than a panic. Generating a key from a degraded entropy source would produce an
            // identity an attacker could reproduce, so this must fail loudly instead.
            let mut seed = [0u8; SIGNING_KEY_BYTES];
            getrandom::fill(&mut seed).map_err(|e| AppError::Storage {
                message: format!("could not read system entropy to generate an identity: {e}"),
            })?;

            let identity = DeviceIdentity::from_signing_key_bytes(&seed)?;

            // The private half goes to the platform store and nowhere else. Written before
            // the record, so a crash between the two leaves a key with no record — which
            // reads as a first run and regenerates harmlessly. The opposite order would
            // leave a record with no key, which is the unrecoverable state above.
            secrets.set(IDENTITY_SECRET_NAME, &seed)?;

            records.save_identity_record(&DeviceIdentityRecord {
                device_id: identity.device_id().clone(),
                verifying_key: identity.verifying_key().clone(),
            })?;

            Ok(identity)
        }
    }
}

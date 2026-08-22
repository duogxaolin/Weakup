//! An opaque identifier for one device.
//!
//! Deliberately structureless. It does not embed an account, a platform, a public key
//! fingerprint, or anything else — see the module's own reasoning below, and design D10.
//!
//! The Dart implementation in `mobile/lib/domain/device_id.dart` mirrors this file.

use serde::{Deserialize, Serialize};

use crate::core::{AppError, AppResult};

/// The identity of a device, as an opaque string.
///
/// **No structure is imposed, and that is the decision rather than an omission.** The
/// authorization rule's own spec says that being signed in to the same account is not
/// sufficient to command a device, so account identity is not what any of these rules
/// branch on. Giving this type structure — an embedded account, a platform tag, a key
/// fingerprint — would bake a guess about the account model into rules that do not need
/// one, which is the same argument that keeps `PlatformCapabilities` out of
/// [`RemoteCommandContext`].
///
/// The change that introduces a transport chooses what a device identifier actually looks
/// like. These rules only need to compare two of them and record one.
///
/// [`RemoteCommandContext`]: crate::domain::RemoteCommandContext
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DeviceId(String);

impl DeviceId {
    /// Validates that the identifier is not empty.
    ///
    /// Emptiness is the one property worth refusing without knowing the eventual format: an
    /// empty id compares equal to another empty id, so accepting it would let two unrelated
    /// devices be treated as the same one — and "the same one" is what pairing turns into
    /// authority to shut a machine down.
    pub fn new(value: impl Into<String>) -> AppResult<Self> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(AppError::validation("A device id cannot be empty."));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for DeviceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_id_is_refused() {
        // Two empty ids would compare equal, making two unrelated devices one device.
        assert!(DeviceId::new("").is_err());
        assert!(DeviceId::new("   ").is_err());
    }

    #[test]
    fn an_ordinary_id_round_trips_unchanged() {
        let id = DeviceId::new("phone-a").expect("non-empty");
        assert_eq!(id.as_str(), "phone-a");
        assert_eq!(id.to_string(), "phone-a");
    }

    #[test]
    fn no_structure_is_imposed_on_the_value() {
        // A uuid, an opaque token, and something with an at-sign are all equally acceptable.
        // If a later change makes one of these fail, it has embedded an assumption about the
        // account model into an identifier that must not carry one.
        for value in [
            "3f2504e0-4f89-11d3-9a0c-0305e82c3301",
            "opaque-token",
            "device@example",
            "  padded but not empty  ",
        ] {
            assert!(DeviceId::new(value).is_ok(), "{value} should be accepted");
        }
    }

    #[test]
    fn two_ids_with_the_same_value_are_the_same_device() {
        assert_eq!(
            DeviceId::new("phone-a").unwrap(),
            DeviceId::new("phone-a").unwrap()
        );
        assert_ne!(
            DeviceId::new("phone-a").unwrap(),
            DeviceId::new("phone-b").unwrap()
        );
    }
}

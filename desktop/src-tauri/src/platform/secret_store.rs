//! The seam between the app and the platform's protected credential store.
//!
//! Keychain on macOS, Credential Manager on Windows, Secret Service on Linux. This module
//! is the only place any of them is named, so every decision that *depends* on the store
//! can be tested against [`InMemorySecretStore`] instead of against an OS facility that
//! behaves differently on every target (design D7).
//!
//! # Why "not found" and "unavailable" are different errors
//!
//! This is the distinction the whole module exists to preserve, and conflating the two is
//! what would cause the worst failure this change can produce.
//!
//! A device records in its database that it holds an identity. If the secret store then
//! cannot produce the private key, there are two possible reasons, and they call for
//! opposite responses:
//!
//! - **Not found** — there is genuinely no entry. On a first run this is normal and the
//!   caller generates one.
//! - **Unavailable** — the store exists but could not be reached: the keychain is locked,
//!   the D-Bus session is missing, the credential service is not running. The entry may be
//!   perfectly intact behind that failure.
//!
//! An implementation returning "not found" for both would make the second case look like a
//! first run. The caller would generate a replacement identity, and every pairing this
//! device holds would be silently destroyed — peers would keep trusting a key it no longer
//! has, and it would appear to all of them as a stranger. A locked keychain is recoverable;
//! a regenerated identity is not. So the store reports which happened and lets the caller
//! refuse to guess.
//!
//! The Dart implementation in `mobile/lib/platform/secret_store.dart` mirrors this file.

use std::collections::HashMap;
use std::sync::Mutex;

use crate::core::{AppError, AppResult};

/// Why a secret could not be read.
///
/// A closed enum rather than a string, so a caller must match on the distinction rather
/// than parse for it — see the module comment for why the distinction is load-bearing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecretLookup<T> {
    /// The entry exists and here it is.
    Found(T),
    /// There is genuinely no entry under this name. On a first run this is the expected
    /// answer, and the only case in which generating a new secret is correct.
    NotFound,
}

impl<T> SecretLookup<T> {
    pub fn found(self) -> Option<T> {
        match self {
            Self::Found(value) => Some(value),
            Self::NotFound => None,
        }
    }

    pub fn is_found(&self) -> bool {
        matches!(self, Self::Found(_))
    }
}

/// The platform's protected credential store, as the app sees it.
///
/// Deliberately small: get, set, delete over a namespaced name. Anything richer would be a
/// surface the fake has to reproduce faithfully, and the fake is what every other test in
/// this change depends on.
///
/// A *store unavailable* condition is reported as an `Err`, while a genuinely absent entry
/// is `Ok(SecretLookup::NotFound)`. That split is the module's reason for existing.
pub trait SecretStore: Send + Sync {
    /// The secret stored under `name`, or [`SecretLookup::NotFound`] if there is none.
    ///
    /// Returns `Err` only when the store itself could not be consulted. A caller must not
    /// treat that as an absent entry — see the module comment.
    fn get(&self, name: &str) -> AppResult<SecretLookup<Vec<u8>>>;

    /// Writes `secret` under `name`, replacing any existing value.
    fn set(&self, name: &str, secret: &[u8]) -> AppResult<()>;

    /// Removes the entry under `name`. Removing an absent entry is not an error: the
    /// caller's intent is that nothing be stored there, and that is already true.
    fn delete(&self, name: &str) -> AppResult<()>;
}

/// The service name every entry is filed under, so the app's secrets are namespaced away
/// from every other application's on the same machine.
const SERVICE: &str = "com.weakup.desktop";

/// The real store, backed by `keyring`.
///
/// # UNVERIFIED on Windows and Linux
///
/// Written against `keyring`'s documented behaviour and exercised only on macOS. Keychain,
/// Credential Manager, and Secret Service differ in their failure modes, and two of the
/// three cannot be reached from the machine this was developed on. Design D7 records the
/// gap; closing it requires running on each target rather than more tests here. Every
/// *decision* that consumes this trait is covered against [`InMemorySecretStore`], so what
/// is unverified is the storage mechanism, not the logic around it.
pub struct KeyringSecretStore;

impl KeyringSecretStore {
    pub fn new() -> Self {
        Self
    }

    fn entry(name: &str) -> AppResult<keyring::Entry> {
        keyring::Entry::new(SERVICE, name).map_err(|e| AppError::Storage {
            message: format!("the platform credential store could not be reached: {e}"),
        })
    }
}

impl Default for KeyringSecretStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SecretStore for KeyringSecretStore {
    fn get(&self, name: &str) -> AppResult<SecretLookup<Vec<u8>>> {
        match Self::entry(name)?.get_secret() {
            Ok(secret) => Ok(SecretLookup::Found(secret)),
            // The one error that means "there is nothing here" rather than "I could not
            // look". Everything else is a failure to consult the store, and is reported as
            // such so the caller does not mistake it for a first run.
            Err(keyring::Error::NoEntry) => Ok(SecretLookup::NotFound),
            Err(e) => Err(AppError::Storage {
                message: format!("the platform credential store could not be read: {e}"),
            }),
        }
    }

    fn set(&self, name: &str, secret: &[u8]) -> AppResult<()> {
        Self::entry(name)?
            .set_secret(secret)
            .map_err(|e| AppError::Storage {
                message: format!("the platform credential store could not be written: {e}"),
            })
    }

    fn delete(&self, name: &str) -> AppResult<()> {
        match Self::entry(name)?.delete_credential() {
            Ok(()) => Ok(()),
            // Already absent, which is what the caller asked for.
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(AppError::Storage {
                message: format!("the platform credential store could not be cleared: {e}"),
            }),
        }
    }
}

/// A non-persistent store for tests.
///
/// Exists because the platform stores cannot be exercised on every target from a developer
/// machine, and because a test that wrote to the real Keychain would leave state behind on
/// the machine running it. Every decision in this change — generate once, reuse, error
/// rather than regenerate, sign but never read — is tested against this.
///
/// [`fail_with`] makes the store report itself unavailable, which is how the
/// "an unreadable identity is an error" scenario is reached without locking a real keychain.
///
/// [`fail_with`]: InMemorySecretStore::fail_with
pub struct InMemorySecretStore {
    entries: Mutex<HashMap<String, Vec<u8>>>,
    /// When set, every operation fails with this message rather than touching `entries`.
    /// Models a locked keychain or an absent credential service — the case that must never
    /// be mistaken for an empty store.
    unavailable: Mutex<Option<String>>,
}

impl InMemorySecretStore {
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            unavailable: Mutex::new(None),
        }
    }

    /// Makes every subsequent operation report the store as unavailable.
    ///
    /// Note what this is *not*: it does not remove the entries. That is the whole point —
    /// the secret is still there, the store simply cannot be consulted, and a caller that
    /// responded by generating a replacement would destroy a recoverable situation.
    pub fn fail_with(&self, message: impl Into<String>) {
        *self.unavailable.lock().expect("not poisoned") = Some(message.into());
    }

    /// Restores normal operation, so a test can prove the secret survived the outage.
    pub fn recover(&self) {
        *self.unavailable.lock().expect("not poisoned") = None;
    }

    fn check_available(&self) -> AppResult<()> {
        match self.unavailable.lock().expect("not poisoned").as_ref() {
            Some(message) => Err(AppError::Storage {
                message: message.clone(),
            }),
            None => Ok(()),
        }
    }
}

impl Default for InMemorySecretStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SecretStore for InMemorySecretStore {
    fn get(&self, name: &str) -> AppResult<SecretLookup<Vec<u8>>> {
        self.check_available()?;
        Ok(match self.entries.lock().expect("not poisoned").get(name) {
            Some(secret) => SecretLookup::Found(secret.clone()),
            None => SecretLookup::NotFound,
        })
    }

    fn set(&self, name: &str, secret: &[u8]) -> AppResult<()> {
        self.check_available()?;
        self.entries
            .lock()
            .expect("not poisoned")
            .insert(name.to_string(), secret.to_vec());
        Ok(())
    }

    fn delete(&self, name: &str) -> AppResult<()> {
        self.check_available()?;
        self.entries.lock().expect("not poisoned").remove(name);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NAME: &str = "device-identity-private-key";

    #[test]
    fn a_stored_secret_round_trips() {
        let store = InMemorySecretStore::new();

        store.set(NAME, &[1, 2, 3, 4]).expect("set");

        assert_eq!(
            store.get(NAME).expect("get"),
            SecretLookup::Found(vec![1, 2, 3, 4])
        );
    }

    #[test]
    fn an_absent_entry_is_not_found_rather_than_an_error() {
        // The first-run answer. It must be `Ok`, because a caller distinguishing "generate
        // one" from "something is wrong" reads exactly this.
        let store = InMemorySecretStore::new();

        assert_eq!(store.get(NAME).expect("get"), SecretLookup::NotFound);
    }

    #[test]
    fn writing_twice_replaces_rather_than_appends() {
        let store = InMemorySecretStore::new();

        store.set(NAME, &[1, 2, 3]).expect("first");
        store.set(NAME, &[9, 9]).expect("second");

        assert_eq!(store.get(NAME).expect("get"), SecretLookup::Found(vec![9, 9]));
    }

    #[test]
    fn a_deleted_entry_reads_as_not_found() {
        let store = InMemorySecretStore::new();
        store.set(NAME, &[1, 2, 3]).expect("set");

        store.delete(NAME).expect("delete");

        assert_eq!(store.get(NAME).expect("get"), SecretLookup::NotFound);
    }

    #[test]
    fn deleting_an_absent_entry_is_not_an_error() {
        // The caller's intent is that nothing be stored there, and that is already true.
        let store = InMemorySecretStore::new();

        assert!(store.delete(NAME).is_ok());
    }

    #[test]
    fn an_unavailable_store_is_an_error_and_not_a_missing_entry() {
        // The distinction this module exists to preserve, asserted on the *shape* of the
        // result rather than on any message. A store that reported `NotFound` here would
        // tell a caller "there is no identity", and the caller would generate a replacement
        // — silently destroying every pairing this device holds.
        let store = InMemorySecretStore::new();
        store.set(NAME, &[1, 2, 3]).expect("set");

        store.fail_with("the keychain is locked");

        let result = store.get(NAME);
        assert!(
            result.is_err(),
            "an unreachable store must be an error, never NotFound: a caller that read \
             this as an absent entry would regenerate the identity and lose every pairing"
        );
        // The variant, not the string.
        assert!(matches!(result, Err(AppError::Storage { .. })));
    }

    #[test]
    fn the_two_failure_modes_are_distinguishable_by_matching_alone() {
        // Stated as its own test because the guarantee is that a caller can *tell them
        // apart* — not merely that each is reachable. A caller matching on these two must
        // land in different arms without inspecting any message.
        let empty = InMemorySecretStore::new();
        let unavailable = InMemorySecretStore::new();
        unavailable.fail_with("no credential service");

        let absent = empty.get(NAME);
        let broken = unavailable.get(NAME);

        assert!(matches!(absent, Ok(SecretLookup::NotFound)));
        assert!(broken.is_err());
    }

    #[test]
    fn an_outage_does_not_destroy_the_stored_secret() {
        // Why "unavailable" must not mean "absent": the entry is still there behind the
        // failure. A caller that regenerated on the error would have thrown away a secret
        // that came back on its own.
        let store = InMemorySecretStore::new();
        store.set(NAME, &[7, 7, 7]).expect("set");

        store.fail_with("the keychain is locked");
        assert!(store.get(NAME).is_err());

        store.recover();
        assert_eq!(
            store.get(NAME).expect("get"),
            SecretLookup::Found(vec![7, 7, 7]),
            "the secret was intact the whole time; only the store was unreachable"
        );
    }

    #[test]
    fn entries_are_namespaced_by_name() {
        let store = InMemorySecretStore::new();

        store.set("a", &[1]).expect("set a");
        store.set("b", &[2]).expect("set b");

        assert_eq!(store.get("a").expect("get"), SecretLookup::Found(vec![1]));
        assert_eq!(store.get("b").expect("get"), SecretLookup::Found(vec![2]));
    }

    #[test]
    fn the_fake_satisfies_the_trait_so_callers_need_not_know_which_store_they_hold() {
        // The spec's "the interface does not leak the storage mechanism" scenario, stated
        // as a compiling fact.
        fn takes_store(store: &dyn SecretStore) -> AppResult<SecretLookup<Vec<u8>>> {
            store.get(NAME)
        }

        let store = InMemorySecretStore::new();
        store.set(NAME, &[5]).expect("set");

        assert_eq!(
            takes_store(&store).expect("get"),
            SecretLookup::Found(vec![5])
        );
    }
}

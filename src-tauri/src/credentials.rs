//! OS-keychain-backed credential persistence.
//! `CredentialsStore` never holds credentials itself — every `set`/`get` round-trips through
//! a `keyring_core` credential store, so nothing is lost when the app restarts. Production
//! uses the platform-native store; tests substitute an in-memory `keyring_core::mock::Store`
//! so `cargo test` never touches a real OS keychain.

use keyring_core::api::CredentialStoreApi;
use keyring_core::{Entry, Error};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, OnceLock};

const SERVICE: &str = "repo-dashboard";
const ACCOUNT: &str = "bitbucket";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Credentials {
    pub username: String,
    pub app_password: String,
    pub workspace: String,
}

type StoreHandle = Arc<dyn CredentialStoreApi + Send + Sync>;

/// Where a `CredentialsStore` gets its backing `keyring_core` store from.
enum Backing {
    /// Used by tests, which inject an in-memory `keyring_core::mock::Store` up front.
    #[cfg(test)]
    Fixed(StoreHandle),
    /// Used in production: the platform-native store is only touched on first
    /// `set`/`get`, so a missing OS keychain service surfaces as a command error
    /// instead of panicking at app startup.
    LazyNative(OnceLock<Result<StoreHandle, String>>),
}

pub struct CredentialsStore {
    backing: Backing,
}

impl CredentialsStore {
    #[cfg(test)]
    fn from_store(store: StoreHandle) -> Self {
        Self { backing: Backing::Fixed(store) }
    }

    fn store(&self) -> Result<&StoreHandle, String> {
        match &self.backing {
            #[cfg(test)]
            Backing::Fixed(store) => Ok(store),
            Backing::LazyNative(cell) => cell.get_or_init(native_store).as_ref().map_err(Clone::clone),
        }
    }

    fn entry(&self) -> Result<Entry, String> {
        self.store()?
            .build(SERVICE, ACCOUNT, None)
            .map_err(|e| e.to_string())
    }

    pub fn set(&self, credentials: &Credentials) -> Result<(), String> {
        let payload = serde_json::to_string(credentials).map_err(|e| e.to_string())?;
        self.entry()?.set_password(&payload).map_err(|e| e.to_string())
    }

    pub fn get(&self) -> Result<Option<Credentials>, String> {
        match self.entry()?.get_password() {
            Ok(payload) => serde_json::from_str(&payload).map(Some).map_err(|e| e.to_string()),
            Err(Error::NoEntry) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }
}

fn native_store() -> Result<StoreHandle, String> {
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    let store: StoreHandle = apple_native_keyring_store::keychain::Store::new().map_err(|e| e.to_string())?;
    #[cfg(target_os = "windows")]
    let store: StoreHandle = windows_native_keyring_store::Store::new().map_err(|e| e.to_string())?;
    #[cfg(all(unix, not(any(target_os = "macos", target_os = "ios", target_os = "android"))))]
    let store: StoreHandle = zbus_secret_service_keyring_store::Store::new().map_err(|e| e.to_string())?;
    Ok(store)
}

impl Default for CredentialsStore {
    fn default() -> Self {
        Self { backing: Backing::LazyNative(OnceLock::new()) }
    }
}

#[derive(Default)]
pub struct CredentialsState(pub CredentialsStore);

#[cfg(test)]
mod tests {
    use super::*;

    fn creds(suffix: &str) -> Credentials {
        Credentials {
            username: format!("alice-{suffix}"),
            app_password: format!("hunter2-{suffix}"),
            workspace: format!("acme-{suffix}"),
        }
    }

    #[test]
    fn set_then_restart_then_get_round_trips() {
        let backing = keyring_core::mock::Store::new().unwrap();
        let store = CredentialsStore::from_store(backing.clone());
        let expected = creds("restart");
        store.set(&expected).unwrap();

        // Simulate an app restart: a brand-new `CredentialsStore` wrapper, but pointed at the
        // same backing keychain, since a real restart only drops process state, not the keychain.
        let after_restart = CredentialsStore::from_store(backing);
        assert_eq!(after_restart.get().unwrap(), Some(expected));
    }

    #[test]
    fn get_returns_none_when_nothing_set() {
        let store = CredentialsStore::from_store(keyring_core::mock::Store::new().unwrap());
        assert_eq!(store.get().unwrap(), None);
    }

    #[test]
    fn set_overwrites_previous_value() {
        let backing = keyring_core::mock::Store::new().unwrap();
        let store = CredentialsStore::from_store(backing);
        store.set(&creds("first")).unwrap();
        let updated = creds("second");
        store.set(&updated).unwrap();

        assert_eq!(store.get().unwrap(), Some(updated));
    }
}

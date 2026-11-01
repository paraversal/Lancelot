//! Data source tokens in the system's keychain, so they are in no file (technical design §10).

use keyring::Entry;
use lancelot_service::TokenStore;

/// The keychain item's service: each source is an account under it.
const SERVICE: &str = "Lancelot data source";

pub struct Keychain;

fn entry(id: &str) -> Result<Entry, String> {
    Entry::new(SERVICE, id).map_err(|e| e.to_string())
}

impl TokenStore for Keychain {
    fn get(&self, id: &str) -> Result<Option<String>, String> {
        match entry(id)?.get_password() {
            Ok(token) => Ok(Some(token)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    fn set(&mut self, id: &str, token: &str) -> Result<(), String> {
        entry(id)?.set_password(token).map_err(|e| e.to_string())
    }

    fn delete(&mut self, id: &str) -> Result<(), String> {
        match entry(id)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}

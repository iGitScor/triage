use crate::paths::{VAULT_ACCOUNT, VAULT_SERVICE};
use std::collections::HashMap;
use std::sync::Mutex;

/// Account ID → that account's secrets (tokens).
pub type Secrets = HashMap<String, HashMap<String, String>>;

/// Where tokens live. One entry for all accounts, read once per launch.
pub trait Vault: Send + Sync {
    fn load(&self) -> Result<Secrets, String>;
    fn save(&self, secrets: &Secrets) -> Result<(), String>;
}

/// The Windows Credential Manager (and the macOS Keychain or Secret Service when developing elsewhere).
pub struct CredentialVault;

impl CredentialVault {
    fn entry() -> Result<keyring::Entry, String> {
        keyring::Entry::new(VAULT_SERVICE, VAULT_ACCOUNT).map_err(|e| e.to_string())
    }
}

impl Vault for CredentialVault {
    fn load(&self) -> Result<Secrets, String> {
        match Self::entry()?.get_password() {
            Ok(text) => serde_json::from_str(&text).map_err(|e| e.to_string()),
            Err(keyring::Error::NoEntry) => Ok(Secrets::new()),
            Err(e) => Err(e.to_string()),
        }
    }

    fn save(&self, secrets: &Secrets) -> Result<(), String> {
        let entry = Self::entry()?;
        if secrets.is_empty() {
            return match entry.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
                Err(e) => Err(e.to_string()),
            };
        }
        entry.set_password(&serde_json::to_string(secrets).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
    }
}

/// For tests and the demo.
#[derive(Default)]
pub struct MemoryVault(Mutex<Secrets>);

impl Vault for MemoryVault {
    fn load(&self) -> Result<Secrets, String> {
        Ok(self.0.lock().unwrap().clone())
    }

    fn save(&self, secrets: &Secrets) -> Result<(), String> {
        *self.0.lock().unwrap() = secrets.clone();
        Ok(())
    }
}

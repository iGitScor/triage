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

/// The system's credential store: the Windows Credential Manager, the Secret Service on Linux (GNOME Keyring,
/// KWallet, KeePassXC), the Keychain when developing on a Mac.
pub struct CredentialVault;

impl CredentialVault {
    fn entry() -> Result<keyring::Entry, String> {
        keyring::Entry::new(VAULT_SERVICE, VAULT_ACCOUNT).map_err(|e| e.to_string())
    }
}

/// What went wrong, in words a user can act on: on Linux, no keyring service is the usual cause, and it must not
/// pass for "no tokens", which would look as if every account were disconnected.
fn explain(error: keyring::Error) -> String {
    match error {
        keyring::Error::NoStorageAccess(_) | keyring::Error::PlatformFailure(_) if cfg!(target_os = "linux") => {
            NO_KEYRING.to_string()
        }
        error => error.to_string(),
    }
}

/// Shown when Linux has no Secret Service running.
pub const NO_KEYRING: &str = "No keyring is running: Remora keeps tokens in the system keyring (GNOME Keyring, KWallet or KeePassXC). Start or install one, then open Remora again.";

impl Vault for CredentialVault {
    fn load(&self) -> Result<Secrets, String> {
        match Self::entry()?.get_password() {
            Ok(text) => serde_json::from_str(&text).map_err(|e| e.to_string()),
            Err(keyring::Error::NoEntry) => Ok(Secrets::new()),
            Err(e) => Err(explain(e)),
        }
    }

    fn save(&self, secrets: &Secrets) -> Result<(), String> {
        let entry = Self::entry()?;
        if secrets.is_empty() {
            return match entry.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
                Err(e) => Err(explain(e)),
            };
        }
        entry.set_password(&serde_json::to_string(secrets).map_err(|e| e.to_string())?).map_err(explain)
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

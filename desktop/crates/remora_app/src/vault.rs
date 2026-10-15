use crate::paths::{VAULT_INDEX, VAULT_LEGACY, VAULT_SERVICE};
use std::collections::{BTreeSet, HashMap};
use std::sync::Mutex;

/// Account ID → that account's secrets (tokens).
pub type Secrets = HashMap<String, HashMap<String, String>>;

/// Where tokens live: every account's, read once per launch, saved whole.
pub trait Vault: Send + Sync {
    fn load(&self) -> Result<Secrets, String>;
    fn save(&self, secrets: &Secrets) -> Result<(), String>;
}

/// The entries of the credential store under Remora's service, by user name. `get` gives `None` for no entry, and
/// `delete` of a missing entry is no error.
pub trait CredentialStore: Send + Sync {
    fn get(&self, user: &str) -> Result<Option<String>, String>;
    fn set(&self, user: &str, value: &str) -> Result<(), String>;
    fn delete(&self, user: &str) -> Result<(), String>;
}

/// The system's credential store: the Windows Credential Manager, the Secret Service on Linux (GNOME Keyring,
/// KWallet, KeePassXC), the Keychain when developing on a Mac.
pub struct SystemStore;

impl SystemStore {
    fn entry(user: &str) -> Result<keyring::Entry, String> {
        keyring::Entry::new(VAULT_SERVICE, user).map_err(|e| e.to_string())
    }
}

impl CredentialStore for SystemStore {
    fn get(&self, user: &str) -> Result<Option<String>, String> {
        match Self::entry(user)?.get_password() {
            Ok(text) => Ok(Some(text)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(explain(e)),
        }
    }

    fn set(&self, user: &str, value: &str) -> Result<(), String> {
        Self::entry(user)?.set_password(value).map_err(explain)
    }

    fn delete(&self, user: &str) -> Result<(), String> {
        match Self::entry(user)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(explain(e)),
        }
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

/// One entry per account (user = the account's id), and an index of the ids, since the store can't list its entries
/// (SEC-10). A Windows entry holds at most 2,560 bytes, and a corrupt one loses only its account. The single entry
/// of earlier versions is moved over once.
pub struct CredentialVault {
    store: Box<dyn CredentialStore>,
    /// What the store holds, as last read or saved: a save writes only what changed. `None` when unsure.
    known: Mutex<Option<Known>>,
}

#[derive(Clone)]
struct Known {
    secrets: Secrets,
    /// The ids the index lists, which may include an entry gone missing.
    indexed: BTreeSet<String>,
}

impl CredentialVault {
    pub fn system() -> Self {
        CredentialVault::new(Box::new(SystemStore))
    }

    pub fn new(store: Box<dyn CredentialStore>) -> Self {
        CredentialVault { store, known: Mutex::new(None) }
    }

    /// The ids the index lists; `None` when there is no index yet.
    fn index(&self) -> Result<Option<BTreeSet<String>>, String> {
        match self.store.get(VAULT_INDEX)? {
            Some(text) => {
                serde_json::from_str(&text).map(Some).map_err(|e| format!("The token index is unreadable: {e}"))
            }
            None => Ok(None),
        }
    }

    /// One account's secrets. A corrupt entry is left alone and skipped: that account asks to be reconnected.
    fn account(&self, id: &str) -> Result<Option<HashMap<String, String>>, String> {
        let Some(text) = self.store.get(id)? else { return Ok(None) };
        match serde_json::from_str(&text) {
            Ok(secrets) => Ok(Some(secrets)),
            Err(error) => {
                eprintln!("Remora: the tokens of account {id} are unreadable: {error}");
                Ok(None)
            }
        }
    }

    /// The single entry of earlier versions, split into one entry per account. Safe to stop at any point: each
    /// entry is written and read back, then the index, and only then is the old entry deleted. An old entry that
    /// can't be read changes nothing.
    fn migrate(&self) -> Result<Known, String> {
        let Some(text) = self.store.get(VAULT_LEGACY)? else {
            return Ok(Known { secrets: Secrets::new(), indexed: BTreeSet::new() });
        };
        let secrets: Secrets =
            serde_json::from_str(&text).map_err(|e| format!("The saved tokens are unreadable: {e}"))?;
        for (id, account) in &secrets {
            let value = serde_json::to_string(account).map_err(|e| e.to_string())?;
            self.store.set(id, &value)?;
            if self.store.get(id)?.as_deref() != Some(value.as_str()) {
                return Err(format!("The tokens of account {id} couldn't be moved to their own entry."));
            }
        }
        let indexed: BTreeSet<String> = secrets.keys().cloned().collect();
        self.store.set(VAULT_INDEX, &serde_json::to_string(&indexed).map_err(|e| e.to_string())?)?;
        if let Err(error) = self.store.delete(VAULT_LEGACY) {
            // Ignored from now on, since the index exists; deleted with everything else by Erase.
            eprintln!("Remora: the old token entry couldn't be deleted: {error}");
        }
        Ok(Known { secrets, indexed })
    }

    fn read(&self) -> Result<Known, String> {
        let Some(indexed) = self.index()? else { return self.migrate() };
        let mut secrets = Secrets::new();
        for id in &indexed {
            if let Some(account) = self.account(id)? {
                secrets.insert(id.clone(), account);
            }
        }
        Ok(Known { secrets, indexed })
    }

    /// New and changed entries first, then the removed ones, then the index: stopped half-way, the index may list an
    /// entry already gone (skipped), never miss one still there.
    fn write(&self, secrets: &Secrets, known: Option<Known>) -> Result<Known, String> {
        let (before, indexed) = match known {
            Some(known) => (known.secrets, known.indexed),
            None => (Secrets::new(), self.index().ok().flatten().unwrap_or_default()),
        };
        for (id, account) in secrets {
            if before.get(id) != Some(account) {
                self.store.set(id, &serde_json::to_string(account).map_err(|e| e.to_string())?)?;
            }
        }
        for id in indexed.iter().chain(before.keys()).filter(|id| !secrets.contains_key(*id)) {
            self.store.delete(id)?;
        }
        let ids: BTreeSet<String> = secrets.keys().cloned().collect();
        if secrets.is_empty() {
            // Erase: the index, and the old single entry if it is still there.
            self.store.delete(VAULT_INDEX)?;
            self.store.delete(VAULT_LEGACY)?;
        } else if ids != indexed {
            self.store.set(VAULT_INDEX, &serde_json::to_string(&ids).map_err(|e| e.to_string())?)?;
        }
        Ok(Known { secrets: secrets.clone(), indexed: ids })
    }
}

impl Vault for CredentialVault {
    fn load(&self) -> Result<Secrets, String> {
        let known = self.read()?;
        let secrets = known.secrets.clone();
        *self.known.lock().unwrap() = Some(known);
        Ok(secrets)
    }

    fn save(&self, secrets: &Secrets) -> Result<(), String> {
        let mut known = self.known.lock().unwrap();
        // Unsure until it worked: after a failed save, the store is read again first. Nothing is saved before the
        // old single entry is moved over, which would lose the other accounts' tokens.
        let before = match known.take() {
            None if !secrets.is_empty() => Some(self.read()?),
            before => before,
        };
        let written = self.write(secrets, before)?;
        *known = Some(written);
        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::sync::Arc;

    /// A credential store in memory that can fail its `n`th write or delete from now, or every read.
    #[derive(Default)]
    struct FakeStore {
        entries: Mutex<BTreeMap<String, String>>,
        fail_at: Mutex<Option<usize>>,
        reads_fail: Mutex<bool>,
    }

    impl FakeStore {
        fn with(entries: &[(&str, &str)]) -> Arc<Self> {
            let store = Arc::new(FakeStore::default());
            for (user, value) in entries {
                store.entries.lock().unwrap().insert(user.to_string(), value.to_string());
            }
            store
        }

        fn users(&self) -> Vec<String> {
            self.entries.lock().unwrap().keys().cloned().collect()
        }

        fn value(&self, user: &str) -> Option<String> {
            self.entries.lock().unwrap().get(user).cloned()
        }

        fn fail_after(&self, writes: usize) {
            *self.fail_at.lock().unwrap() = Some(writes);
        }

        fn change(&self) -> Result<(), String> {
            let mut fail_at = self.fail_at.lock().unwrap();
            match *fail_at {
                Some(0) => Err("The credential store is locked.".into()),
                Some(n) => {
                    *fail_at = Some(n - 1);
                    Ok(())
                }
                None => Ok(()),
            }
        }
    }

    impl CredentialStore for Arc<FakeStore> {
        fn get(&self, user: &str) -> Result<Option<String>, String> {
            if *self.reads_fail.lock().unwrap() {
                return Err("The credential store is locked.".into());
            }
            Ok(self.value(user))
        }

        fn set(&self, user: &str, value: &str) -> Result<(), String> {
            self.change()?;
            self.entries.lock().unwrap().insert(user.to_string(), value.to_string());
            Ok(())
        }

        fn delete(&self, user: &str) -> Result<(), String> {
            self.change()?;
            self.entries.lock().unwrap().remove(user);
            Ok(())
        }
    }

    fn vault(store: &Arc<FakeStore>) -> CredentialVault {
        CredentialVault::new(Box::new(store.clone()))
    }

    fn token(value: &str) -> HashMap<String, String> {
        HashMap::from([("token".to_string(), value.to_string())])
    }

    const LEGACY: &str = r#"{"a1":{"token":"t1"},"a2":{"token":"t2"}}"#;

    fn both() -> Secrets {
        Secrets::from([("a1".into(), token("t1")), ("a2".into(), token("t2"))])
    }

    /// SEC-10: each account gets its own entry, listed in the index, and the old single entry goes.
    #[test]
    fn the_single_entry_is_split_once() {
        let store = FakeStore::with(&[(VAULT_LEGACY, LEGACY)]);
        assert_eq!(vault(&store).load().unwrap(), both());
        assert_eq!(store.users(), ["a1", "a2", VAULT_INDEX]);
        assert_eq!(store.value("a1").as_deref(), Some(r#"{"token":"t1"}"#));
        assert_eq!(vault(&store).load().unwrap(), both(), "read again from the new entries");
    }

    /// Stopped at any write, the migration loses nothing: the old entry stays until the new ones are all there, and
    /// the next launch starts again.
    #[test]
    fn an_interrupted_migration_loses_nothing() {
        for writes in 0..3 {
            let store = FakeStore::with(&[(VAULT_LEGACY, LEGACY)]);
            store.fail_after(writes);
            assert!(vault(&store).load().is_err(), "write {writes}");
            assert_eq!(store.value(VAULT_LEGACY).as_deref(), Some(LEGACY), "write {writes}");
            *store.fail_at.lock().unwrap() = None;
            assert_eq!(vault(&store).load().unwrap(), both(), "write {writes}");
            assert!(store.value(VAULT_LEGACY).is_none());
        }
    }

    /// An old entry that couldn't be deleted is ignored once the index exists.
    #[test]
    fn a_leftover_old_entry_is_ignored() {
        let store = FakeStore::with(&[(VAULT_LEGACY, LEGACY)]);
        store.fail_after(3);
        assert_eq!(vault(&store).load().unwrap(), both());
        assert_eq!(store.value(VAULT_LEGACY).as_deref(), Some(LEGACY));
        *store.fail_at.lock().unwrap() = None;
        let vault = vault(&store);
        vault.save(&Secrets::from([("a1".into(), token("t1"))])).unwrap();
        assert_eq!(vault.load().unwrap().len(), 1, "a2 doesn't come back from the old entry");
    }

    /// An old entry that can't be read is an error, and nothing is written or deleted.
    #[test]
    fn an_unreadable_old_entry_wipes_nothing() {
        let store = FakeStore::with(&[(VAULT_LEGACY, "{not json")]);
        assert!(vault(&store).load().is_err());
        assert_eq!(store.users(), [VAULT_LEGACY]);
    }

    /// A corrupt entry loses only its own account.
    #[test]
    fn a_corrupt_entry_loses_only_its_account() {
        let store =
            FakeStore::with(&[(VAULT_INDEX, r#"["a1","a2"]"#), ("a1", "{not json"), ("a2", r#"{"token":"t2"}"#)]);
        assert_eq!(vault(&store).load().unwrap(), Secrets::from([("a2".into(), token("t2"))]));
        assert_eq!(store.value("a1").as_deref(), Some("{not json"), "left for the user to reconnect");
    }

    /// A store that can't be read isn't taken for "no tokens".
    #[test]
    fn a_failed_read_is_an_error() {
        let store = FakeStore::with(&[(VAULT_INDEX, r#"["a1"]"#), ("a1", r#"{"token":"t1"}"#)]);
        *store.reads_fail.lock().unwrap() = true;
        assert!(vault(&store).load().is_err());
    }

    /// A save writes the accounts that changed, deletes the removed ones, and keeps the index current.
    #[test]
    fn a_save_writes_only_what_changed() {
        let store = FakeStore::with(&[]);
        let vault = vault(&store);
        assert!(vault.load().unwrap().is_empty());
        vault.save(&both()).unwrap();
        assert_eq!(store.users(), ["a1", "a2", VAULT_INDEX]);

        store.fail_after(1);
        let mut renewed = both();
        renewed.insert("a2".into(), token("new"));
        vault.save(&renewed).unwrap();
        assert_eq!(store.value("a2").as_deref(), Some(r#"{"token":"new"}"#), "one write: a2 alone");

        *store.fail_at.lock().unwrap() = None;
        vault.save(&Secrets::from([("a2".into(), token("new"))])).unwrap();
        assert_eq!(store.users(), ["a2", VAULT_INDEX]);
        assert_eq!(store.value(VAULT_INDEX).as_deref(), Some(r#"["a2"]"#));
    }

    /// After a failed save, the next one starts from what the store holds.
    #[test]
    fn a_failed_save_is_written_whole_next_time() {
        let store = FakeStore::with(&[]);
        let vault = vault(&store);
        vault.save(&both()).unwrap();
        store.entries.lock().unwrap().remove("a1");
        store.fail_after(0);
        assert!(vault.save(&both()).is_ok(), "nothing changed, nothing written");
        let mut renewed = both();
        renewed.insert("a2".into(), token("new"));
        assert!(vault.save(&renewed).is_err());
        *store.fail_at.lock().unwrap() = None;
        vault.save(&renewed).unwrap();
        assert_eq!(store.value("a1").as_deref(), Some(r#"{"token":"t1"}"#));
    }

    /// A save before any read moves the old single entry over first.
    #[test]
    fn a_save_before_a_read_keeps_the_other_accounts() {
        let store = FakeStore::with(&[(VAULT_LEGACY, LEGACY)]);
        let mut changed = both();
        changed.insert("a3".into(), token("t3"));
        vault(&store).save(&changed).unwrap();
        assert_eq!(vault(&store).load().unwrap(), changed);
        assert!(store.value(VAULT_LEGACY).is_none());
    }

    /// Erase removes every account's entry, the index and the old single entry.
    #[test]
    fn erase_removes_every_entry() {
        let store = FakeStore::with(&[(VAULT_LEGACY, LEGACY)]);
        store.fail_after(3);
        vault(&store).load().unwrap();
        *store.fail_at.lock().unwrap() = None;
        // A fresh launch: nothing known but the index.
        vault(&store).save(&Secrets::new()).unwrap();
        assert!(store.users().is_empty());
    }
}

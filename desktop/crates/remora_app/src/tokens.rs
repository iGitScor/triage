//! Every change to the tokens goes through one sequence (PERF-07, SEC-09): take the write lock, work out the new
//! tokens under the inbox lock, save them without it (the credential store can be slow), then apply them under the
//! inbox lock again. The write lock is held from the first step to the last, so two changes never race: the second
//! one starts from what the first one saved.

use crate::{Inbox, Secrets, Vault};
use chrono::{DateTime, Utc};
use remora_core::{Account, Brief, Message};
use remora_plugins::SourceSnapshot;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// The vault as the app reaches it: asynchronously, so a slow credential store never blocks the inbox.
#[async_trait::async_trait]
pub trait AsyncVault: Send + Sync {
    async fn load(&self) -> Result<Secrets, String>;
    async fn save(&self, secrets: Secrets) -> Result<(), String>;
}

/// A vault called on a blocking thread of the async runtime: the app's.
pub struct Blocking(pub Arc<dyn Vault>);

#[async_trait::async_trait]
impl AsyncVault for Blocking {
    async fn load(&self) -> Result<Secrets, String> {
        let vault = self.0.clone();
        tokio::task::spawn_blocking(move || vault.load()).await.map_err(|e| e.to_string())?
    }

    async fn save(&self, secrets: Secrets) -> Result<(), String> {
        let vault = self.0.clone();
        tokio::task::spawn_blocking(move || vault.save(&secrets)).await.map_err(|e| e.to_string())?
    }
}

/// A vault called in place: tests.
pub struct InPlace(pub Arc<dyn Vault>);

#[async_trait::async_trait]
impl AsyncVault for InPlace {
    async fn load(&self) -> Result<Secrets, String> {
        self.0.load()
    }

    async fn save(&self, secrets: Secrets) -> Result<(), String> {
        self.0.save(&secrets)
    }
}

/// The tokens' write lock and the vault they are saved to. The inbox is given its own handle on the same vault.
pub struct Tokens {
    writes: Mutex<()>,
    vault: Arc<dyn AsyncVault>,
}

impl Tokens {
    pub fn new(vault: Arc<dyn AsyncVault>) -> Self {
        Tokens { writes: Mutex::new(()), vault }
    }

    /// The one sequence: `change` works out every account's tokens (or refuses), they are saved, then `finish`
    /// applies them. A refusal or a failed save leaves the vault and the inbox as they were.
    pub async fn change_secrets<T>(
        &self,
        inbox: &Mutex<Inbox>,
        change: impl FnOnce(&mut Inbox) -> Result<Secrets, Message>,
        finish: impl FnOnce(&mut Inbox, Secrets) -> Result<T, Message>,
    ) -> Result<T, Message> {
        let _writes = self.writes.lock().await;
        let all = change(&mut *inbox.lock().await)?;
        self.vault.save(all.clone()).await?;
        finish(&mut *inbox.lock().await, all)
    }

    /// Reads the tokens once per launch, without the inbox lock. Kept only if nothing read them meanwhile.
    pub async fn load_once(&self, inbox: &Mutex<Inbox>) {
        if !inbox.lock().await.needs_secrets() {
            return;
        }
        if let Ok(secrets) = self.vault.load().await {
            inbox.lock().await.provide_secrets(secrets);
        }
    }

    /// Saves an account whose first fetch worked (`Inbox::prepare_connect`), or updates the same account connected
    /// again.
    pub async fn connect(
        &self,
        inbox: &Mutex<Inbox>,
        account: Account,
        secrets: HashMap<String, String>,
        snapshot: SourceSnapshot,
        now: DateTime<Utc>,
    ) -> Result<(), Message> {
        let (probe, identity) = (account.clone(), snapshot.identity.clone());
        self.change_secrets(
            inbox,
            |inbox| {
                // Connected again (same person, tool and server): the token goes to the existing account.
                let key = inbox.connected_id(&probe, &identity);
                inbox.secrets_after(|all| {
                    all.insert(key, secrets);
                })
            },
            |inbox, saved| {
                inbox.finish_connect_saved(account, saved, snapshot, now);
                Ok(())
            },
        )
        .await
    }

    /// Saves the assistant tested with `Inbox::prepare_assistant_connect`, in place of any other, with the brief it
    /// wrote as its test, if it wrote one.
    pub async fn connect_assistant(
        &self,
        inbox: &Mutex<Inbox>,
        account: Account,
        secrets: HashMap<String, String>,
        brief: Option<Brief>,
    ) -> Result<(), Message> {
        let probe = account.clone();
        self.change_secrets(
            inbox,
            |inbox| inbox.assistant_secrets(&probe, secrets),
            |inbox, saved| {
                inbox.finish_assistant_connect_saved(account, saved);
                if brief.is_some() {
                    inbox.store_brief(brief);
                }
                Ok(())
            },
        )
        .await
    }

    /// The new token of an account still connected, checked by a fetch (`snapshot`, whose items it stores) or, for
    /// the assistant, by a made-up item (no snapshot).
    pub async fn reconnect(
        &self,
        inbox: &Mutex<Inbox>,
        account_id: &str,
        secrets: HashMap<String, String>,
        snapshot: Option<SourceSnapshot>,
        now: DateTime<Utc>,
    ) -> Result<(), Message> {
        self.change_secrets(
            inbox,
            |inbox| {
                if !inbox.is_connected(account_id) {
                    return Err("This account is no longer connected.".into());
                }
                inbox.secrets_after(|all| {
                    all.insert(account_id.to_string(), secrets);
                })
            },
            |inbox, saved| match snapshot {
                Some(snapshot) => inbox.finish_reconnect_saved(account_id, saved, snapshot, now),
                None => inbox.finish_reconnect_secrets(account_id, saved),
            },
        )
        .await
    }

    pub async fn disconnect(&self, inbox: &Mutex<Inbox>, account_id: &str) -> Result<(), Message> {
        self.change_secrets(
            inbox,
            |inbox| {
                inbox.secrets_after(|all| {
                    all.remove(account_id);
                })
            },
            |inbox, saved| {
                inbox.disconnect_saved(account_id, saved);
                Ok(())
            },
        )
        .await
    }

    /// Settings → Privacy → Erase local data: every token, then every account and file.
    pub async fn erase(&self, inbox: &Mutex<Inbox>) -> Result<(), Message> {
        self.change_secrets(inbox, |_| Ok(Secrets::new()), |inbox, _| inbox.erase_local_data_saved()).await
    }

    /// Secrets a plugin renewed during a refresh (a Microsoft sign-in's refresh token), for accounts still here. A
    /// failed save keeps the old ones, still valid.
    pub async fn save_renewed(&self, inbox: &Mutex<Inbox>) -> Result<(), Message> {
        let renewed = inbox.lock().await.take_renewed_secrets();
        if renewed.is_empty() {
            return Ok(());
        }
        self.change_secrets(
            inbox,
            |inbox| {
                inbox.secrets_after(|all| {
                    for (account_id, secrets) in renewed {
                        all.entry(account_id).or_default().extend(secrets);
                    }
                })
            },
            |inbox, saved| {
                inbox.renewed_saved(saved);
                Ok(())
            },
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Managed, MemoryVault};
    use futures::executor::block_on;
    use std::sync::atomic::{AtomicBool, Ordering};

    fn token(value: &str) -> HashMap<String, String> {
        HashMap::from([("token".to_string(), value.to_string())])
    }

    fn account(id: &str, plugin_id: &str) -> Account {
        Account { id: id.into(), plugin_id: plugin_id.into(), name: None, settings: HashMap::new(), identity: None }
    }

    /// Two accounts, a1 and a2, with their tokens in `vault`.
    fn connected(vault: Arc<MemoryVault>) -> Mutex<Inbox> {
        vault.save(&Secrets::from([("a1".into(), token("t1")), ("a2".into(), token("t2"))])).unwrap();
        let mut inbox = Inbox::in_memory(vault, Managed::default());
        *inbox.accounts_mut() = vec![account("a1", "github"), account("a2", "linear")];
        Mutex::new(inbox)
    }

    /// A vault whose first save waits until `open`, and that remembers every save; or refuses them all.
    struct Gated {
        vault: Arc<MemoryVault>,
        gate: tokio::sync::Notify,
        gated: AtomicBool,
        refuses: bool,
        saves: std::sync::Mutex<Vec<Secrets>>,
    }

    impl Gated {
        fn new(vault: Arc<MemoryVault>, gated: bool, refuses: bool) -> Arc<Self> {
            Arc::new(Gated {
                vault,
                gate: tokio::sync::Notify::new(),
                gated: AtomicBool::new(gated),
                refuses,
                saves: Default::default(),
            })
        }

        fn saves(&self) -> Vec<Secrets> {
            self.saves.lock().unwrap().clone()
        }
    }

    #[async_trait::async_trait]
    impl AsyncVault for Gated {
        async fn load(&self) -> Result<Secrets, String> {
            self.vault.load()
        }

        async fn save(&self, secrets: Secrets) -> Result<(), String> {
            if self.gated.swap(false, Ordering::SeqCst) {
                self.gate.notified().await;
            }
            if self.refuses {
                return Err("The credential store is locked.".into());
            }
            self.saves.lock().unwrap().push(secrets.clone());
            self.vault.save(&secrets)
        }
    }

    /// PERF-07, SEC-09: a second change that starts while the first one saves waits for it, and starts from what it
    /// saved. Without the write lock it would start from the old tokens and put back the disconnected account's.
    #[test]
    fn a_second_change_waits_for_the_first_save() {
        let vault = Arc::new(MemoryVault::default());
        let inbox = connected(vault.clone());
        let port = Gated::new(vault.clone(), true, false);
        let tokens = Tokens::new(port.clone());
        block_on(async {
            let first = tokens.disconnect(&inbox, "a1");
            let second = tokens.reconnect(&inbox, "a2", token("new"), None, Utc::now());
            let open = async { port.gate.notify_one() };
            let (first, second, ()) = futures::join!(first, second, open);
            first.unwrap();
            second.unwrap();
        });
        let expected = Secrets::from([("a2".into(), token("new"))]);
        assert_eq!(vault.load().unwrap(), expected, "neither change is lost");
        assert_eq!(
            port.saves(),
            [Secrets::from([("a2".into(), token("t2"))]), expected.clone()],
            "one after the other"
        );
        let mut inbox = block_on(inbox.lock());
        assert_eq!(inbox.secrets_after(|_| {}).unwrap(), expected, "the inbox holds what was saved");
        assert_eq!(inbox.accounts().iter().map(|a| a.id.as_str()).collect::<Vec<_>>(), ["a2"]);
    }

    /// An account disconnected before its reconnect is saved: nothing is written, a source or the assistant alike.
    #[test]
    fn a_reconnect_checks_the_account_is_still_connected() {
        let vault = Arc::new(MemoryVault::default());
        let inbox = connected(vault.clone());
        let port = Gated::new(vault.clone(), false, false);
        let tokens = Tokens::new(port.clone());
        block_on(tokens.disconnect(&inbox, "a1")).unwrap();
        let snapshot = SourceSnapshot { identity: "alice".into(), items: vec![], remarks: vec![] };
        for snapshot in [Some(snapshot), None] {
            let refused = block_on(tokens.reconnect(&inbox, "a1", token("new"), snapshot, Utc::now()));
            assert_eq!(refused.err().map(|e| e.text).as_deref(), Some("This account is no longer connected."));
        }
        assert_eq!(port.saves().len(), 1, "only the disconnect saved");
        assert!(!vault.load().unwrap().contains_key("a1"), "the token doesn't come back");
    }

    /// A save the vault refuses changes nothing in the inbox.
    #[test]
    fn a_failed_save_changes_nothing() {
        let vault = Arc::new(MemoryVault::default());
        let inbox = connected(vault.clone());
        let tokens = Tokens::new(Gated::new(vault.clone(), false, true));
        assert!(block_on(tokens.disconnect(&inbox, "a1")).is_err());
        assert!(block_on(tokens.erase(&inbox)).is_err());
        let mut inbox = block_on(inbox.lock());
        assert_eq!(inbox.accounts().len(), 2);
        assert_eq!(inbox.secrets_after(|_| {}).unwrap().len(), 2);
    }

    /// The tokens are read once, and not over ones the inbox already has.
    #[test]
    fn the_tokens_are_read_once() {
        let vault = Arc::new(MemoryVault::default());
        let inbox = connected(vault.clone());
        let tokens = Tokens::new(Arc::new(InPlace(vault.clone())));
        block_on(tokens.load_once(&inbox));
        vault.save(&Secrets::new()).unwrap();
        block_on(tokens.load_once(&inbox));
        assert_eq!(block_on(inbox.lock()).secrets_after(|_| {}).unwrap().len(), 2);
    }
}

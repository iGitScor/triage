//! The accounts service: the connected accounts, their tokens and their sessions. Every change to the list is saved
//! here, so the file and the list never disagree. Ported from the macOS app's `AccountsService`.

use super::persistence::{Persistence, ACCOUNTS, CACHE, STATES};
use super::{Inbox, SignInPlan};
use crate::session::AccountSession;
use crate::{Secrets, Vault};
use chrono::{DateTime, Utc};
use remora_core::{host_matches, Account, InboxBundle, InboxItem, Message};
use remora_plugins::{
    claude, oauth, registry, AccountCache, AssistantPlugin, ETagStore, GuardedHttpClient, HttpClient, PluginError,
    Request, SourcePlugin, SourceSnapshot,
};
use std::collections::HashMap;
use std::sync::Arc;

/// The tokens were asked for before the vault could be read.
pub const TOKENS_NOT_READ: &str = "The saved tokens haven't been read yet.";

pub struct AccountsService {
    list: Vec<Account>,
    vault: Arc<dyn Vault>,
    /// Every account's tokens, read from the vault once per launch (`Tokens::load_once`).
    secrets: Option<Secrets>,
    /// Why the last read failed, while nothing was read.
    unreadable: Option<Message>,
    /// Per account, in memory: what a refresh may reuse from the last one (PERF-09).
    sessions: HashMap<String, AccountSession>,
    persistence: Persistence,
}

/// The accounts, read as a list.
impl std::ops::Deref for AccountsService {
    type Target = [Account];

    fn deref(&self) -> &[Account] {
        &self.list
    }
}

impl AccountsService {
    pub(super) fn new(list: Vec<Account>, vault: Arc<dyn Vault>, persistence: Persistence) -> Self {
        AccountsService { list, vault, secrets: None, unreadable: None, sessions: HashMap::new(), persistence }
    }

    pub fn find(&self, id: &str) -> Option<&Account> {
        self.list.iter().find(|a| a.id == id)
    }

    pub fn contains(&self, id: &str) -> bool {
        self.find(id).is_some()
    }

    pub fn vault(&self) -> Arc<dyn Vault> {
        self.vault.clone()
    }

    // MARK: Tokens

    /// Every account's tokens, once `Tokens::load_once` read them. Never read here: the vault can be slow, and this
    /// runs under the inbox lock (CODE-07).
    pub fn secrets(&self) -> Result<&Secrets, Message> {
        self.secrets.as_ref().ok_or_else(|| self.unreadable.clone().unwrap_or_else(|| TOKENS_NOT_READ.into()))
    }

    /// Why the vault couldn't be read, said by whatever needs the tokens until a later read works.
    pub fn secrets_unreadable(&mut self, error: Message) {
        self.unreadable = Some(error);
    }

    /// The tokens haven't been read yet (once per launch).
    pub fn needs_secrets(&self) -> bool {
        self.secrets.is_none()
    }

    /// Tokens read from the vault outside the lock; kept only if nothing read them meanwhile.
    pub fn provide_secrets(&mut self, secrets: Secrets) {
        self.unreadable = None;
        self.secrets.get_or_insert(secrets);
    }

    /// Every account's tokens after `change`, to save, then pass to `secrets_saved`.
    pub fn secrets_after(&mut self, change: impl FnOnce(&mut Secrets)) -> Result<Secrets, Message> {
        let mut all = self.secrets()?.clone();
        change(&mut all);
        Ok(all)
    }

    /// The tokens as the vault now holds them.
    pub fn secrets_saved(&mut self, saved: Secrets) {
        self.secrets = Some(saved);
    }

    /// The tokens with `account` as the only assistant.
    pub fn with_only_assistant(
        &mut self,
        account: &Account,
        secrets: HashMap<String, String>,
    ) -> Result<Secrets, Message> {
        let previous: Vec<String> =
            self.list.iter().filter(|a| claude::is_assistant(&a.plugin_id)).map(|a| a.id.clone()).collect();
        self.secrets_after(|all| {
            for id in &previous {
                all.remove(id);
            }
            all.insert(account.id.clone(), secrets);
        })
    }

    /// Secrets the plugins renewed during the last fetches (an OAuth refresh token rotated), for accounts still here.
    pub fn take_renewed(&mut self) -> Vec<(String, HashMap<String, String>)> {
        AccountSession::take_renewed(&self.sessions).into_iter().filter(|(id, _)| self.contains(id)).collect()
    }

    /// What a refresh of this account may reuse: a new session when its settings or token changed.
    pub fn session(
        &mut self,
        account_id: &str,
        settings: &HashMap<String, String>,
        secrets: &HashMap<String, String>,
    ) -> (Arc<AccountCache>, Arc<ETagStore>) {
        AccountSession::current(&mut self.sessions, account_id, settings, secrets)
    }

    // MARK: The list, saved on every change

    pub fn rename(&mut self, account_id: &str, name: &str) {
        if let Some(account) = self.list.iter_mut().find(|a| a.id == account_id) {
            account.name = Some(name.trim().to_string()).filter(|n| !n.is_empty());
        }
        self.save();
    }

    /// A new account (the same one connected again keeps its place: `existing`).
    pub fn add(&mut self, account: Account) {
        self.list.push(account);
        self.save();
    }

    pub fn set_name(&mut self, account_id: &str, name: String) {
        if let Some(account) = self.list.iter_mut().find(|a| a.id == account_id) {
            account.name = Some(name);
            self.save();
        }
    }

    /// Who the token belongs to, as the last fetch said.
    pub fn set_identity(&mut self, account_id: &str, identity: String) {
        if let Some(account) = self.list.iter_mut().find(|a| a.id == account_id) {
            account.identity = Some(identity);
            self.save();
        }
    }

    /// One assistant at a time: a new one replaces the previous one.
    pub fn replace_assistant(&mut self, account: Account) {
        self.list.retain(|a| !claude::is_assistant(&a.plugin_id));
        self.add(account);
    }

    /// The account and what its session kept.
    pub fn remove(&mut self, account_id: &str) {
        self.list.retain(|a| a.id != account_id);
        self.sessions.remove(account_id);
        self.save();
    }

    /// The account already connected for the same person on the same tool and server, as on macOS
    /// (`isSame(as:identity:)`): connecting it again updates that one instead of adding a duplicate.
    pub fn existing(&self, account: &Account, identity: &str) -> Option<&Account> {
        if identity.is_empty() {
            return None;
        }
        // "gitlab.acme.io", "https://gitlab.acme.io/" and an empty field (the default host) compare by host, and the
        // path: two Azure DevOps organizations share dev.azure.com.
        let host = |account: &Account| {
            let value = account.settings.get("host").map(|h| h.trim()).filter(|h| !h.is_empty())?;
            let rest = value.split_once("://").map_or(value, |(_, rest)| rest);
            let rest = rest.split(['?', '#']).next().unwrap_or_default().trim_end_matches('/');
            let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
            let host = host.split(':').next().unwrap_or_default();
            Some(format!("{host}/{path}").to_lowercase())
        };
        self.list.iter().find(|saved| {
            saved.plugin_id == account.plugin_id
                && saved.identity.as_deref().is_some_and(|known| known.eq_ignore_ascii_case(identity))
                && host(saved) == host(account)
        })
    }

    /// Replaces the whole list: the demo.
    pub(super) fn replace_all(&mut self, list: Vec<Account>) {
        self.list = list;
    }

    fn save(&self) {
        self.persistence.save(ACCOUNTS, &self.list);
    }

    /// The list, for tests to arrange.
    #[cfg(test)]
    pub(crate) fn list_mut(&mut self) -> &mut Vec<Account> {
        &mut self.list
    }
}

// MARK: The inbox's accounts

// The credential store can be slow: the app reads and writes it without holding the inbox lock (PERF-07), through
// `Tokens` (tokens.rs): `needs_secrets` and `provide_secrets` for the read, then `secrets_after` (or a method built on
// it) and a `…_saved` method for each change. Those are for `Tokens` alone.
impl Inbox {
    #[cfg(test)]
    pub(crate) fn vault(&self) -> Arc<dyn Vault> {
        self.accounts.vault()
    }

    /// The tokens haven't been read yet (once per launch).
    pub(crate) fn needs_secrets(&self) -> bool {
        self.accounts.needs_secrets()
    }

    /// Tokens read from the vault outside the lock; kept only if nothing read them meanwhile.
    pub(crate) fn provide_secrets(&mut self, secrets: Secrets) {
        self.accounts.provide_secrets(secrets);
    }

    /// The vault couldn't be read: every account says why until a later read works.
    pub(crate) fn secrets_unreadable(&mut self, error: &Message) {
        self.accounts.secrets_unreadable(error.clone());
        for account in self.accounts.iter() {
            self.sync.refused(&account.id, error.clone());
        }
    }

    /// Secrets the plugins renewed during the last fetches (an OAuth refresh token rotated), for accounts still
    /// here: `Tokens::save_renewed` saves them, outside the lock.
    pub(crate) fn take_renewed_secrets(&mut self) -> Vec<(String, HashMap<String, String>)> {
        self.accounts.take_renewed()
    }

    /// Every account's tokens with the renewed ones in, once saved.
    pub(crate) fn renewed_saved(&mut self, saved: Secrets) {
        self.accounts.secrets_saved(saved);
    }

    /// Every account's tokens after `change`, to save, then pass to a `…_saved` method.
    pub(crate) fn secrets_after(&mut self, change: impl FnOnce(&mut Secrets)) -> Result<Secrets, Message> {
        self.accounts.secrets_after(change)
    }

    /// Checks the settings by building the plugin; the caller fetches once, then `Tokens::connect` saves it.
    pub fn prepare_connect(
        &self,
        plugin_id: &str,
        name: Option<String>,
        settings: HashMap<String, String>,
        secrets: &HashMap<String, String>,
        http: Arc<dyn HttpClient>,
    ) -> Result<(Account, Box<dyn SourcePlugin>), Message> {
        let manifest = registry::manifest(plugin_id).ok_or_else(|| format!("Unknown plugin “{plugin_id}”."))?;
        if let Some(refusal) = self.policy().refusal(&manifest) {
            return Err(refusal.into());
        }
        let account = self.privacy().with_policy_settings(&Account {
            id: uuid::Uuid::new_v4().to_string(),
            plugin_id: plugin_id.into(),
            name: name.map(|n| n.trim().to_string()).filter(|n| !n.is_empty()),
            settings,
            identity: None,
        });
        let guarded: Arc<dyn HttpClient> =
            Arc::new(GuardedHttpClient::new(http, registry::allowed_hosts(&account, &manifest)));
        let plugin = registry::make(&account, secrets, guarded, Default::default()).map_err(|e| e.message())?;
        Ok((account, plugin))
    }

    /// What a browser sign-in needs: the plugin's `SignIn`, the settings it fills in (the account's own when signing
    /// in again), and the HTTP client limited to the plugin's hosts. The page the browser opens must be one of them.
    pub fn prepare_sign_in(
        &self,
        plugin_id: &str,
        account_id: Option<&str>,
        settings: HashMap<String, String>,
        http: Arc<dyn HttpClient>,
    ) -> Result<SignInPlan, Message> {
        let manifest = registry::manifest(plugin_id).ok_or_else(|| format!("Unknown plugin “{plugin_id}”."))?;
        let sign_in = manifest.sign_in.clone().ok_or_else(|| format!("{} has no sign-in.", manifest.name))?;
        if let Some(refusal) = self.policy().refusal(&manifest) {
            return Err(refusal.into());
        }
        let settings = match account_id {
            Some(id) => self.accounts.find(id).ok_or("This account is no longer connected.")?.settings.clone(),
            None => settings,
        };
        let account = self.privacy().with_policy_settings(&Account {
            id: String::new(),
            plugin_id: plugin_id.into(),
            name: None,
            settings,
            identity: None,
        });
        let hosts = registry::allowed_hosts(&account, &manifest);
        let page = oauth::fill(&sign_in.authorize_url, &account.settings).map_err(|e| e.message())?;
        let host = Request::get(page).host();
        if !host_matches(&host, &hosts) {
            return Err(PluginError::BlockedHost(host).message());
        }
        Ok(SignInPlan { sign_in, settings: account.settings, http: Arc::new(GuardedHttpClient::new(http, hosts)) })
    }

    /// Connecting an assistant: the caller tests it once with the plugin (a brief, or a made-up item
    /// when the whole-inbox brief is off, so no inbox content leaves yet), then `Tokens::connect_assistant` saves it.
    pub fn prepare_assistant_connect(
        &self,
        plugin_id: &str,
        name: Option<String>,
        settings: HashMap<String, String>,
        secrets: &HashMap<String, String>,
        http: Arc<dyn HttpClient>,
        language: &str,
    ) -> Result<(Account, Box<dyn AssistantPlugin>), Message> {
        let manifest = claude::assistant_manifests()
            .into_iter()
            .find(|m| m.id == plugin_id)
            .ok_or_else(|| format!("Unknown plugin “{plugin_id}”."))?;
        let privacy = self.privacy();
        let account = privacy.with_policy_settings(&Account {
            id: uuid::Uuid::new_v4().to_string(),
            plugin_id: plugin_id.into(),
            name: name.map(|n| n.trim().to_string()).filter(|n| !n.is_empty()),
            settings,
            identity: None,
        });
        if let Some(refusal) = privacy.compliance().account_refusal(&manifest, &account.settings) {
            return Err(refusal.into());
        }
        let guarded: Arc<dyn HttpClient> =
            Arc::new(GuardedHttpClient::new(http, registry::allowed_hosts(&account, &manifest)));
        let constrained = privacy.constrained(&account, &manifest)?;
        let plugin = claude::make_assistant(&constrained, secrets, guarded, language).map_err(|e| e.message())?;
        Ok((account, plugin))
    }

    /// The tokens with `account` as the only assistant.
    pub(crate) fn assistant_secrets(
        &mut self,
        account: &Account,
        secrets: HashMap<String, String>,
    ) -> Result<Secrets, Message> {
        self.accounts.with_only_assistant(account, secrets)
    }

    /// One assistant at a time: a new one replaces the previous one, and its token.
    pub(crate) fn finish_assistant_connect_saved(&mut self, account: Account, saved: Secrets) {
        self.accounts.secrets_saved(saved);
        self.accounts.replace_assistant(account);
    }

    /// A made-up item to test an assistant with when the whole-inbox brief is off: nothing from the inbox.
    pub fn connection_test(now: DateTime<Utc>) -> InboxItem {
        InboxItem {
            id: "remora/connection-test".into(),
            account_id: "remora".into(),
            plugin_id: "remora".into(),
            bundle: InboxBundle::reminders(),
            title: "Connection test".into(),
            context: "Remora".into(),
            preview: None,
            url: None,
            app_url: None,
            author: None,
            participants: vec![],
            badges: vec![],
            date: now,
            needs_action: false,
            priority: None,
            due: None,
            expires: None,
            changes: None,
            suggested_people: None,
            recurrence: None,
            reactions: None,
        }
    }

    /// The account already connected for the same person on the same tool and server (`AccountsService::existing`).
    pub fn existing_account(&self, account: &Account, identity: &str) -> Option<&Account> {
        self.accounts.existing(account, identity)
    }

    /// The id a newly connected account's token is kept under: the existing account's when it is the same one.
    pub(crate) fn connected_id(&self, account: &Account, identity: &str) -> String {
        self.existing_account(account, identity).map_or_else(|| account.id.clone(), |a| a.id.clone())
    }

    /// Saves an account whose first fetch worked. Its items arrive silently.
    pub(crate) fn finish_connect_saved(
        &mut self,
        mut account: Account,
        saved: Secrets,
        snapshot: SourceSnapshot,
        now: DateTime<Utc>,
    ) {
        self.accounts.secrets_saved(saved);
        // The same account connected again: it keeps its id, so Done, snoozes and pins stay; a new name replaces
        // the old one, and its items come under its id.
        if let Some(existing) = self.existing_account(&account, &snapshot.identity).map(|a| a.id.clone()) {
            let mut snapshot = snapshot;
            for item in &mut snapshot.items {
                item.account_id = existing.clone();
                if let Some(rest) = item.id.strip_prefix(account.id.as_str()) {
                    item.id = format!("{existing}{rest}");
                }
            }
            if let Some(name) = account.name {
                self.accounts.set_name(&existing, name);
            }
            self.sync.retry(&existing);
            self.apply(vec![(existing, Ok(snapshot))], now);
            return;
        }
        account.identity = Some(snapshot.identity.clone());
        let id = account.id.clone();
        self.accounts.add(account);
        self.apply(vec![(id, Ok(snapshot))], now);
    }

    /// Reconnect: the same account with a new token, so its Done, snoozes and pins stay. The caller
    /// fetches once with the plugin, then `Tokens::reconnect` saves it.
    pub fn prepare_reconnect(
        &self,
        account_id: &str,
        secrets: &HashMap<String, String>,
        http: Arc<dyn HttpClient>,
    ) -> Result<Box<dyn SourcePlugin>, Message> {
        let account = &self
            .privacy()
            .with_policy_settings(self.accounts.find(account_id).ok_or("This account is no longer connected.")?);
        let manifest =
            registry::manifest(&account.plugin_id).ok_or_else(|| format!("Unknown plugin “{}”.", account.plugin_id))?;
        if let Some(refusal) = self.policy().refusal(&manifest) {
            return Err(refusal.into());
        }
        let guarded: Arc<dyn HttpClient> =
            Arc::new(GuardedHttpClient::new(http, registry::allowed_hosts(account, &manifest)));
        registry::make(account, secrets, guarded, Default::default()).map_err(|e| e.message())
    }

    /// Whether the account is still here: a token is saved only for one that is (`Tokens::reconnect`).
    pub(crate) fn is_connected(&self, account_id: &str) -> bool {
        self.accounts.contains(account_id)
    }

    /// Saves the new token of an account whose fetch worked with it, and its items.
    pub(crate) fn finish_reconnect_saved(
        &mut self,
        account_id: &str,
        saved: Secrets,
        snapshot: SourceSnapshot,
        now: DateTime<Utc>,
    ) -> Result<(), Message> {
        if !self.is_connected(account_id) {
            return Err("This account is no longer connected.".into());
        }
        self.accounts.secrets_saved(saved);
        self.sync.retry(account_id);
        self.apply(vec![(account_id.to_string(), Ok(snapshot))], now);
        Ok(())
    }

    /// Reconnecting the assistant: the same account with a new key. The caller tests it once with a made-up item
    /// (no inbox content), then `Tokens::reconnect` saves the key.
    pub fn prepare_assistant_reconnect(
        &self,
        account_id: &str,
        secrets: &HashMap<String, String>,
        http: Arc<dyn HttpClient>,
        language: &str,
    ) -> Result<Box<dyn AssistantPlugin>, Message> {
        let account = self
            .accounts
            .find(account_id)
            .filter(|a| claude::is_assistant(&a.plugin_id))
            .ok_or("This account is no longer connected.")?;
        self.assistant_with(account, secrets, http, language)
    }

    /// The tokens are saved: an account reconnected without a fetch (the assistant) uses them from now on.
    pub(crate) fn finish_reconnect_secrets(&mut self, account_id: &str, saved: Secrets) -> Result<(), Message> {
        if !self.is_connected(account_id) {
            return Err("This account is no longer connected.".into());
        }
        self.accounts.secrets_saved(saved);
        Ok(())
    }

    pub fn rename(&mut self, account_id: &str, name: &str) {
        self.accounts.rename(account_id, name);
    }

    pub(crate) fn disconnect_saved(&mut self, account_id: &str, saved: Secrets) {
        self.undo_point = None;
        self.accounts.secrets_saved(saved);
        self.accounts.remove(account_id);
        self.items.remove(account_id);
        self.recompute_links();
        self.sync.forget(account_id);
        self.prune();
        self.persistence.save(CACHE, &self.items);
        self.persistence.save(STATES, &self.states);
        self.enforce_ai_policy();
    }
}

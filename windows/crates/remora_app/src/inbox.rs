use crate::{JsonStore, Managed, Preferences, Secrets, Vault};
use chrono::{DateTime, Utc};
use remora_core::{
    notices, Account, CompliancePolicy, InboxAssembler, InboxBundle, InboxItem, InboxLayout, ItemState, Mark, Notice, NoticeKind,
    PluginManifest, Snooze, SnoozeMode, SnoozeReason, VerbClassifier,
};
use remora_plugins::{registry, GuardedHttpClient, HttpClient, PluginError, SourcePlugin, SourceSnapshot};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

const ACCOUNTS: &str = "accounts";
const CACHE: &str = "cache";
const STATES: &str = "states";
const REMINDERS: &str = "reminders";
const PREFERENCES: &str = "preferences";

/// One account's fetch, built under the lock and run without it.
pub struct FetchJob {
    pub account_id: String,
    pub plugin: Box<dyn SourcePlugin>,
}

pub type FetchResult = (String, Result<SourceSnapshot, PluginError>);

/// Runs fetches concurrently.
pub async fn run(jobs: Vec<FetchJob>) -> Vec<FetchResult> {
    futures::future::join_all(jobs.into_iter().map(|job| async move { (job.account_id, job.plugin.fetch().await) })).await
}

/// A source as the Settings screen lists it: its manifest, and why it can't be used, if it can't.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceInfo {
    pub manifest: PluginManifest,
    pub refusal: Option<String>,
}

/// A connected account with what Settings → Privacy shows about it.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountInfo {
    #[serde(flatten)]
    pub account: Account,
    pub plugin_name: String,
    pub hosts: Vec<String>,
    pub egress: String,
    pub allowed: bool,
    pub error: Option<String>,
}

/// Everything the inbox knows, and every action on it. Ported from the macOS app's `InboxModel`.
pub struct Inbox {
    store: Option<JsonStore>,
    vault: Arc<dyn Vault>,
    pub managed: Managed,
    pub preferences: Preferences,
    pub accounts: Vec<Account>,
    items: HashMap<String, Vec<InboxItem>>,
    pub states: HashMap<String, ItemState>,
    reminders: Vec<InboxItem>,
    errors: HashMap<String, String>,
    synced: HashSet<String>,
    secrets: Option<Secrets>,
    pub last_refresh: Option<DateTime<Utc>>,
}

impl Inbox {
    /// The real inbox, from its files and the credential store.
    pub fn open(store: JsonStore, vault: Arc<dyn Vault>, managed: Managed) -> Self {
        let items: HashMap<String, Vec<InboxItem>> = store.load(CACHE).unwrap_or_default();
        Inbox {
            preferences: store.load(PREFERENCES).unwrap_or_default(),
            accounts: store.load(ACCOUNTS).unwrap_or_default(),
            items: items.into_iter().map(|(id, items)| (id, items.into_iter().map(VerbClassifier::classify).collect())).collect(),
            states: store.load(STATES).unwrap_or_default(),
            reminders: store.load(REMINDERS).unwrap_or_default(),
            store: Some(store),
            vault,
            managed,
            errors: HashMap::new(),
            synced: HashSet::new(),
            secrets: None,
            last_refresh: None,
        }
    }

    /// An inbox that saves nothing: the demo, and tests.
    pub fn in_memory(vault: Arc<dyn Vault>, managed: Managed) -> Self {
        Inbox {
            store: None,
            vault,
            managed,
            preferences: Preferences::default(),
            accounts: vec![],
            items: HashMap::new(),
            states: HashMap::new(),
            reminders: vec![],
            errors: HashMap::new(),
            synced: HashSet::new(),
            secrets: None,
            last_refresh: None,
        }
    }

    // MARK: Reading

    pub fn policy(&self) -> CompliancePolicy {
        self.managed.policy(&self.preferences)
    }

    fn allows(&self, plugin_id: &str) -> bool {
        registry::manifest(plugin_id).is_some_and(|m| self.policy().allows(&m))
    }

    /// Items of allowed accounts, plus reminders.
    pub fn all_items(&self) -> Vec<InboxItem> {
        let mut all: Vec<InboxItem> = self
            .accounts
            .iter()
            .filter(|a| self.allows(&a.plugin_id))
            .flat_map(|a| self.items.get(&a.id).cloned().unwrap_or_default())
            .collect();
        all.extend(self.reminders.iter().cloned());
        all
    }

    pub fn item(&self, id: &str) -> Option<InboxItem> {
        self.all_items().into_iter().find(|i| i.id == id)
    }

    pub fn layout(&self, now: DateTime<Utc>, query: &str) -> InboxLayout {
        InboxAssembler { wake_on_activity: self.preferences.wake_on_activity }.layout(&self.all_items(), &self.states, now, query)
    }

    pub fn sources(&self) -> Vec<SourceInfo> {
        let policy = self.policy();
        registry::manifests().into_iter().map(|m| SourceInfo { refusal: policy.refusal(&m).map(str::to_string), manifest: m }).collect()
    }

    pub fn account_infos(&self) -> Vec<AccountInfo> {
        let policy = self.policy();
        self.accounts
            .iter()
            .filter_map(|account| {
                let manifest = registry::manifest(&account.plugin_id)?;
                Some(AccountInfo {
                    hosts: registry::allowed_hosts(account, &manifest),
                    egress: manifest.egress.description.clone(),
                    allowed: policy.allows(&manifest),
                    plugin_name: manifest.name,
                    error: self.errors.get(&account.id).cloned(),
                    account: account.clone(),
                })
            })
            .collect()
    }

    pub fn errors(&self) -> Vec<String> {
        self.errors.values().cloned().collect()
    }

    // MARK: Refreshing

    fn secrets(&mut self) -> Result<&Secrets, String> {
        if self.secrets.is_none() {
            self.secrets = Some(self.vault.load()?);
        }
        Ok(self.secrets.as_ref().unwrap())
    }

    /// One plugin per allowed account, each with an HTTP client limited to its declared hosts.
    /// Refused accounts get an error instead.
    pub fn fetch_jobs(&mut self, http: Arc<dyn HttpClient>) -> Vec<FetchJob> {
        let secrets = match self.secrets() {
            Ok(secrets) => secrets.clone(),
            Err(error) => {
                for account in &self.accounts {
                    self.errors.insert(account.id.clone(), error.clone());
                }
                return vec![];
            }
        };
        let policy = self.policy();
        let mut jobs = vec![];
        for account in self.accounts.clone() {
            let Some(manifest) = registry::manifest(&account.plugin_id) else { continue };
            if let Some(refusal) = policy.refusal(&manifest) {
                self.errors.insert(account.id.clone(), refusal.to_string());
                continue;
            }
            let guarded: Arc<dyn HttpClient> = Arc::new(GuardedHttpClient::new(http.clone(), registry::allowed_hosts(&account, &manifest)));
            match registry::make(&account, secrets.get(&account.id).unwrap_or(&HashMap::new()), guarded) {
                Ok(plugin) => jobs.push(FetchJob { account_id: account.id.clone(), plugin }),
                Err(error) => {
                    self.errors.insert(account.id.clone(), error.to_string());
                }
            }
        }
        jobs
    }

    /// Stores what came back and says what deserves a notification. The first sync of an account is
    /// silent; failed accounts keep their last items.
    pub fn apply(&mut self, results: Vec<FetchResult>, now: DateTime<Utc>) -> Vec<Notice> {
        let mut out = vec![];
        for (account_id, result) in results {
            match result {
                Err(error) => {
                    self.errors.insert(account_id, error.to_string());
                }
                Ok(snapshot) => {
                    self.errors.remove(&account_id);
                    let items: Vec<InboxItem> = snapshot.items.into_iter().map(VerbClassifier::classify).collect();
                    let previous = self.synced.contains(&account_id).then(|| self.items.get(&account_id).cloned().unwrap_or_default());
                    for notice in notices(previous.as_deref(), &items) {
                        let wanted = if notice.kind == NoticeKind::Arrival { self.preferences.notify_arrivals } else { self.preferences.notify_status_changes };
                        if wanted {
                            out.push(notice);
                        }
                    }
                    self.wake_touched(&items);
                    self.synced.insert(account_id.clone());
                    if let Some(account) = self.accounts.iter_mut().find(|a| a.id == account_id) {
                        account.identity = Some(snapshot.identity);
                    }
                    self.items.insert(account_id, items);
                }
            }
        }
        self.last_refresh = Some(now);
        self.prune();
        self.persist(ACCOUNTS, &self.accounts);
        self.persist(CACHE, &self.items);
        self.persist(STATES, &self.states);
        out
    }

    /// Hidden snoozes end early on new activity, when that preference is on.
    fn wake_touched(&mut self, items: &[InboxItem]) {
        if !self.preferences.wake_on_activity {
            return;
        }
        for item in items {
            if let Some(state) = self.states.get_mut(&item.id) {
                if state.snooze.as_ref().is_some_and(|s| s.mode == SnoozeMode::Hide && s.fingerprint != item.fingerprint()) {
                    state.snooze = None;
                }
            }
        }
        self.states.retain(|_, state| !state.is_empty());
    }

    /// Ends snoozes whose time has come: the item comes back marked as a reminder, with a notification.
    pub fn tick(&mut self, now: DateTime<Utc>) -> Vec<Notice> {
        let items = self.all_items();
        let mut out = vec![];
        for (id, state) in self.states.iter_mut() {
            let Some(snooze) = state.snooze.clone().filter(|s| s.until <= now) else { continue };
            state.snooze = None;
            state.reminded_at = Some(now);
            state.done = None;
            if let Some(item) = items.iter().find(|i| &i.id == id) {
                let reminder = item.bundle == InboxBundle::reminders();
                out.push(Notice {
                    kind: NoticeKind::Reminder,
                    item_id: id.clone(),
                    title: if reminder { "Reminder" } else { "Back in your inbox" }.into(),
                    subtitle: item.context.clone(),
                    body: [Some(item.title.clone()), snooze.note].into_iter().flatten().collect::<Vec<_>>().join("\n"),
                    url: item.url.clone(),
                });
            }
        }
        if !out.is_empty() {
            self.persist(STATES, &self.states);
        }
        out
    }

    /// Forgets states of items that no longer exist, except snoozes (their item may come back).
    fn prune(&mut self) {
        let known: HashSet<String> = self.all_items().into_iter().map(|i| i.id).collect();
        self.states.retain(|id, state| known.contains(id) || state.snooze.is_some());
    }

    // MARK: Actions

    fn update(&mut self, id: &str, change: impl FnOnce(&mut ItemState)) {
        let mut state = self.states.remove(id).unwrap_or_default();
        change(&mut state);
        if !state.is_empty() {
            self.states.insert(id.to_string(), state);
        }
        self.persist(STATES, &self.states);
    }

    /// Opening an item clears its reminder chip.
    pub fn opened(&mut self, id: &str) {
        self.update(id, |s| s.reminded_at = None);
    }

    /// Done until it changes. A reminder marked done is gone for good.
    pub fn toggle_done(&mut self, id: &str, now: DateTime<Utc>) {
        let Some(item) = self.item(id) else { return };
        let marking = self.states.get(id).and_then(|s| s.done.as_ref()).is_none();
        if item.bundle == InboxBundle::reminders() && marking {
            self.reminders.retain(|r| r.id != id);
            self.states.remove(id);
            self.persist(REMINDERS, &self.reminders);
            self.persist(STATES, &self.states);
            return;
        }
        let fingerprint = item.fingerprint();
        self.update(id, |s| {
            s.done = marking.then_some(Mark { at: now, fingerprint, cleared_at: None });
            s.reminded_at = None;
            s.snooze = None;
        });
    }

    /// Empties the Done tab; cleared items still come back on new activity.
    pub fn clear_done(&mut self, now: DateTime<Utc>) {
        for state in self.states.values_mut() {
            if let Some(done) = state.done.as_mut() {
                done.cleared_at.get_or_insert(now);
            }
        }
        self.persist(STATES, &self.states);
    }

    pub fn toggle_pin(&mut self, id: &str) {
        self.update(id, |s| s.pinned = !s.pinned);
    }

    pub fn snooze(&mut self, id: &str, until: DateTime<Utc>, mode: SnoozeMode, reason: Option<SnoozeReason>, until_news: bool) {
        let Some(item) = self.item(id) else { return };
        let fingerprint = item.fingerprint();
        self.update(id, |s| {
            s.snooze = Some(Snooze { until, mode, note: None, fingerprint, reason, until_news: until_news.then_some(true) });
            s.done = None;
            s.reminded_at = None;
        });
    }

    pub fn unsnooze(&mut self, id: &str) {
        self.update(id, |s| s.snooze = None);
    }

    /// A reminder is an item of its own, hidden until its time.
    pub fn add_reminder(&mut self, title: &str, at: DateTime<Utc>, now: DateTime<Utc>) -> String {
        let id = format!("reminder/{}", uuid::Uuid::new_v4());
        self.reminders.push(InboxItem {
            id: id.clone(),
            account_id: "reminders".into(),
            plugin_id: "reminders".into(),
            bundle: InboxBundle::reminders(),
            title: title.trim().to_string(),
            context: "Reminder".into(),
            preview: None,
            url: None,
            app_url: None,
            author: None,
            participants: vec![],
            badges: vec![],
            date: now,
            needs_action: true,
            priority: None,
            due: None,
        });
        self.persist(REMINDERS, &self.reminders);
        self.snooze(&id, at, SnoozeMode::Hide, None, false);
        id
    }

    pub fn set_preferences(&mut self, preferences: Preferences) {
        self.preferences = preferences;
        self.persist(PREFERENCES, &self.preferences);
    }

    // MARK: Accounts

    /// Checks the settings by building the plugin; the caller fetches once, then calls `finish_connect`.
    pub fn prepare_connect(
        &self,
        plugin_id: &str,
        name: Option<String>,
        settings: HashMap<String, String>,
        secrets: &HashMap<String, String>,
        http: Arc<dyn HttpClient>,
    ) -> Result<(Account, Box<dyn SourcePlugin>), String> {
        let manifest = registry::manifest(plugin_id).ok_or_else(|| format!("Unknown plugin “{plugin_id}”."))?;
        if let Some(refusal) = self.policy().refusal(&manifest) {
            return Err(refusal.to_string());
        }
        let account = Account {
            id: uuid::Uuid::new_v4().to_string(),
            plugin_id: plugin_id.into(),
            name: name.map(|n| n.trim().to_string()).filter(|n| !n.is_empty()),
            settings,
            identity: None,
        };
        let guarded: Arc<dyn HttpClient> = Arc::new(GuardedHttpClient::new(http, registry::allowed_hosts(&account, &manifest)));
        let plugin = registry::make(&account, secrets, guarded).map_err(|e| e.to_string())?;
        Ok((account, plugin))
    }

    /// Saves an account whose first fetch worked. Its items arrive silently.
    pub fn finish_connect(&mut self, mut account: Account, secrets: HashMap<String, String>, snapshot: SourceSnapshot, now: DateTime<Utc>) -> Result<(), String> {
        let mut all = self.secrets()?.clone();
        all.insert(account.id.clone(), secrets);
        self.vault.save(&all)?;
        self.secrets = Some(all);
        account.identity = Some(snapshot.identity.clone());
        let id = account.id.clone();
        self.accounts.push(account);
        self.apply(vec![(id, Ok(snapshot))], now);
        Ok(())
    }

    pub fn rename(&mut self, account_id: &str, name: &str) {
        if let Some(account) = self.accounts.iter_mut().find(|a| a.id == account_id) {
            account.name = Some(name.trim().to_string()).filter(|n| !n.is_empty());
        }
        self.persist(ACCOUNTS, &self.accounts);
    }

    pub fn disconnect(&mut self, account_id: &str) -> Result<(), String> {
        let mut all = self.secrets()?.clone();
        all.remove(account_id);
        self.vault.save(&all)?;
        self.secrets = Some(all);
        self.accounts.retain(|a| a.id != account_id);
        self.items.remove(account_id);
        self.errors.remove(account_id);
        self.synced.remove(account_id);
        self.prune();
        self.persist(ACCOUNTS, &self.accounts);
        self.persist(CACHE, &self.items);
        self.persist(STATES, &self.states);
        Ok(())
    }

    /// Settings → Privacy → Erase local data: every account, file and token.
    pub fn erase_local_data(&mut self) -> Result<(), String> {
        self.vault.save(&Secrets::new())?;
        if let Some(store) = &self.store {
            store.erase().map_err(|e| e.to_string())?;
        }
        let (vault, managed, store) = (self.vault.clone(), self.managed.clone(), self.store.take());
        *self = Inbox::in_memory(vault, managed);
        self.store = store;
        Ok(())
    }

    fn persist<T: Serialize>(&self, name: &str, value: &T) {
        if let Some(store) = &self.store {
            if let Err(error) = store.save(name, value) {
                eprintln!("Remora: could not save {name}: {error}");
            }
        }
    }

    /// Demo items, as if an account had synced: see `demo.rs`.
    pub fn load_demo(&mut self, accounts: Vec<Account>, items: HashMap<String, Vec<InboxItem>>, states: HashMap<String, ItemState>) {
        self.accounts = accounts;
        self.items = items.into_iter().map(|(id, items)| (id, items.into_iter().map(VerbClassifier::classify).collect())).collect();
        self.synced = self.items.keys().cloned().collect();
        self.states = states;
        self.last_refresh = Some(Utc::now());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MemoryVault;
    use chrono::Duration;
    use remora_core::Badge;
    use remora_plugins::{Request, Response};

    struct NoNetwork;

    #[async_trait::async_trait]
    impl HttpClient for NoNetwork {
        async fn send(&self, request: Request) -> Result<Response, PluginError> {
            Err(PluginError::Network(format!("no network in tests ({})", request.url)))
        }
    }

    fn now() -> DateTime<Utc> {
        "2026-10-08T09:00:00Z".parse().unwrap()
    }

    fn item(id: &str, bundle: InboxBundle) -> InboxItem {
        InboxItem {
            id: id.into(),
            account_id: "a1".into(),
            plugin_id: "github".into(),
            bundle,
            title: format!("Item {id}"),
            context: "acme/app #1".into(),
            preview: None,
            url: Some("https://github.com/acme/app/pull/1".into()),
            app_url: None,
            author: None,
            participants: vec![],
            badges: vec![],
            date: now() - Duration::hours(1),
            needs_action: true,
            priority: None,
            due: None,
        }
    }

    fn inbox() -> Inbox {
        let mut inbox = Inbox::in_memory(Arc::new(MemoryVault::default()), Managed::default());
        inbox.accounts = vec![Account { id: "a1".into(), plugin_id: "github".into(), name: None, settings: HashMap::new(), identity: None }];
        inbox
    }

    fn snapshot(items: Vec<InboxItem>) -> Vec<FetchResult> {
        vec![("a1".into(), Ok(SourceSnapshot { identity: "alice".into(), items }))]
    }

    #[test]
    fn first_sync_is_silent_then_arrivals_and_status_changes_notify() {
        let mut inbox = inbox();
        assert!(inbox.apply(snapshot(vec![item("1", InboxBundle::reviews())]), now()).is_empty());
        assert_eq!(inbox.accounts[0].identity.as_deref(), Some("alice"));

        let mut approved = item("1", InboxBundle::authored());
        approved.badges = vec![Badge::new("approved", "Approved", remora_core::Tone::Accent).notifying("Approved")];
        let notices = inbox.apply(snapshot(vec![approved, item("2", InboxBundle::reviews())]), now());
        assert_eq!(notices.iter().map(|n| n.kind).collect::<Vec<_>>(), [NoticeKind::StatusChange, NoticeKind::Arrival]);

        inbox.preferences.notify_arrivals = false;
        assert!(inbox.apply(snapshot(vec![item("1", InboxBundle::authored()), item("2", InboxBundle::reviews()), item("3", InboxBundle::reviews())]), now()).is_empty());
    }

    #[test]
    fn a_failed_refresh_keeps_the_last_items_and_reports_the_error() {
        let mut inbox = inbox();
        inbox.apply(snapshot(vec![item("1", InboxBundle::reviews())]), now());
        inbox.apply(vec![("a1".into(), Err(PluginError::Unauthorized))], now());
        assert_eq!(inbox.all_items().len(), 1);
        assert_eq!(inbox.errors().len(), 1);
    }

    #[test]
    fn done_pin_and_snooze_move_items_between_tabs() {
        let mut inbox = inbox();
        inbox.apply(snapshot(vec![item("1", InboxBundle::reviews()), item("2", InboxBundle::reviews())]), now());
        inbox.toggle_done("1", now());
        inbox.snooze("2", now() + Duration::hours(3), SnoozeMode::Hide, Some(SnoozeReason::NoTime), false);
        let layout = inbox.layout(now(), "");
        assert_eq!(layout.done.len(), 1);
        assert_eq!(layout.snoozed.len(), 1);
        assert_eq!(layout.my_turn_items().count(), 0);

        inbox.toggle_done("1", now());
        inbox.toggle_pin("1");
        assert_eq!(inbox.layout(now(), "").pinned.len(), 1);

        inbox.clear_done(now());
        inbox.toggle_done("1", now());
        inbox.clear_done(now());
        assert!(inbox.layout(now(), "").done.is_empty(), "cleared items leave the Done tab");
    }

    #[test]
    fn snoozes_end_with_a_notice_and_a_reminder_chip() {
        let mut inbox = inbox();
        inbox.apply(snapshot(vec![item("1", InboxBundle::reviews())]), now());
        inbox.snooze("1", now() + Duration::minutes(30), SnoozeMode::Hide, None, false);
        assert!(inbox.tick(now()).is_empty());
        let notices = inbox.tick(now() + Duration::minutes(31));
        assert_eq!(notices.len(), 1);
        assert_eq!(notices[0].title, "Back in your inbox");
        assert!(inbox.states["1"].reminded_at.is_some());
        assert_eq!(inbox.layout(now() + Duration::minutes(31), "").my_turn_items().count(), 1);
        inbox.opened("1");
        assert!(!inbox.states.contains_key("1"));
    }

    #[test]
    fn new_activity_wakes_a_hidden_snooze_unless_turned_off() {
        let mut inbox = inbox();
        inbox.apply(snapshot(vec![item("1", InboxBundle::reviews())]), now());
        inbox.snooze("1", now() + Duration::days(2), SnoozeMode::Hide, None, false);
        let mut touched = item("1", InboxBundle::reviews());
        touched.date = now();
        inbox.preferences.wake_on_activity = false;
        inbox.apply(snapshot(vec![touched.clone()]), now());
        assert!(inbox.states["1"].snooze.is_some());
        inbox.preferences.wake_on_activity = true;
        touched.date = now() + Duration::minutes(1);
        inbox.apply(snapshot(vec![touched]), now());
        assert!(!inbox.states.contains_key("1"));
    }

    #[test]
    fn reminders_hide_until_due_then_done_removes_them() {
        let mut inbox = inbox();
        let id = inbox.add_reminder("  Call the bank ", now() + Duration::minutes(10), now());
        assert_eq!(inbox.layout(now(), "").snoozed.len(), 1);
        let notices = inbox.tick(now() + Duration::minutes(10));
        assert_eq!((notices[0].title.as_str(), notices[0].body.as_str()), ("Reminder", "Call the bank"));
        assert_eq!(inbox.layout(now() + Duration::minutes(10), "").my_turn_items().count(), 1);
        inbox.toggle_done(&id, now());
        assert!(inbox.all_items().is_empty());
    }

    #[test]
    fn the_policy_refuses_accounts_and_connections() {
        let mut inbox = inbox();
        inbox.apply(snapshot(vec![item("1", InboxBundle::reviews())]), now());
        inbox.managed = Managed { allowed_plugins: Some(vec!["slack".into()]), ..Managed::default() };
        assert!(inbox.all_items().is_empty(), "items of a refused tool disappear");
        assert!(inbox.fetch_jobs(Arc::new(NoNetwork)).is_empty());
        assert_eq!(inbox.errors(), ["Not allowed by your privacy policy."]);
        let refused = inbox.prepare_connect("github", None, HashMap::new(), &HashMap::new(), Arc::new(NoNetwork));
        assert_eq!(refused.err().as_deref(), Some("Not allowed by your privacy policy."));
        assert!(!inbox.sources().iter().find(|s| s.manifest.id == "github").unwrap().refusal.is_none());
    }

    #[tokio::test]
    async fn connecting_saves_the_account_and_its_secrets_and_disconnecting_forgets_them() {
        let vault = Arc::new(MemoryVault::default());
        let mut inbox = Inbox::in_memory(vault.clone(), Managed::default());
        let secrets = HashMap::from([("token".to_string(), "lin_api_k".to_string())]);
        let (account, plugin) = inbox.prepare_connect("linear", Some(" Work ".into()), HashMap::new(), &secrets, Arc::new(NoNetwork)).unwrap();
        assert_eq!(account.name.as_deref(), Some("Work"));
        assert!(plugin.fetch().await.is_err(), "fetches go through the client we gave");

        let id = account.id.clone();
        inbox.finish_connect(account, secrets, SourceSnapshot { identity: "Alice".into(), items: vec![] }, now()).unwrap();
        assert_eq!(vault.load().unwrap()[&id]["token"], "lin_api_k");
        assert_eq!(inbox.account_infos()[0].hosts, ["api.linear.app", "public.linear.app"]);

        inbox.disconnect(&id).unwrap();
        assert!(inbox.accounts.is_empty() && vault.load().unwrap().is_empty());
    }

    #[test]
    fn everything_is_saved_and_erasing_removes_it() {
        let dir = tempfile::tempdir().unwrap();
        let store = JsonStore::new(dir.path().join("Remora"));
        let vault: Arc<dyn Vault> = Arc::new(MemoryVault::default());
        {
            let mut saved = Inbox::open(store.clone(), vault.clone(), Managed::default());
            saved.accounts = inbox().accounts;
            saved.apply(snapshot(vec![item("1", InboxBundle::reviews())]), now());
            saved.toggle_pin("1");
            saved.add_reminder("Stand-up notes", now() + Duration::hours(1), now());
            saved.set_preferences(Preferences { refresh_minutes: 15, ..Preferences::default() });
        }
        let mut reopened = Inbox::open(store.clone(), vault, Managed::default());
        assert_eq!(reopened.all_items().len(), 2);
        assert!(reopened.states["1"].pinned);
        assert_eq!(reopened.preferences.refresh_minutes, 15);

        reopened.erase_local_data().unwrap();
        assert!(reopened.all_items().is_empty());
        assert!(store.load::<Vec<Account>>(ACCOUNTS).is_none());
    }
}

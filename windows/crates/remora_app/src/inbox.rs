use crate::{JsonStore, Managed, Preferences, Secrets, Vault};
use chrono::{DateTime, Utc};
use remora_core::{
    notices, Account, ActionOutcome, ActionRecord, AssistantPolicy, Backoff, Brief, BundleSummary, CompliancePolicy,
    Failure, FailureKind, InboxAssembler, InboxBundle, InboxItem, InboxLayout, ItemState, LinkFinder, Mark, Notice,
    NoticeKind, PersonalRanker, PluginManifest, ReviewPace, ReviewPrep, ReviewTiming, Snooze, SnoozeAdvisor,
    SnoozeInsight, SnoozeMode, SnoozeReason, SnoozeRecord, SnoozedItem, SourcesHealth, TriageAction, TriageSuggestion,
    VerbClassifier, WaitingAssistant, WaitingHelp,
};
use remora_plugins::{
    claude, links, registry, AssistantPlugin, GuardedHttpClient, HttpClient, PluginError, SourcePlugin, SourceSnapshot,
};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

const ACCOUNTS: &str = "accounts";
const CACHE: &str = "cache";
const STATES: &str = "states";
const REMINDERS: &str = "reminders";
const PREFERENCES: &str = "preferences";
/// Every snooze, kept on this computer to learn habits and spot patterns.
const HISTORY: &str = "snooze-history";
/// What you handle quickly or put off, to rank by your habits.
const LEARNING: &str = "learning";
/// Timed reviews against their estimate, for review prep's pace.
const REVIEW_TIMES: &str = "review-times";
/// What the assistant wrote: fetched again when stale, deleted when AI is no longer allowed.
const BRIEF: &str = "brief";
const SUMMARIES: &str = "bundle-summaries";

/// One account's fetch, built under the lock and run without it.
pub struct FetchJob {
    pub account_id: String,
    pub plugin: Box<dyn SourcePlugin>,
}

pub type FetchResult = (String, Result<SourceSnapshot, PluginError>);

/// Runs fetches concurrently.
pub async fn run(jobs: Vec<FetchJob>) -> Vec<FetchResult> {
    futures::future::join_all(jobs.into_iter().map(|job| async move { (job.account_id, job.plugin.fetch().await) }))
        .await
}

/// A source as the Settings screen lists it: its manifest, and why it can't be used, if it can't.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceInfo {
    pub manifest: PluginManifest,
    pub refusal: Option<String>,
    /// An assistant (Claude), shown apart from the sources.
    pub assistant: bool,
}

/// Any plugin's manifest: a source's, or an assistant's.
fn any_manifest(id: &str) -> Option<PluginManifest> {
    registry::manifest(id).or_else(|| claude::assistant_manifests().into_iter().find(|m| m.id == id))
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
    /// What fixes it: `auth` gets a Reconnect button.
    pub error_kind: Option<FailureKind>,
    /// What the last fetch said besides its items: a list cut at its limit, a missing permission.
    pub remarks: Vec<String>,
}

/// What opening an item opens: `app` first when there is one, then `web` if the app didn't open.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct OpenPlan {
    pub app: Option<String>,
    pub web: Option<String>,
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
    /// Every snooze with its reason and, once done, when: the advisor's input.
    snooze_history: Vec<SnoozeRecord>,
    /// Insights the user set aside ("Not now"), by id, for this session.
    dismissed_insights: HashSet<String>,
    action_records: Vec<ActionRecord>,
    ranker: PersonalRanker,
    review_timings: Vec<ReviewTiming>,
    /// Item id → items from other tools about the same thing, worked out when the items change.
    links: HashMap<String, Vec<String>>,
    pub brief: Option<Brief>,
    pub bundle_summaries: HashMap<String, BundleSummary>,
    /// Claude's proposal for the snoozed pile, until applied or discarded.
    pub triage: Vec<TriageSuggestion>,
    /// Each failing account's last failure, by kind.
    errors: HashMap<String, Failure>,
    /// What each account's last fetch said besides its items.
    remarks: HashMap<String, Vec<String>>,
    /// Accounts that failed lately, and when to ask them again.
    backoff: HashMap<String, Backoff>,
    synced: HashSet<String>,
    secrets: Option<Secrets>,
    pub last_refresh: Option<DateTime<Utc>>,
    /// The states and reminders before the last Done or Clear all, until the next one.
    undo_point: Option<(HashMap<String, ItemState>, Vec<InboxItem>)>,
}

impl Inbox {
    /// The real inbox, from its files and the credential store.
    pub fn open(store: JsonStore, vault: Arc<dyn Vault>, managed: Managed) -> Self {
        let items: HashMap<String, Vec<InboxItem>> = store.load(CACHE).unwrap_or_default();
        let mut inbox = Inbox {
            preferences: store.load(PREFERENCES).unwrap_or_default(),
            accounts: store.load(ACCOUNTS).unwrap_or_default(),
            items: items
                .into_iter()
                .map(|(id, items)| (id, items.into_iter().map(VerbClassifier::classify).collect()))
                .collect(),
            states: store.load(STATES).unwrap_or_default(),
            reminders: store.load(REMINDERS).unwrap_or_default(),
            snooze_history: store.load(HISTORY).unwrap_or_default(),
            dismissed_insights: HashSet::new(),
            ranker: PersonalRanker::new(&store.load::<Vec<ActionRecord>>(LEARNING).unwrap_or_default()),
            action_records: store.load(LEARNING).unwrap_or_default(),
            review_timings: store.load(REVIEW_TIMES).unwrap_or_default(),
            links: HashMap::new(),
            brief: store.load(BRIEF).unwrap_or_default(),
            bundle_summaries: store.load(SUMMARIES).unwrap_or_default(),
            triage: vec![],
            store: Some(store),
            vault,
            managed,
            errors: HashMap::new(),
            remarks: HashMap::new(),
            backoff: HashMap::new(),
            synced: HashSet::new(),
            secrets: None,
            last_refresh: None,
            undo_point: None,
        };
        inbox.recompute_links();
        inbox
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
            snooze_history: vec![],
            dismissed_insights: HashSet::new(),
            action_records: vec![],
            ranker: PersonalRanker::new(&[]),
            review_timings: vec![],
            links: HashMap::new(),
            brief: None,
            bundle_summaries: HashMap::new(),
            triage: vec![],
            errors: HashMap::new(),
            remarks: HashMap::new(),
            backoff: HashMap::new(),
            synced: HashSet::new(),
            secrets: None,
            last_refresh: None,
            undo_point: None,
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

    /// One item of an allowed account, or a reminder. Looked up in place: only that item is copied.
    pub fn item(&self, id: &str) -> Option<InboxItem> {
        self.accounts
            .iter()
            .filter(|a| self.allows(&a.plugin_id))
            .filter_map(|a| self.items.get(&a.id))
            .flatten()
            .chain(&self.reminders)
            .find(|i| i.id == id)
            .cloned()
    }

    pub fn layout(&self, now: DateTime<Utc>, query: &str) -> InboxLayout {
        // Your habits break ties inside each group, once there are enough of them.
        let score = |item: &InboxItem| self.ranker.score(item);
        InboxAssembler { wake_on_activity: self.preferences.wake_on_activity }.layout_ranked(
            &self.all_items(),
            &self.states,
            now,
            query,
            Some(&score),
        )
    }

    pub fn sources(&self) -> Vec<SourceInfo> {
        let policy = self.policy();
        let sources = registry::manifests().into_iter().map(|m| (m, false));
        let assistants = claude::assistant_manifests().into_iter().map(|m| (m, true));
        sources
            .chain(assistants)
            .map(|(m, assistant)| SourceInfo {
                refusal: policy.refusal(&m).map(str::to_string),
                manifest: m,
                assistant,
            })
            .collect()
    }

    pub fn account_infos(&self) -> Vec<AccountInfo> {
        let policy = self.policy();
        self.accounts
            .iter()
            .filter_map(|account| {
                let manifest = any_manifest(&account.plugin_id)?;
                Some(AccountInfo {
                    hosts: registry::allowed_hosts(account, &manifest),
                    egress: manifest.egress.description.clone(),
                    allowed: policy.allows(&manifest),
                    plugin_name: manifest.name,
                    error: self.errors.get(&account.id).map(|f| f.message.clone()),
                    error_kind: self.errors.get(&account.id).map(|f| f.kind),
                    remarks: self.remarks.get(&account.id).cloned().unwrap_or_default(),
                    account: account.clone(),
                })
            })
            .collect()
    }

    pub fn errors(&self) -> Vec<String> {
        self.errors.values().map(|f| f.message.clone()).collect()
    }

    /// What the footer says about the sources.
    pub fn health(&self) -> SourcesHealth {
        SourcesHealth::of(&self.errors)
    }

    pub fn remarks(&self) -> Vec<String> {
        self.remarks.values().flatten().cloned().collect()
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
    /// `manual`: asked for by the user, which doesn't wait out a slow-down (`Backoff`). An account that keeps failing,
    /// or is rate-limited, sits this refresh out and keeps its items and its error.
    pub fn fetch_jobs(&mut self, http: Arc<dyn HttpClient>, now: DateTime<Utc>, manual: bool) -> Vec<FetchJob> {
        let secrets = match self.secrets() {
            Ok(secrets) => secrets.clone(),
            Err(error) => {
                for account in &self.accounts {
                    self.errors.insert(account.id.clone(), Failure::new(FailureKind::Other, error.clone()));
                }
                return vec![];
            }
        };
        let policy = self.policy();
        let mut jobs = vec![];
        for account in self.accounts.clone() {
            if self.backoff.get(&account.id).is_some_and(|b| b.waits(now, manual)) {
                continue;
            }
            let Some(manifest) = registry::manifest(&account.plugin_id) else { continue };
            if let Some(refusal) = policy.refusal(&manifest) {
                self.errors.insert(account.id.clone(), Failure::new(FailureKind::Other, refusal));
                continue;
            }
            let guarded: Arc<dyn HttpClient> =
                Arc::new(GuardedHttpClient::new(http.clone(), registry::allowed_hosts(&account, &manifest)));
            match registry::make(&account, secrets.get(&account.id).unwrap_or(&HashMap::new()), guarded) {
                Ok(plugin) => jobs.push(FetchJob { account_id: account.id.clone(), plugin }),
                Err(error) => {
                    self.errors.insert(account.id.clone(), Failure::new(FailureKind::Other, error.to_string()));
                }
            }
        }
        jobs
    }

    /// Stores what came back and says what deserves a notification. The first sync of an account is
    /// silent; failed accounts keep their last items.
    pub fn apply(&mut self, results: Vec<FetchResult>, now: DateTime<Utc>) -> Vec<Notice> {
        // Fetches run without the lock: an account disconnected, or everything erased, while they ran must not come
        // back. A reconnected account has a new id, so its old results are dropped too.
        let results: Vec<FetchResult> =
            results.into_iter().filter(|(id, _)| self.accounts.iter().any(|a| &a.id == id)).collect();
        self.last_refresh = Some(now);
        if results.is_empty() {
            // Nothing to store: in particular, nothing written back after Erase.
            return vec![];
        }
        let mut out = vec![];
        let interval = chrono::Duration::minutes(i64::from(self.preferences.refresh_minutes.max(1)));
        // Every source failed to connect: that's the network, not the tools. Said once, and no slow-down for it. With a
        // single source, an unreachable server and no network look the same: it says it can't reach the server.
        let offline = results.len() > 1
            && results.iter().all(|(_, result)| matches!(result, Err(e) if e.kind() == FailureKind::Unreachable));
        for (account_id, result) in results {
            match result {
                Err(error) => {
                    let kind = if offline { FailureKind::Offline } else { error.kind() };
                    if kind == FailureKind::Offline {
                        self.errors.insert(account_id, Failure::new(kind, error.to_string()));
                        continue;
                    }
                    let resume = match error {
                        // No reset time given: wait at least one refresh.
                        PluginError::RateLimited(at) => {
                            Some(at.and_then(|s| DateTime::from_timestamp(s, 0)).unwrap_or(now + interval))
                        }
                        _ => None,
                    };
                    self.backoff.entry(account_id.clone()).or_default().failed(now, interval, resume);
                    self.errors.insert(account_id, Failure::new(kind, error.to_string()));
                }
                Ok(snapshot) => {
                    self.backoff.remove(&account_id);
                    self.errors.remove(&account_id);
                    if snapshot.remarks.is_empty() {
                        self.remarks.remove(&account_id);
                    } else {
                        self.remarks.insert(account_id.clone(), snapshot.remarks.clone());
                    }
                    let items: Vec<InboxItem> = snapshot.items.into_iter().map(VerbClassifier::classify).collect();
                    let previous = self
                        .synced
                        .contains(&account_id)
                        .then(|| self.items.get(&account_id).cloned().unwrap_or_default());
                    let plugin_id = self.accounts.iter().find(|a| a.id == account_id).map(|a| a.plugin_id.clone());
                    for notice in notices(previous.as_deref(), &items) {
                        let wanted = if notice.kind == NoticeKind::Arrival {
                            self.preferences.notify_arrivals
                        } else {
                            self.preferences.notify_status_changes
                        };
                        if wanted {
                            out.push(self.respecting_privacy(notice, plugin_id.as_deref()));
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
        self.prune();
        self.recompute_links();
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
                if state
                    .snooze
                    .as_ref()
                    .is_some_and(|s| s.mode == SnoozeMode::Hide && s.fingerprint != item.fingerprint())
                {
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
                let hidden = self.preferences.hidden_content_plugins.contains(&item.plugin_id);
                let notice = Notice {
                    kind: NoticeKind::Reminder,
                    item_id: id.clone(),
                    title: if reminder { "Reminder" } else { "Back in your inbox" }.into(),
                    subtitle: item.context.clone(),
                    body: [Some(item.title.clone()), snooze.note].into_iter().flatten().collect::<Vec<_>>().join("\n"),
                    url: item.url.clone(),
                };
                out.push(if hidden { notice.redacted() } else { notice });
            }
        }
        if !out.is_empty() {
            self.persist(STATES, &self.states);
        }
        out
    }

    /// Hides what was written when the user doesn't want that integration's content on screen.
    fn respecting_privacy(&self, notice: Notice, plugin_id: Option<&str>) -> Notice {
        match plugin_id {
            Some(id) if self.preferences.hidden_content_plugins.iter().any(|p| p == id) => notice.redacted(),
            _ => notice,
        }
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
        if let Some(item) = self.item(id) {
            self.learn_handled(&item, Utc::now());
        }
    }

    /// Done until it changes. A reminder marked done is gone for good.
    pub fn toggle_done(&mut self, id: &str, now: DateTime<Utc>) {
        let Some(item) = self.item(id) else { return };
        self.remember();
        let marking = self.states.get(id).and_then(|s| s.done.as_ref()).is_none();
        if item.bundle == InboxBundle::reminders() && marking {
            self.reminders.retain(|r| r.id != id);
            self.states.remove(id);
            self.persist(REMINDERS, &self.reminders);
            self.persist(STATES, &self.states);
            return;
        }
        let fingerprint = item.fingerprint();
        let started = self.states.get(id).and_then(|s| s.started_at);
        self.update(id, |s| {
            s.done = marking.then_some(Mark { at: now, fingerprint, cleared_at: None });
            s.reminded_at = None;
            s.snooze = None;
            s.started_at = None;
        });
        if marking {
            self.learn_handled(&item, now);
            if let Some(started) = started {
                self.learn_review_time(&item, started, now);
            }
        }
        // The snooze that ended in Done: when you finally do things teaches your best hour.
        if marking {
            if let Some(record) = self.snooze_history.iter_mut().rev().find(|r| r.item_id == id && r.done_at.is_none())
            {
                record.done_at = Some(now);
                self.persist(HISTORY, &self.snooze_history);
            }
        }
    }

    /// Empties the Done tab; cleared items still come back on new activity.
    pub fn clear_done(&mut self, now: DateTime<Utc>) {
        self.remember();
        for state in self.states.values_mut() {
            if let Some(done) = state.done.as_mut() {
                done.cleared_at.get_or_insert(now);
            }
        }
        self.persist(STATES, &self.states);
    }

    /// Marks the item as the one being worked on: it moves to the In progress tab and the tray says so.
    pub fn start(&mut self, id: &str, now: DateTime<Utc>) {
        self.update(id, |s| {
            s.started_at = Some(now);
            s.snooze = None;
            s.reminded_at = None;
            s.done = None;
        });
    }

    /// Puts a started item back where it was before.
    pub fn stop(&mut self, id: &str) {
        self.update(id, |s| s.started_at = None);
    }

    pub fn toggle_pin(&mut self, id: &str) {
        self.update(id, |s| s.pinned = !s.pinned);
    }

    pub fn snooze(
        &mut self,
        id: &str,
        until: DateTime<Utc>,
        mode: SnoozeMode,
        reason: Option<SnoozeReason>,
        until_news: bool,
    ) {
        let Some(item) = self.item(id) else { return };
        let fingerprint = item.fingerprint();
        self.update(id, |s| {
            s.snooze =
                Some(Snooze { until, mode, note: None, fingerprint, reason, until_news: until_news.then_some(true) });
            s.done = None;
            s.reminded_at = None;
            s.started_at = None;
        });
        // Learned from, on this computer only; a reminder isn't a habit.
        if item.bundle != InboxBundle::reminders() {
            self.learn(ActionRecord::new(&item, ActionOutcome::Deferred, Utc::now()));
            self.snooze_history.push(SnoozeRecord::new(&item, reason, Utc::now(), until));
            let excess = self.snooze_history.len().saturating_sub(500);
            self.snooze_history.drain(..excess);
            self.persist(HISTORY, &self.snooze_history);
        }
    }

    /// Opened or finished within a day of its last activity: something you handle quickly.
    fn learn_handled(&mut self, item: &InboxItem, now: DateTime<Utc>) {
        if item.bundle != InboxBundle::reminders() && now - item.date < chrono::Duration::days(1) {
            self.learn(ActionRecord::new(item, ActionOutcome::Quick, now));
        }
    }

    fn learn(&mut self, record: ActionRecord) {
        self.action_records.push(record);
        let excess = self.action_records.len().saturating_sub(1_000);
        self.action_records.drain(..excess);
        self.ranker = PersonalRanker::new(&self.action_records);
        self.persist(LEARNING, &self.action_records);
    }

    /// A review started then done: its real duration against the plain estimate, the last 50 kept.
    fn learn_review_time(&mut self, item: &InboxItem, started: DateTime<Utc>, now: DateTime<Utc>) {
        let Some(prep) = ReviewPrep::new(item, 1.0) else { return };
        self.review_timings
            .push(ReviewTiming { estimated: i64::from(prep.estimated_minutes), actual: (now - started).num_minutes() });
        let excess = self.review_timings.len().saturating_sub(50);
        self.review_timings.drain(..excess);
        self.persist(REVIEW_TIMES, &self.review_timings);
    }

    // MARK: The assistant

    /// The connected assistant, only when the privacy policy allows it.
    pub fn assistant_account(&self) -> Option<Account> {
        let policy = self.policy();
        self.accounts
            .iter()
            .find(|a| {
                claude::is_assistant(&a.plugin_id)
                    && claude::assistant_manifests().iter().any(|m| m.id == a.plugin_id && policy.allows(m))
            })
            .cloned()
    }

    /// What the assistant may read: everything but the sources you keep from it.
    pub fn assistant_policy(&self) -> AssistantPolicy {
        AssistantPolicy::new(self.preferences.assistant_excluded_sources.iter().cloned().collect(), None)
    }

    /// The assistant for an account, through a client limited to its declared hosts.
    fn assistant_for(
        &mut self,
        account: &Account,
        http: Arc<dyn HttpClient>,
        language: &str,
    ) -> Result<Box<dyn AssistantPlugin>, String> {
        let manifest = claude::assistant_manifests()
            .into_iter()
            .find(|m| m.id == account.plugin_id)
            .ok_or("Unknown assistant.")?;
        if let Some(refusal) = self.policy().refusal(&manifest) {
            return Err(refusal.to_string());
        }
        let secrets = self.secrets()?.get(&account.id).cloned().unwrap_or_default();
        let guarded: Arc<dyn HttpClient> =
            Arc::new(GuardedHttpClient::new(http, registry::allowed_hosts(account, &manifest)));
        let defaults: HashMap<String, String> =
            manifest.fields.iter().map(|f| (f.key.clone(), f.default_value.clone())).collect();
        let mut constrained = account.clone();
        constrained.settings = self.assistant_policy().constrained(&account.settings, &defaults);
        claude::make_assistant(&constrained, &secrets, guarded, language).map_err(|e| e.to_string())
    }

    /// The assistant to ask now, or None when there is none or it isn't allowed.
    pub fn assistant(
        &mut self,
        http: Arc<dyn HttpClient>,
        language: &str,
    ) -> Option<Result<Box<dyn AssistantPlugin>, String>> {
        let account = self.assistant_account()?;
        Some(self.assistant_for(&account, http, language))
    }

    /// The whole inbox as the assistant may see it, for a brief.
    pub fn brief_items(&self, now: DateTime<Utc>) -> Vec<InboxItem> {
        let layout = self.layout(now, "");
        let items: Vec<InboxItem> =
            layout.pinned.into_iter().chain(layout.my_turn.into_iter().flat_map(|g| g.items)).collect();
        self.assistant_policy().items(&items)
    }

    pub fn brief_is_fresh(&self, now: DateTime<Utc>) -> bool {
        self.brief.as_ref().is_some_and(|b| b.is_fresh(self.preferences.brief_cache_minutes, now))
    }

    pub fn store_brief(&mut self, brief: Option<Brief>) {
        self.brief = brief;
        self.persist(BRIEF, &self.brief);
    }

    /// A group's items as the assistant may see them, and whether its summary is still fresh.
    pub fn bundle_items(&self, bundle_id: &str, now: DateTime<Utc>) -> (Vec<InboxItem>, bool) {
        let layout = self.layout(now, "");
        let items: Vec<InboxItem> = layout
            .my_turn
            .into_iter()
            .chain(layout.waiting)
            .filter(|g| g.bundle.id == bundle_id)
            .flat_map(|g| g.items)
            .collect();
        let items = self.assistant_policy().items(&items);
        let fresh = self
            .bundle_summaries
            .get(bundle_id)
            .is_some_and(|s| s.is_fresh(&items, self.preferences.brief_cache_minutes, now));
        (items, fresh)
    }

    pub fn store_summary(&mut self, bundle_id: &str, summary: BundleSummary) {
        self.bundle_summaries.insert(bundle_id.to_string(), summary);
        self.persist(SUMMARIES, &self.bundle_summaries);
    }

    /// The snoozed pile as the assistant may see it, with how often each was snoozed.
    pub fn snoozed_for_triage(&self, now: DateTime<Utc>) -> Vec<SnoozedItem> {
        let snoozed = self.assistant_policy().items(&self.layout(now, "").snoozed);
        snoozed
            .into_iter()
            .filter_map(|item| {
                let snooze = self.states.get(&item.id)?.snooze.clone()?;
                let times = self.snooze_count(&item.id, now) as u32;
                Some(SnoozedItem { item, snooze, times })
            })
            .collect()
    }

    /// Applies the ticked suggestions. Every "now" item comes back; only the first is opened, never a burst.
    /// Returns the item to open, if any.
    pub fn apply_triage(&mut self, selected: &[TriageSuggestion], now: DateTime<Utc>) -> Option<String> {
        let mut open = None;
        for suggestion in selected {
            if self.item(&suggestion.id).is_none() {
                continue;
            }
            match suggestion.action {
                TriageAction::Keep => {}
                TriageAction::Reschedule => {
                    if let Some(until) = suggestion.until {
                        self.reschedule(&suggestion.id, until);
                    }
                }
                TriageAction::Done => self.toggle_done(&suggestion.id, now),
                TriageAction::Now => {
                    self.unsnooze(&suggestion.id);
                    open.get_or_insert_with(|| suggestion.id.clone());
                }
            }
        }
        let applied: HashSet<&str> = selected.iter().map(|s| s.id.as_str()).collect();
        self.triage.retain(|s| !applied.contains(s.id.as_str()));
        open
    }

    /// Without an allowed assistant, nothing it wrote is kept or shown.
    pub fn enforce_ai_policy(&mut self) {
        if self.assistant_account().is_none()
            && (self.brief.is_some() || !self.bundle_summaries.is_empty() || !self.triage.is_empty())
        {
            self.store_brief(None);
            self.bundle_summaries.clear();
            self.persist(SUMMARIES, &self.bundle_summaries);
            self.triage.clear();
        }
    }

    // MARK: Help on a row

    /// Snoozes of this item in the last 30 days.
    pub fn snooze_count(&self, id: &str, now: DateTime<Utc>) -> usize {
        let since = now - chrono::Duration::days(30);
        self.snooze_history.iter().filter(|r| r.item_id == id && r.at > since).count()
    }

    /// How your reviews compare with the estimate; 1 until five were timed.
    pub fn review_pace(&self) -> f64 {
        ReviewPace::factor(&self.review_timings)
    }

    pub fn review_prep(&self, item: &InboxItem) -> Option<ReviewPrep> {
        (item.bundle == InboxBundle::reviews()).then(|| ReviewPrep::new(item, self.review_pace())).flatten()
    }

    /// Who could review your PR, or a nudge when its reviewers stay silent.
    pub fn waiting_help(&self, item: &InboxItem, now: DateTime<Utc>) -> Option<WaitingHelp> {
        let me = self.accounts.iter().find(|a| a.id == item.account_id).and_then(|a| a.identity.as_deref());
        WaitingAssistant::default().help(item, &self.all_items(), me, now)
    }

    /// The message to paste for this help: who could review, or a kind nudge. Nothing is sent.
    pub fn waiting_draft(&self, id: &str, now: DateTime<Utc>, tr: &dyn Fn(&str) -> String) -> Option<String> {
        let item = self.item(id)?;
        let help = self.waiting_help(&item, now)?;
        Some(WaitingAssistant::default().draft(&help, &item, tr))
    }

    /// The reason usually given in this item's repo or channel, preselected in the snooze sheet.
    pub fn usual_reason(&self, id: &str) -> Option<SnoozeReason> {
        let item = self.item(id)?;
        SnoozeAdvisor::new().usual_reason(&item, &self.snooze_history)
    }

    /// Reviews waiting for you, in session order: pressing first, then quick wins, then whoever waited longest.
    pub fn review_queue(&self, now: DateTime<Utc>) -> Vec<InboxItem> {
        let reviews: Vec<InboxItem> = self
            .layout(now, "")
            .my_turn
            .into_iter()
            .filter(|g| g.bundle == InboxBundle::reviews())
            .flat_map(|g| g.items)
            .collect();
        remora_core::ReviewQueue::order(&reviews, now)
    }

    /// Items from other tools about the same thing.
    pub fn linked(&self, id: &str) -> Vec<InboxItem> {
        self.links.get(id).map(|ids| ids.iter().filter_map(|id| self.item(id)).collect()).unwrap_or_default()
    }

    /// Why an item ranks higher than its priority alone would put it.
    pub fn ranking_reason(&self, item: &InboxItem, tr: &dyn Fn(&str) -> String) -> Option<String> {
        self.ranker.reason(item, tr)
    }

    fn recompute_links(&mut self) {
        self.links = LinkFinder::default().links(&self.all_items());
    }

    /// Moves an existing snooze without counting a new one (spreading a pile-up, one time for a cluster).
    pub fn reschedule(&mut self, id: &str, until: DateTime<Utc>) {
        if self.states.get(id).and_then(|s| s.snooze.as_ref()).is_none() {
            self.snooze(id, until, SnoozeMode::Hide, None, false);
            return;
        }
        self.update(id, |s| {
            if let Some(snooze) = s.snooze.as_mut() {
                snooze.until = until;
            }
        });
    }

    pub fn snooze_many(&mut self, ids: &[String], until: DateTime<Utc>, reason: Option<SnoozeReason>) {
        for id in ids {
            self.snooze(id, until, SnoozeMode::Hide, reason, false);
        }
    }

    /// When to come back for this reason, from your habits. Weekends are skipped.
    pub fn suggested_return(&self, reason: SnoozeReason, now: DateTime<Utc>) -> DateTime<Utc> {
        SnoozeAdvisor::new().suggested_return(reason, &self.snooze_history, now)
    }

    /// Patterns in the snoozed pile, without the ones set aside.
    pub fn insights(&self, now: DateTime<Utc>) -> Vec<SnoozeInsight> {
        let snoozed = self.layout(now, "").snoozed;
        SnoozeAdvisor::new()
            .insights(&snoozed, &self.states, &self.snooze_history, now)
            .into_iter()
            .filter(|insight| !self.dismissed_insights.contains(&insight.id()))
            .collect()
    }

    pub fn dismiss_insight(&mut self, id: &str) {
        self.dismissed_insights.insert(id.to_string());
    }

    /// A pile-up, 30 minutes apart from its slot.
    pub fn spread(&mut self, ids: &[String], from: DateTime<Utc>) {
        let items: Vec<InboxItem> = ids.iter().filter_map(|id| self.item(id)).collect();
        for (id, until) in SnoozeAdvisor::new().spread(&items, from) {
            self.reschedule(&id, until);
        }
    }

    /// Every item back at the earliest of their return times.
    pub fn align_returns(&mut self, ids: &[String]) {
        let Some(earliest) =
            ids.iter().filter_map(|id| self.states.get(id).and_then(|s| s.snooze.as_ref()).map(|s| s.until)).min()
        else {
            return;
        };
        for id in ids {
            self.reschedule(id, earliest);
        }
    }

    /// Several items done at once ("Let them go"), undoable together.
    pub fn sweep(&mut self, ids: &[String], now: DateTime<Utc>) {
        self.remember();
        let undo = self.undo_point.clone();
        for id in ids {
            if self.states.get(id).and_then(|s| s.done.as_ref()).is_none() {
                self.toggle_done(id, now);
            }
        }
        self.undo_point = undo;
    }

    pub fn unsnooze(&mut self, id: &str) {
        self.update(id, |s| s.snooze = None);
    }

    /// A reminder is an item of its own, hidden until its time. It needs a title.
    pub fn add_reminder(&mut self, title: &str, at: DateTime<Utc>, now: DateTime<Utc>) -> Result<String, String> {
        if title.trim().is_empty() {
            return Err("Type what to remember.".into());
        }
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
            expires: None,
            changes: None,
            suggested_people: None,
        });
        self.persist(REMINDERS, &self.reminders);
        self.snooze(&id, at, SnoozeMode::Hide, None, false);
        Ok(id)
    }

    /// Saves the preferences. True when the refresh interval changed, so the next refresh is rescheduled.
    pub fn set_preferences(&mut self, preferences: Preferences) -> bool {
        let interval_changed = self.preferences.refresh_minutes != preferences.refresh_minutes;
        self.preferences = preferences;
        self.persist(PREFERENCES, &self.preferences);
        // Privacy settings may have just turned the assistant off: what it wrote goes too.
        self.enforce_ai_policy();
        interval_changed
    }

    /// What opening an item opens, as a click in the inbox or a notification does: the tool's app when the
    /// preference allows it (`app`, tried first), else the web page (`web`). Only web pages and the tools' apps
    /// are ever opened (`links`). The item stops being reminded.
    pub fn open_plan(&mut self, id: &str, in_browser: bool) -> Result<OpenPlan, String> {
        let item = self.item(id).ok_or("This item is gone.")?;
        self.opened(id);
        let http_hosts = links::http_hosts(self.accounts.iter().find(|a| a.id == item.account_id));
        let app = item.app_url.filter(|url| !in_browser && self.preferences.open_in_apps && links::is_app_link(url));
        let web = item.url.filter(|url| links::is_web_link(url, &http_hosts));
        Ok(OpenPlan { app, web })
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
        let guarded: Arc<dyn HttpClient> =
            Arc::new(GuardedHttpClient::new(http, registry::allowed_hosts(&account, &manifest)));
        let plugin = registry::make(&account, secrets, guarded).map_err(|e| e.to_string())?;
        Ok((account, plugin))
    }

    /// Connecting an assistant: the caller tests it once with the plugin (a brief, or a made-up item
    /// when the whole-inbox brief is off, so no inbox content leaves yet), then calls `finish_assistant_connect`.
    pub fn prepare_assistant_connect(
        &self,
        plugin_id: &str,
        name: Option<String>,
        settings: HashMap<String, String>,
        secrets: &HashMap<String, String>,
        http: Arc<dyn HttpClient>,
        language: &str,
    ) -> Result<(Account, Box<dyn AssistantPlugin>), String> {
        let manifest = claude::assistant_manifests()
            .into_iter()
            .find(|m| m.id == plugin_id)
            .ok_or_else(|| format!("Unknown plugin “{plugin_id}”."))?;
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
        let guarded: Arc<dyn HttpClient> =
            Arc::new(GuardedHttpClient::new(http, registry::allowed_hosts(&account, &manifest)));
        let defaults: HashMap<String, String> =
            manifest.fields.iter().map(|f| (f.key.clone(), f.default_value.clone())).collect();
        let mut constrained = account.clone();
        constrained.settings = self.assistant_policy().constrained(&account.settings, &defaults);
        let plugin = claude::make_assistant(&constrained, secrets, guarded, language).map_err(|e| e.to_string())?;
        Ok((account, plugin))
    }

    /// One assistant at a time: a new one replaces the previous one, and its token.
    pub fn finish_assistant_connect(
        &mut self,
        account: Account,
        secrets: HashMap<String, String>,
    ) -> Result<(), String> {
        let previous: Vec<String> =
            self.accounts.iter().filter(|a| claude::is_assistant(&a.plugin_id)).map(|a| a.id.clone()).collect();
        let mut all = self.secrets()?.clone();
        for id in &previous {
            all.remove(id);
        }
        all.insert(account.id.clone(), secrets);
        self.vault.save(&all)?;
        self.secrets = Some(all);
        self.accounts.retain(|a| !claude::is_assistant(&a.plugin_id));
        self.accounts.push(account);
        self.persist(ACCOUNTS, &self.accounts);
        Ok(())
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
        }
    }

    /// Saves an account whose first fetch worked. Its items arrive silently.
    pub fn finish_connect(
        &mut self,
        mut account: Account,
        secrets: HashMap<String, String>,
        snapshot: SourceSnapshot,
        now: DateTime<Utc>,
    ) -> Result<(), String> {
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

    /// Reconnect: the same account with a new token, so its Done, snoozes and pins stay. The caller
    /// fetches once with the plugin, then calls `finish_reconnect`.
    pub fn prepare_reconnect(
        &self,
        account_id: &str,
        secrets: &HashMap<String, String>,
        http: Arc<dyn HttpClient>,
    ) -> Result<Box<dyn SourcePlugin>, String> {
        let account =
            self.accounts.iter().find(|a| a.id == account_id).ok_or("This account is no longer connected.")?;
        let manifest =
            registry::manifest(&account.plugin_id).ok_or_else(|| format!("Unknown plugin “{}”.", account.plugin_id))?;
        if let Some(refusal) = self.policy().refusal(&manifest) {
            return Err(refusal.to_string());
        }
        let guarded: Arc<dyn HttpClient> =
            Arc::new(GuardedHttpClient::new(http, registry::allowed_hosts(account, &manifest)));
        registry::make(account, secrets, guarded).map_err(|e| e.to_string())
    }

    /// Saves the new token of an account whose fetch worked with it, and its items.
    pub fn finish_reconnect(
        &mut self,
        account_id: &str,
        secrets: HashMap<String, String>,
        snapshot: SourceSnapshot,
        now: DateTime<Utc>,
    ) -> Result<(), String> {
        if !self.accounts.iter().any(|a| a.id == account_id) {
            return Err("This account is no longer connected.".into());
        }
        let mut all = self.secrets()?.clone();
        all.insert(account_id.to_string(), secrets);
        self.vault.save(&all)?;
        self.secrets = Some(all);
        self.backoff.remove(account_id);
        self.apply(vec![(account_id.to_string(), Ok(snapshot))], now);
        Ok(())
    }

    pub fn rename(&mut self, account_id: &str, name: &str) {
        if let Some(account) = self.accounts.iter_mut().find(|a| a.id == account_id) {
            account.name = Some(name.trim().to_string()).filter(|n| !n.is_empty());
        }
        self.persist(ACCOUNTS, &self.accounts);
    }

    pub fn disconnect(&mut self, account_id: &str) -> Result<(), String> {
        self.undo_point = None;
        let mut all = self.secrets()?.clone();
        all.remove(account_id);
        self.vault.save(&all)?;
        self.secrets = Some(all);
        self.accounts.retain(|a| a.id != account_id);
        self.items.remove(account_id);
        self.recompute_links();
        self.errors.remove(account_id);
        self.remarks.remove(account_id);
        self.synced.remove(account_id);
        self.prune();
        self.persist(ACCOUNTS, &self.accounts);
        self.persist(CACHE, &self.items);
        self.persist(STATES, &self.states);
        self.enforce_ai_policy();
        Ok(())
    }

    /// Settings → Privacy → Erase local data: every account, file and token.
    pub fn erase_local_data(&mut self) -> Result<(), String> {
        self.remarks.clear();
        self.undo_point = None;
        self.vault.save(&Secrets::new())?;
        if let Some(store) = &self.store {
            store.erase().map_err(|e| e.to_string())?;
        }
        let (vault, managed, store) = (self.vault.clone(), self.managed.clone(), self.store.take());
        *self = Inbox::in_memory(vault, managed);
        self.store = store;
        Ok(())
    }

    fn remember(&mut self) {
        self.undo_point = Some((self.states.clone(), self.reminders.clone()));
    }

    /// Puts back what the last Done or Clear all changed: a reminder marked done comes back too. False when there's
    /// nothing to undo.
    pub fn undo(&mut self) -> bool {
        let Some((states, reminders)) = self.undo_point.take() else { return false };
        self.states = states;
        self.reminders = reminders;
        self.persist(STATES, &self.states);
        self.persist(REMINDERS, &self.reminders);
        true
    }

    fn persist<T: Serialize>(&self, name: &str, value: &T) {
        if let Some(store) = &self.store {
            if let Err(error) = store.save(name, value) {
                eprintln!("Remora: could not save {name}: {error}");
            }
        }
    }

    /// Demo items, as if an account had synced: see `demo.rs`.
    pub fn load_demo(
        &mut self,
        accounts: Vec<Account>,
        items: HashMap<String, Vec<InboxItem>>,
        states: HashMap<String, ItemState>,
    ) {
        self.accounts = accounts;
        self.items = items
            .into_iter()
            .map(|(id, items)| (id, items.into_iter().map(VerbClassifier::classify).collect()))
            .collect();
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
            expires: None,
            changes: None,
            suggested_people: None,
        }
    }

    fn inbox() -> Inbox {
        let mut inbox = Inbox::in_memory(Arc::new(MemoryVault::default()), Managed::default());
        inbox.accounts = vec![Account {
            id: "a1".into(),
            plugin_id: "github".into(),
            name: None,
            settings: HashMap::new(),
            identity: None,
        }];
        inbox
    }

    fn snapshot(items: Vec<InboxItem>) -> Vec<FetchResult> {
        vec![("a1".into(), Ok(SourceSnapshot { identity: "alice".into(), items, remarks: vec![] }))]
    }

    #[test]
    fn hidden_content_keeps_the_notice_but_not_the_text() {
        let mut inbox = inbox();
        inbox.preferences.hidden_content_plugins = vec!["github".into()];
        inbox.apply(snapshot(vec![]), now());
        let found = inbox.apply(snapshot(vec![item("1", InboxBundle::reviews())]), now());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].subtitle, "acme/app #1");
        assert!(!found[0].body.contains("Item 1"));
    }

    /// What a click opens, decided here rather than in the tray app.
    #[test]
    fn opening_prefers_the_app_then_the_web_page() {
        let mut inbox = inbox();
        let mut slack = item("1", InboxBundle::reviews());
        slack.app_url = Some("slack://channel?team=T1&id=C1".into());
        let mut bad = item("2", InboxBundle::reviews());
        bad.url = Some("file:///etc/passwd".into());
        bad.app_url = Some("ms-settings:privacy".into());
        inbox.apply(snapshot(vec![slack, bad]), now());

        let web = Some("https://github.com/acme/app/pull/1".to_string());
        let plan = inbox.open_plan("1", false).unwrap();
        assert_eq!(plan, OpenPlan { app: Some("slack://channel?team=T1&id=C1".into()), web: web.clone() });
        assert_eq!(inbox.open_plan("1", true).unwrap(), OpenPlan { app: None, web: web.clone() }, "in the browser");
        inbox.preferences.open_in_apps = false;
        assert_eq!(inbox.open_plan("1", false).unwrap().app, None);
        assert_eq!(inbox.open_plan("2", false).unwrap(), OpenPlan::default(), "only web pages and the tools' apps");
        assert!(inbox.open_plan("gone", false).is_err());
    }

    #[test]
    fn opening_ends_the_reminder() {
        let mut inbox = inbox();
        let id = inbox.add_reminder("Call the bank", now() - Duration::minutes(1), now() - Duration::hours(1)).unwrap();
        inbox.tick(now());
        assert!(inbox.states[&id].reminded_at.is_some());
        inbox.open_plan(&id, false).unwrap();
        assert!(inbox.states.get(&id).is_none_or(|s| s.reminded_at.is_none()));
    }

    /// Done, a reminder's Done and Clear all can be undone, once.
    #[test]
    fn done_and_clear_all_can_be_undone() {
        let mut inbox = inbox();
        inbox.apply(snapshot(vec![item("1", InboxBundle::reviews()), item("2", InboxBundle::reviews())]), now());
        let reminder =
            inbox.add_reminder("Call the bank", now() - Duration::minutes(1), now() - Duration::hours(1)).unwrap();
        inbox.tick(now());
        assert!(!inbox.undo(), "nothing to undo yet");

        inbox.toggle_done("1", now());
        assert!(inbox.undo());
        assert!(inbox.states.get("1").is_none_or(|s| s.done.is_none()));
        assert!(!inbox.undo(), "only once");

        inbox.toggle_done(&reminder, now());
        assert!(inbox.item(&reminder).is_none(), "a reminder marked done is gone");
        assert!(inbox.undo());
        assert!(inbox.item(&reminder).is_some() && inbox.states[&reminder].reminded_at.is_some());

        inbox.toggle_done("2", now());
        inbox.clear_done(now());
        assert!(inbox.layout(now(), "").done.is_empty());
        assert!(inbox.undo());
        assert_eq!(inbox.layout(now(), "").done.len(), 1, "back in Done, not cleared");
    }

    #[test]
    fn a_reminder_needs_a_title() {
        let mut inbox = inbox();
        assert_eq!(inbox.add_reminder("   ", now(), now()), Err("Type what to remember.".into()));
        assert!(inbox.reminders.is_empty());
    }

    #[test]
    fn preferences_say_when_the_refresh_interval_changed() {
        let mut inbox = inbox();
        let same = inbox.preferences.clone();
        assert!(!inbox.set_preferences(Preferences { open_in_apps: !same.open_in_apps, ..same.clone() }));
        assert!(inbox.set_preferences(Preferences { refresh_minutes: same.refresh_minutes + 5, ..same }));
    }

    #[test]
    fn started_item_moves_to_in_progress_until_done() {
        let mut inbox = inbox();
        inbox.apply(snapshot(vec![item("1", InboxBundle::reviews())]), now());
        inbox.start("1", now());
        let layout = inbox.layout(now(), "");
        assert_eq!(layout.in_progress.len(), 1);
        assert_eq!(layout.action_count(), 0);
        inbox.toggle_done("1", now());
        assert!(inbox.layout(now(), "").in_progress.is_empty());
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
        assert!(inbox
            .apply(
                snapshot(vec![
                    item("1", InboxBundle::authored()),
                    item("2", InboxBundle::reviews()),
                    item("3", InboxBundle::reviews())
                ]),
                now()
            )
            .is_empty());
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
        let id = inbox.add_reminder("  Call the bank ", now() + Duration::minutes(10), now()).unwrap();
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
        assert!(inbox.fetch_jobs(Arc::new(NoNetwork), now(), false).is_empty());
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
        let (account, plugin) = inbox
            .prepare_connect("linear", Some(" Work ".into()), HashMap::new(), &secrets, Arc::new(NoNetwork))
            .unwrap();
        assert_eq!(account.name.as_deref(), Some("Work"));
        assert!(plugin.fetch().await.is_err(), "fetches go through the client we gave");

        let id = account.id.clone();
        inbox
            .finish_connect(
                account,
                secrets,
                SourceSnapshot { identity: "Alice".into(), items: vec![], remarks: vec![] },
                now(),
            )
            .unwrap();
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
            saved.add_reminder("Stand-up notes", now() + Duration::hours(1), now()).unwrap();
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

    /// A refresh started before Erase finishes after it, and used to write the cache back.
    #[test]
    fn a_refresh_that_finishes_after_erase_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let store = JsonStore::new(dir.path().join("Remora"));
        let mut inbox = Inbox::open(store.clone(), Arc::new(MemoryVault::default()), Managed::default());
        inbox.accounts = self::inbox().accounts;
        let in_flight = snapshot(vec![item("1", InboxBundle::reviews())]);

        inbox.erase_local_data().unwrap();
        inbox.apply(in_flight, now());

        assert!(inbox.all_items().is_empty());
        assert_eq!(crate::store::json_files(&dir.path().join("Remora")).map(|files| files.count()).unwrap_or(0), 0);
    }

    #[test]
    fn results_for_a_disconnected_account_are_dropped() {
        let mut inbox = inbox();
        let mut in_flight = snapshot(vec![item("1", InboxBundle::reviews())]);
        in_flight.push(("a1".into(), Err(PluginError::Network("timeout".into()))));

        inbox.disconnect("a1").unwrap();
        inbox.apply(in_flight, now());

        assert!(inbox.all_items().is_empty());
        assert!(inbox.errors().is_empty());
    }

    /// An account that keeps failing is asked less often; a rate limit holds even for a manual refresh.
    #[test]
    fn failing_and_rate_limited_accounts_sit_refreshes_out() {
        let mut inbox = inbox();
        inbox.accounts[0].settings.insert("host".into(), "https://github.com".into());
        inbox
            .vault
            .save(&HashMap::from([("a1".to_string(), HashMap::from([("token".to_string(), "t".to_string())]))]))
            .unwrap();
        let jobs = |inbox: &mut Inbox, at: DateTime<Utc>, manual: bool| {
            inbox.fetch_jobs(Arc::new(NoNetwork), at, manual).len()
        };
        assert_eq!(jobs(&mut inbox, now(), false), 1);

        let failure = || vec![("a1".to_string(), Err(PluginError::Network("down".into())))];
        inbox.apply(failure(), now());
        assert_eq!(
            jobs(&mut inbox, now() + Duration::minutes(5), false),
            1,
            "one failure: the next refresh tries again"
        );
        inbox.apply(failure(), now());
        assert_eq!(jobs(&mut inbox, now() + Duration::minutes(5), false), 0, "two: skips a refresh");
        assert_eq!(jobs(&mut inbox, now() + Duration::minutes(5), true), 1, "unless the user asks");

        let reset = (now() + Duration::minutes(40)).timestamp();
        inbox.apply(vec![("a1".to_string(), Err(PluginError::RateLimited(Some(reset))))], now());
        assert_eq!(jobs(&mut inbox, now() + Duration::minutes(30), true), 0, "a rate limit holds even then");
        assert_eq!(jobs(&mut inbox, now() + Duration::minutes(41), false), 1);
        assert_eq!(inbox.errors(), ["Too many requests: Remora waits a little before trying again."]);
    }

    fn two_accounts() -> Inbox {
        let mut inbox = inbox();
        inbox.accounts.push(Account {
            id: "a2".into(),
            plugin_id: "linear".into(),
            name: None,
            settings: HashMap::new(),
            identity: None,
        });
        inbox
    }

    /// Every source failing to connect is the network: said once, no slow-down, and it clears on success.
    #[test]
    fn no_network_is_offline_not_failing() {
        let mut inbox = two_accounts();
        let down = |id: &str| (id.to_string(), Err(PluginError::Network("connect".into())));
        inbox.apply(vec![down("a1"), down("a2")], now());
        assert_eq!(inbox.health(), SourcesHealth::Offline);
        assert!(inbox.backoff.is_empty(), "no slow-down for being offline");
        assert_eq!(inbox.errors(), vec![remora_core::OFFLINE.to_string(); 2]);

        inbox.apply(snapshot(vec![]), now());
        inbox.apply(
            vec![("a2".into(), Ok(SourceSnapshot { identity: "bob".into(), items: vec![], remarks: vec![] }))],
            now(),
        );
        assert_eq!(inbox.health(), SourcesHealth::Fine);
    }

    /// One server unreachable while another answers: that server, not the network.
    #[test]
    fn one_unreachable_server_is_failing() {
        let mut inbox = two_accounts();
        let mut results = snapshot(vec![]);
        results.push(("a2".into(), Err(PluginError::Network("connect".into()))));
        inbox.apply(results, now());
        assert_eq!(inbox.health(), SourcesHealth::Failing { count: 1 });
        assert_eq!(inbox.errors(), vec![remora_core::UNREACHABLE.to_string()]);
    }

    /// A rejected token asks for reconnecting; reconnecting keeps the account and its states.
    #[test]
    fn a_rejected_token_is_reconnected_in_place() {
        let mut inbox = inbox();
        inbox.accounts[0].settings.insert("host".into(), "https://github.com".into());
        inbox.apply(snapshot(vec![item("x", InboxBundle::reviews())]), now());
        inbox.toggle_pin("x");
        inbox.apply(vec![("a1".into(), Err(PluginError::Unauthorized))], now());
        assert_eq!(inbox.health(), SourcesHealth::Reconnect { accounts: vec!["a1".into()] });
        assert_eq!(inbox.account_infos()[0].error_kind, Some(FailureKind::Auth));

        let token = HashMap::from([("token".to_string(), "new".to_string())]);
        if let Err(e) = inbox.prepare_reconnect("a1", &token, Arc::new(NoNetwork)) {
            panic!("{e}")
        }
        let fetched = SourceSnapshot {
            identity: "alice".into(),
            items: vec![item("x", InboxBundle::reviews())],
            remarks: vec![],
        };
        inbox.finish_reconnect("a1", token, fetched, now()).unwrap();
        assert_eq!(inbox.accounts.len(), 1, "the same account");
        assert_eq!(inbox.health(), SourcesHealth::Fine);
        assert!(inbox.states.get("x").is_some_and(|s| s.pinned), "its states stay");
        assert_eq!(inbox.secrets().unwrap().get("a1").and_then(|s| s.get("token")).map(String::as_str), Some("new"));
        assert!(inbox.prepare_reconnect("gone", &HashMap::new(), Arc::new(NoNetwork)).is_err());
    }

    // MARK: What snoozes teach

    /// Snoozes are learned from: a history with reasons, Done closing the record, and a loop spotted after three.
    #[test]
    fn snoozes_are_remembered_and_spotted() {
        let mut inbox = inbox();
        inbox.apply(snapshot(vec![item("x", InboxBundle::reviews())]), now());
        for _ in 0..3 {
            inbox.snooze("x", Utc::now() + Duration::hours(2), SnoozeMode::Hide, Some(SnoozeReason::Focus), false);
        }
        assert_eq!(inbox.snooze_count("x", Utc::now()), 3);
        let insights = inbox.insights(Utc::now());
        let id = insights.iter().map(|i| i.id()).find(|id| id.starts_with("loop/")).expect("a loop");
        inbox.dismiss_insight(&id);
        assert!(inbox.insights(Utc::now()).iter().all(|i| i.id() != id), "Not now");
        assert!(inbox.suggested_return(SnoozeReason::NoTime, Utc::now()) > Utc::now());
        inbox.toggle_done("x", Utc::now());
        assert!(inbox.snooze_history.last().is_some_and(|r| r.done_at.is_some()), "Done closes the last snooze");
    }

    /// A reminder isn't a habit: snoozing one teaches nothing.
    #[test]
    fn reminders_are_not_learned_from() {
        let mut inbox = inbox();
        let id = inbox.add_reminder("Call the bank", Utc::now() + Duration::hours(1), Utc::now()).unwrap();
        inbox.snooze(&id, Utc::now() + Duration::hours(2), SnoozeMode::Hide, None, false);
        assert!(inbox.snooze_history.is_empty() && inbox.action_records.is_empty());
    }

    /// A review started then done records how long it really took.
    #[test]
    fn a_timed_review_is_remembered() {
        let mut inbox = inbox();
        let mut review = item("r", InboxBundle::reviews());
        review.changes = Some(remora_core::ChangeSet::new(
            vec![remora_core::ChangedFile { path: "src/retry.ts".into(), additions: Some(40), deletions: Some(0) }],
            None,
        ));
        inbox.apply(snapshot(vec![review]), now());
        assert!(inbox.review_prep(&inbox.item("r").unwrap()).is_some());
        inbox.start("r", Utc::now() - Duration::minutes(12));
        inbox.toggle_done("r", Utc::now());
        assert_eq!(inbox.review_timings.len(), 1);
        assert_eq!(inbox.review_timings[0].actual, 12);
        assert_eq!(inbox.review_pace(), 1.0, "not before five");
    }

    /// Without an allowed assistant, what it wrote goes.
    #[test]
    fn assistant_output_goes_without_an_assistant() {
        let mut inbox = inbox();
        inbox.store_brief(Some(Brief::new("Two reviews wait.", vec![], Utc::now())));
        inbox.triage =
            vec![TriageSuggestion { id: "x".into(), action: TriageAction::Done, until: None, reason: "old".into() }];
        inbox.enforce_ai_policy();
        assert!(inbox.brief.is_none() && inbox.triage.is_empty());
    }

    /// Applying a triage: only what was ticked, and the first "now" item is the one to open.
    #[test]
    fn triage_applies_what_was_ticked() {
        let mut inbox = inbox();
        inbox.apply(
            snapshot(vec![
                item("a", InboxBundle::reviews()),
                item("b", InboxBundle::reviews()),
                item("c", InboxBundle::reviews()),
            ]),
            now(),
        );
        for id in ["a", "b", "c"] {
            inbox.snooze(id, Utc::now() + Duration::days(1), SnoozeMode::Hide, None, false);
        }
        let later = Utc::now() + Duration::days(3);
        let selected = vec![
            TriageSuggestion {
                id: "a".into(),
                action: TriageAction::Reschedule,
                until: Some(later),
                reason: String::new(),
            },
            TriageSuggestion { id: "b".into(), action: TriageAction::Done, until: None, reason: String::new() },
            TriageSuggestion { id: "c".into(), action: TriageAction::Now, until: None, reason: String::new() },
        ];
        inbox.triage = selected.clone();
        let open = inbox.apply_triage(&selected, Utc::now());
        assert_eq!(open.as_deref(), Some("c"));
        assert_eq!(inbox.states["a"].snooze.as_ref().map(|s| s.until), Some(later));
        assert!(inbox.states["b"].done.is_some());
        assert!(inbox.states.get("c").and_then(|s| s.snooze.as_ref()).is_none(), "back in the inbox");
        assert!(inbox.triage.is_empty());
    }

    /// Links between tools follow the items: a Linear ticket and the PR that names it.
    #[test]
    fn items_about_the_same_ticket_are_linked() {
        let mut inbox = two_accounts();
        let mut pr = item("pr", InboxBundle::reviews());
        pr.title = "ENG-42 Fix CSV export".into();
        let mut ticket = item("t", InboxBundle::tasks());
        ticket.account_id = "a2".into();
        ticket.plugin_id = "linear".into();
        ticket.title = "Fix CSV export".into();
        ticket.context = "ENG-42".into();
        inbox.apply(
            vec![
                ("a1".into(), Ok(SourceSnapshot { identity: "alice".into(), items: vec![pr], remarks: vec![] })),
                ("a2".into(), Ok(SourceSnapshot { identity: "alice".into(), items: vec![ticket], remarks: vec![] })),
            ],
            now(),
        );
        assert_eq!(inbox.linked("pr").iter().map(|i| i.id.as_str()).collect::<Vec<_>>(), ["t"]);
    }

    /// The waiting assistant's draft is written locally, from the item, in the interface's words.
    #[test]
    fn a_nudge_is_drafted_for_silent_reviewers() {
        let mut inbox = inbox();
        let mut mine = item("m", InboxBundle::awaiting());
        mine.date = Utc::now() - Duration::days(3);
        mine.participants = vec![remora_core::Person { name: "erin".into(), avatar_url: None, tone: None }];
        inbox.apply(snapshot(vec![mine]), Utc::now());
        let draft = inbox.waiting_draft("m", Utc::now(), &|key| key.to_string()).expect("a nudge");
        assert!(draft.contains("@erin") && draft.contains("3 days"), "{draft}");
    }
}

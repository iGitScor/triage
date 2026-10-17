//! The inbox: one facade over the services it is made of, as on macOS (`InboxModel`).
//!
//! - `accounts.rs`, the accounts service: the accounts, their tokens and sessions; connecting, reconnecting,
//!   disconnecting.
//! - `sync.rs`, the sync engine: fetch jobs, applying their results, slow-downs, notifications.
//! - `snoozes.rs`, the snooze service: the history, the advisor, insights.
//! - `assistant.rs`, the assistant coordinator: the brief, summaries and triage.
//! - `learning.rs`, the learning service: the ranker and review times, learned from your actions.
//! - `persistence.rs`, the persistence port: the files each of them saves.
//! - `policy.rs`, the privacy policy: the preferences with the organization's settings in place.
//!
//! This file holds what they share (the items, their states, reminders, preferences and the organization's settings),
//! the actions on items, and Undo. Fields are private: a change goes through a method that saves it and applies the
//! policy.

mod accounts;
mod assistant;
mod learning;
mod persistence;
mod policy;
mod snoozes;
mod sync;

pub use accounts::AccountsService;
pub use assistant::AssistantCoordinator;
pub use learning::LearningService;
pub use persistence::Persistence;
pub use policy::InboxPolicy;
pub use snoozes::SnoozeService;
pub use sync::SyncEngine;

use crate::{JsonStore, Managed, Preferences, Vault};
use chrono::{DateTime, Utc};
use persistence::{
    ACCOUNTS, AGENT_READS, BRIEF, CACHE, HISTORY, LEARNING, PREFERENCES, REMINDERS, REVIEW_TIMES, STATES, SUMMARIES,
    USAGE,
};
use remora_core::{
    Account, CompliancePolicy, FailureKind, InboxAssembler, InboxBundle, InboxItem, InboxLayout, ItemState, LinkFinder,
    Mark, Message, PluginManifest, Recurrence, RecurrenceRule, SignIn, Snooze, SnoozeMode, SnoozeReason, SourcesHealth,
    VerbClassifier, WaitingAssistant, WaitingHelp,
};
use remora_plugins::{claude, links, registry, AssistantPlugin, HttpClient, PluginError, SourcePlugin, SourceSnapshot};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use ts_rs::TS;

/// What a browser sign-in needs (`Inbox::prepare_sign_in`).
pub struct SignInPlan {
    pub sign_in: SignIn,
    /// The settings its URLs are filled with.
    pub settings: HashMap<String, String>,
    /// Limited to the plugin's hosts.
    pub http: Arc<dyn HttpClient>,
}

/// One account's fetch, built under the lock and run without it.
pub struct FetchJob {
    pub account_id: String,
    pub plugin: Box<dyn SourcePlugin>,
}

pub type FetchResult = (String, Result<SourceSnapshot, PluginError>);
/// The assistant to ask, with its account's id (an answer is kept only while that account still answers).
pub type AssistantJob = (String, Result<Box<dyn AssistantPlugin>, Message>);

/// Runs fetches concurrently.
pub async fn run(jobs: Vec<FetchJob>) -> Vec<FetchResult> {
    futures::future::join_all(jobs.into_iter().map(|job| async move { (job.account_id, job.plugin.fetch().await) }))
        .await
}

/// A source as the Settings screen lists it: its manifest, and why it can't be used, if it can't.
#[derive(Clone, Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename = "Source")]
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
#[derive(Clone, Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename = "Account")]
pub struct AccountInfo {
    #[serde(flatten)]
    pub account: Account,
    pub plugin_name: String,
    pub hosts: Vec<String>,
    pub egress: String,
    pub allowed: bool,
    /// Why it's blocked, when it is.
    pub refusal: Option<String>,
    pub error: Option<Message>,
    /// What fixes it: `auth` gets a Reconnect button.
    pub error_kind: Option<FailureKind>,
    /// What the last fetch said besides its items: a list cut at its limit, a missing permission.
    pub remarks: Vec<Message>,
}

/// What opening an item opens: `app` first when there is one, then `web` if the app didn't open.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct OpenPlan {
    pub app: Option<String>,
    pub web: Option<String>,
}

/// Shown when the organization's `ClaudeCodePath` can't be read: Claude Code isn't run at all.
pub const CLAUDE_CODE_PATH_UNREADABLE: &str =
    "Your organization’s Claude Code path can’t be read, so Claude Code is off. Ask your IT team to check it.";

/// Everything the inbox knows, and every action on it. Ported from the macOS app's `InboxModel`.
pub struct Inbox {
    persistence: Persistence,
    managed: Managed,
    preferences: Preferences,
    accounts: AccountsService,
    sync: SyncEngine,
    snoozes: SnoozeService,
    assistant: AssistantCoordinator,
    learning: LearningService,
    items: HashMap<String, Vec<InboxItem>>,
    states: HashMap<String, ItemState>,
    reminders: Vec<InboxItem>,
    /// Item id → items from other tools about the same thing, worked out when the items change.
    links: HashMap<String, Vec<String>>,
    /// The states and reminders before the last Done or Clear all, until the next one.
    undo_point: Option<(HashMap<String, ItemState>, Vec<InboxItem>)>,
    /// What agents asked through the MCP server, oldest first.
    agent_reads: Vec<remora_core::AgentRead>,
}

impl Inbox {
    /// The real inbox, from its files and the credential store.
    pub fn open(store: JsonStore, vault: Arc<dyn Vault>, managed: Managed) -> Self {
        let files = Persistence::new(Some(store));
        let mut inbox = Inbox {
            preferences: files.load(PREFERENCES).unwrap_or_default(),
            accounts: AccountsService::new(files.load(ACCOUNTS).unwrap_or_default(), vault, files.clone()),
            sync: SyncEngine::default(),
            snoozes: SnoozeService::new(files.load(HISTORY).unwrap_or_default(), files.clone()),
            assistant: AssistantCoordinator::new(
                files.load(BRIEF).unwrap_or_default(),
                files.load(SUMMARIES).unwrap_or_default(),
                files.load(USAGE).unwrap_or_default(),
                files.clone(),
            ),
            // Cached items keep the verbs the refresh gave them.
            items: files.load(CACHE).unwrap_or_default(),
            states: files.load(STATES).unwrap_or_default(),
            reminders: files.load(REMINDERS).unwrap_or_default(),
            learning: LearningService::new(
                files.load(LEARNING).unwrap_or_default(),
                files.load(REVIEW_TIMES).unwrap_or_default(),
                files.clone(),
            ),
            links: HashMap::new(),
            undo_point: None,
            agent_reads: files.load(AGENT_READS).unwrap_or_default(),
            managed,
            persistence: files,
        };
        inbox.recompute_links();
        inbox
    }

    /// An inbox that saves nothing: the demo, and tests.
    pub fn in_memory(vault: Arc<dyn Vault>, managed: Managed) -> Self {
        Inbox::empty(Persistence::default(), vault, managed)
    }

    /// Nothing in it yet, saving to `persistence`.
    fn empty(persistence: Persistence, vault: Arc<dyn Vault>, managed: Managed) -> Self {
        Inbox {
            accounts: AccountsService::new(vec![], vault, persistence.clone()),
            sync: SyncEngine::default(),
            snoozes: SnoozeService::new(vec![], persistence.clone()),
            assistant: AssistantCoordinator::new(None, HashMap::new(), Default::default(), persistence.clone()),
            learning: LearningService::new(vec![], vec![], persistence.clone()),
            managed,
            preferences: Preferences::default(),
            items: HashMap::new(),
            states: HashMap::new(),
            reminders: vec![],
            links: HashMap::new(),
            undo_point: None,
            agent_reads: vec![],
            persistence,
        }
    }

    // MARK: Reading

    pub fn preferences(&self) -> &Preferences {
        &self.preferences
    }

    /// What the organization set (Group Policy, Intune, /etc/remora).
    pub fn managed(&self) -> &Managed {
        &self.managed
    }

    pub fn accounts(&self) -> &[Account] {
        &self.accounts
    }

    /// Done, pins, snoozes and reminders' marks, by item id.
    pub fn states(&self) -> &HashMap<String, ItemState> {
        &self.states
    }

    pub fn last_refresh(&self) -> Option<DateTime<Utc>> {
        self.sync.last_refresh()
    }

    pub fn policy(&self) -> CompliancePolicy {
        self.privacy().compliance()
    }

    /// The organization allows update checks, and the user wants them (or the organization turned them on).
    pub fn checks_for_updates(&self) -> bool {
        self.privacy().checks_for_updates()
    }

    fn allows(&self, plugin_id: &str) -> bool {
        self.privacy().allows(plugin_id)
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
        let score = |item: &InboxItem| self.learning.score(item);
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
                let account = &self.privacy().with_policy_settings(account);
                let refusal = policy.account_refusal(&manifest, &account.settings);
                let failure = self.sync.failure(&account.id);
                Some(AccountInfo {
                    hosts: registry::allowed_hosts(account, &manifest),
                    egress: manifest.egress.description.clone(),
                    allowed: refusal.is_none(),
                    refusal: refusal.map(str::to_string),
                    plugin_name: manifest.name,
                    error: failure.map(|f| f.message.clone()),
                    error_kind: failure.map(|f| f.kind),
                    remarks: self.sync.remarks_of(&account.id),
                    account: account.clone(),
                })
            })
            .collect()
    }

    pub fn errors(&self) -> Vec<Message> {
        self.sync.errors()
    }

    /// What the footer says about the sources.
    pub fn health(&self) -> SourcesHealth {
        self.sync.health()
    }

    pub fn remarks(&self) -> Vec<Message> {
        self.sync.remarks()
    }

    // MARK: Policy

    /// The privacy policy, from the preferences as they are now (`policy.rs`).
    pub fn privacy(&self) -> InboxPolicy<'_> {
        InboxPolicy::new(&self.preferences, &self.managed)
    }

    /// The user's preferences with the organization's settings in place: what Remora acts on.
    pub fn effective_preferences(&self) -> Preferences {
        self.privacy().effective_preferences()
    }

    /// Saves the preferences. True when the refresh interval changed, so the next refresh is rescheduled.
    pub fn set_preferences(&mut self, preferences: Preferences) -> bool {
        let interval_changed = self.preferences.refresh_minutes != preferences.refresh_minutes;
        self.preferences = preferences;
        self.persistence.save(PREFERENCES, &self.preferences);
        // Privacy settings may have just turned the assistant off: what it wrote goes too.
        self.enforce_ai_policy();
        interval_changed
    }

    /// The organization's policy, read again (a change from Group Policy or Intune applies without a restart): what
    /// it no longer allows goes at once.
    pub fn set_managed(&mut self, managed: Managed) {
        self.managed = managed;
        self.enforce_ai_policy();
    }

    // MARK: Actions

    fn update(&mut self, id: &str, change: impl FnOnce(&mut ItemState)) {
        let mut state = self.states.remove(id).unwrap_or_default();
        change(&mut state);
        if !state.is_empty() {
            self.states.insert(id.to_string(), state);
        }
        self.persistence.save(STATES, &self.states);
    }

    /// Opening an item clears its reminder chip.
    pub fn opened(&mut self, id: &str) {
        self.update(id, |s| s.reminded_at = None);
        if let Some(item) = self.item(id) {
            self.learning.handled(&item, Utc::now());
        }
    }

    /// Done until it changes. A reminder marked done is gone for good, unless it repeats: it is then hidden until
    /// its next occurrence, which is returned.
    pub fn toggle_done(&mut self, id: &str, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        let item = self.item(id)?;
        self.remember();
        let marking = self.states.get(id).and_then(|s| s.done.as_ref()).is_none();
        if item.bundle == InboxBundle::reminders() && marking {
            if let Some(next) = item.recurrence.as_ref().and_then(|r| r.next_after(now)) {
                self.snooze(id, next, SnoozeMode::Hide, None, false);
                return Some(next);
            }
            self.reminders.retain(|r| r.id != id);
            self.states.remove(id);
            self.persistence.save(REMINDERS, &self.reminders);
            self.persistence.save(STATES, &self.states);
            return None;
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
            self.learning.handled(&item, now);
            if let Some(started) = started {
                self.learning.reviewed(&item, started, now);
            }
            // The snooze that ended in Done: when you finally do things teaches your best hour.
            self.snoozes.done(id, now);
        }
        None
    }

    /// Empties the Done tab; cleared items still come back on new activity.
    pub fn clear_done(&mut self, now: DateTime<Utc>) {
        // The mark is for the item as it is now: one the tool put in Done (your ✅, a finished conversation) may have
        // no mark, or one from an older message, and must clear too. New activity brings it back.
        let shown: Vec<(String, String)> =
            self.layout(now, "").done.iter().map(|item| (item.id.clone(), item.fingerprint())).collect();
        self.remember();
        for state in self.states.values_mut() {
            if let Some(done) = state.done.as_mut() {
                done.cleared_at.get_or_insert(now);
            }
        }
        for (id, fingerprint) in shown {
            let state = self.states.entry(id).or_default();
            let at = state.done.as_ref().map_or(now, |done| done.at);
            state.done = Some(Mark { at, fingerprint, cleared_at: Some(now) });
        }
        self.persistence.save(STATES, &self.states);
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

    /// A button pressed on a notification. Done on an item already done does nothing, as on macOS.
    pub fn perform(&mut self, action: crate::tray::NotificationAction, id: &str, now: DateTime<Utc>) {
        use crate::tray::NotificationAction::*;
        if self.item(id).is_none() {
            return;
        }
        match action {
            Done if self.states.get(id).and_then(|s| s.done.as_ref()).is_none() => {
                self.toggle_done(id, now);
            }
            Done => {}
            SnoozeTenMinutes => self.snooze(id, now + chrono::Duration::minutes(10), SnoozeMode::Hide, None, false),
            SnoozeHour => self.snooze(id, now + chrono::Duration::hours(1), SnoozeMode::Hide, None, false),
            SnoozeTomorrow => self.snooze(
                id,
                remora_core::SnoozeClock::tomorrow_morning(now.with_timezone(&chrono::Local)),
                SnoozeMode::Hide,
                None,
                false,
            ),
            Start => self.start(id, now),
        }
    }

    /// Puts a started item back where it was before.
    pub fn stop(&mut self, id: &str) {
        self.update(id, |s| s.started_at = None);
    }

    pub fn toggle_pin(&mut self, id: &str) {
        self.update(id, |s| s.pinned = !s.pinned);
    }

    /// The user says a message Remora sorted elsewhere waits on them: it stays in To reply, whatever its words say.
    /// Only towards To reply, so a correction never hides a request; Undo puts it back.
    pub fn move_to_reply(&mut self, id: &str, now: DateTime<Utc>) {
        if !self.item(id).is_some_and(|item| item.can_move_to_reply()) {
            return;
        }
        self.remember();
        self.update(id, |s| s.moved_to_reply_at = Some(now));
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
            self.learning.deferred(&item, Utc::now());
            self.snoozes.record(&item, reason, Utc::now(), until);
        }
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

    pub fn snooze_many(
        &mut self,
        ids: &[String],
        until: DateTime<Utc>,
        reason: Option<SnoozeReason>,
        until_news: bool,
    ) {
        self.remember();
        for id in ids {
            self.snooze(id, until, SnoozeMode::Hide, reason, until_news);
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

    /// A reminder is an item of its own, hidden until its time. It needs a title. With `repeat`, Done brings it
    /// back at the next occurrence, up to `until` (the last day, inclusive).
    pub fn add_reminder(
        &mut self,
        title: &str,
        at: DateTime<Utc>,
        repeat: Option<RecurrenceRule>,
        until: Option<DateTime<Utc>>,
        now: DateTime<Utc>,
    ) -> Result<String, Message> {
        if title.trim().is_empty() {
            return Err("Type what to remember.".into());
        }
        let recurrence = repeat.map(|rule| Recurrence::new(rule, at, until));
        if recurrence.as_ref().is_some_and(Recurrence::ends_before_start) {
            return Err("The end is before the first reminder.".into());
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
            recurrence,
            reactions: None,
            reason: None,
        });
        self.persistence.save(REMINDERS, &self.reminders);
        self.snooze(&id, at, SnoozeMode::Hide, None, false);
        Ok(id)
    }

    /// The reminder stays as it is, but Done will end it.
    pub fn stop_repeating(&mut self, id: &str) -> bool {
        let Some(reminder) = self.reminders.iter_mut().find(|r| r.id == id && r.recurrence.is_some()) else {
            return false;
        };
        reminder.recurrence = None;
        self.persistence.save(REMINDERS, &self.reminders);
        true
    }

    /// What opening an item opens, as a click in the inbox or a notification does: the tool's app when the
    /// preference allows it (`app`, tried first), else the web page (`web`). Only web pages and the tools' apps
    /// are ever opened (`links`). The item stops being reminded.
    pub fn open_plan(&mut self, id: &str, in_browser: bool) -> Result<OpenPlan, Message> {
        let item = self.item(id).ok_or("This item is gone.")?;
        self.opened(id);
        let http_hosts = links::http_hosts(self.accounts.find(&item.account_id));
        let opens_apps = self.effective_preferences().open_in_apps;
        let app = item.app_url.filter(|url| !in_browser && opens_apps && links::is_app_link(url));
        let web = item.url.filter(|url| links::is_web_link(url, &http_hosts));
        Ok(OpenPlan { app, web })
    }

    // MARK: Help on a row

    /// Who could review your PR, or a nudge when its reviewers stay silent. Asked for every shown item: one that
    /// isn't waiting on anyone returns at once, and the others read the inbox by reference, never copying it.
    pub fn waiting_help(&self, item: &InboxItem, now: DateTime<Utc>) -> Option<WaitingHelp> {
        if item.bundle != InboxBundle::awaiting() {
            return None;
        }
        let me = self.accounts.find(&item.account_id).and_then(|a| a.identity.as_deref());
        let items: Vec<&InboxItem> = self
            .accounts
            .iter()
            .filter(|a| self.allows(&a.plugin_id))
            .flat_map(|a| self.items.get(&a.id).into_iter().flatten())
            .collect();
        WaitingAssistant::default().help(item, &items, me, now)
    }

    /// The message to paste for this help: who could review, or a kind nudge. Nothing is sent.
    pub fn waiting_draft(&self, id: &str, now: DateTime<Utc>, tr: &dyn Fn(&str) -> String) -> Option<String> {
        let item = self.item(id)?;
        let help = self.waiting_help(&item, now)?;
        Some(WaitingAssistant::default().draft(&help, &item, tr))
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

    fn recompute_links(&mut self) {
        self.links = LinkFinder::default().links(&self.all_items());
    }

    // MARK: Undo

    fn remember(&mut self) {
        self.undo_point = Some((self.states.clone(), self.reminders.clone()));
    }

    /// Puts back what the last Done or Clear all changed: a reminder marked done comes back too. False when there's
    /// nothing to undo.
    pub fn undo(&mut self) -> bool {
        let Some((states, reminders)) = self.undo_point.take() else { return false };
        self.states = states;
        self.reminders = reminders;
        self.persistence.save(STATES, &self.states);
        self.persistence.save(REMINDERS, &self.reminders);
        true
    }

    // MARK: Files

    /// The files, for the app to flush when it quits.
    pub fn store(&self) -> Option<JsonStore> {
        self.persistence.store()
    }

    /// An agent's request through the MCP server, kept for Settings: the most recent `AGENT_READS_KEPT`.
    pub fn record_agent_read(&mut self, read: remora_core::AgentRead) {
        self.agent_reads.push(read);
        let excess = self.agent_reads.len().saturating_sub(remora_core::AGENT_READS_KEPT);
        self.agent_reads.drain(..excess);
        self.persistence.save(AGENT_READS, &self.agent_reads);
    }

    /// Whether the `remora` command and the MCP server may read the inbox (`AllowLocalAPI`).
    pub fn local_api_allowed(&self) -> bool {
        self.managed.local_api_allowed()
    }

    /// What agents asked, most recent first.
    pub fn agent_reads(&self) -> Vec<remora_core::AgentRead> {
        self.agent_reads.iter().rev().cloned().collect()
    }

    /// Settings → Privacy → Erase local data, the tokens already deleted (`Tokens::erase`). The files go under the
    /// lock, so no save can bring one back.
    pub fn erase_local_data_saved(&mut self) -> Result<(), Message> {
        self.sync.forget_remarks();
        self.undo_point = None;
        self.persistence.erase()?;
        let (persistence, vault, managed) = (self.persistence.clone(), self.accounts.vault(), self.managed.clone());
        *self = Inbox::empty(persistence, vault, managed);
        Ok(())
    }

    /// Demo items, as if an account had synced: see `demo.rs`.
    pub fn load_demo(
        &mut self,
        accounts: Vec<Account>,
        items: HashMap<String, Vec<InboxItem>>,
        states: HashMap<String, ItemState>,
    ) {
        self.accounts.replace_all(accounts);
        self.items = items
            .into_iter()
            .map(|(id, items)| (id, items.into_iter().map(VerbClassifier::classify).collect()))
            .collect();
        self.sync.synced_all(self.items.keys().cloned(), Utc::now());
        self.states = states;
    }

    /// The accounts, for tests to arrange.
    #[cfg(test)]
    pub(crate) fn accounts_mut(&mut self) -> &mut Vec<Account> {
        self.accounts.list_mut()
    }

    /// The preferences and the policy, for tests to arrange without saving or enforcing anything.
    #[cfg(test)]
    pub(crate) fn arrange(&mut self) -> (&mut Preferences, &mut Managed) {
        (&mut self.preferences, &mut self.managed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MemoryVault;
    use crate::Secrets;
    use chrono::Duration;
    use remora_core::{AssistantPolicy, Brief, BundleSummary, NoticeKind, TriageAction, TriageSuggestion};
    use remora_core::{Badge, EXTERNAL_AI_OFF, LOCAL_AI_NOT_ALLOWED, SERVER_NOT_ALLOWED};
    use remora_plugins::{Request, Response};
    use std::collections::HashSet;

    /// Runs a token change as the app does, through `Tokens`, on an inbox the test owns:
    /// `saved!(inbox, |tokens, shared| tokens.disconnect(&shared, "a1"))`.
    macro_rules! saved {
        ($inbox:expr, |$tokens:ident, $shared:ident| $call:expr) => {{
            let vault = $inbox.vault();
            let placeholder = Inbox::in_memory(vault.clone(), Managed::default());
            let $shared = tokio::sync::Mutex::new(std::mem::replace(&mut $inbox, placeholder));
            let $tokens = crate::Tokens::new(Arc::new(crate::InPlace(vault)));
            let result = futures::executor::block_on($call);
            $inbox = $shared.into_inner();
            result
        }};
    }

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
            recurrence: None,
            reactions: None,
            reason: None,
        }
    }

    fn inbox() -> Inbox {
        let mut inbox = Inbox::in_memory(Arc::new(MemoryVault::default()), Managed::default());
        *inbox.accounts_mut() = vec![Account {
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

    /// Put in To reply by the user: it stays there across refreshes, says so, and Undo puts it back. A message already
    /// there, or what the source sorted, can't be moved.
    #[test]
    fn a_message_put_in_to_reply_stays_there_until_undone() {
        let mut inbox = inbox();
        let mut news = item("n", InboxBundle::mentions());
        news.title = "FYI the build is green".into();
        inbox.apply(snapshot(vec![news.clone(), item("r", InboxBundle::reviews())]), now());
        let bundle = |inbox: &Inbox| {
            let layout = inbox.layout(now(), "");
            let found = layout.my_turn_items().find(|i| i.id == "n").cloned().expect("shown");
            (found.bundle, found.reason.map(|r| r.code))
        };
        assert_eq!(bundle(&inbox), (InboxBundle::read(), Some("info".to_string())));

        inbox.move_to_reply("n", now());
        inbox.move_to_reply("r", now());
        assert_eq!(bundle(&inbox), (InboxBundle::reply(), Some("moved".to_string())));
        assert!(!inbox.states.contains_key("r"), "the source sorted it: nothing to correct");
        inbox.apply(snapshot(vec![news, item("r", InboxBundle::reviews())]), now());
        assert_eq!(bundle(&inbox).0, InboxBundle::reply(), "still there after a refresh");

        assert!(inbox.undo());
        assert_eq!(bundle(&inbox).0, InboxBundle::read());
    }

    /// After a launch, the first sync compares with the saved copy: what arrived meanwhile is announced.
    #[test]
    fn what_arrived_while_closed_is_announced() {
        let mut inbox = inbox();
        inbox.items.insert("a1".into(), vec![item("old", InboxBundle::reviews())]);
        let found = inbox
            .apply(snapshot(vec![item("old", InboxBundle::reviews()), item("new", InboxBundle::reviews())]), now());
        assert_eq!(found.iter().map(|n| n.item_id.as_str()).collect::<Vec<_>>(), ["new"]);
    }

    /// A just-connected account has no saved copy: its first sync stays silent.
    #[test]
    fn a_first_sync_without_a_saved_copy_is_silent() {
        let mut inbox = inbox();
        assert!(inbox.apply(snapshot(vec![item("1", InboxBundle::reviews())]), now()).is_empty());
    }

    /// More than three while closed: one notification with the count, which opens Remora.
    #[test]
    fn many_arrivals_while_closed_become_one_notice() {
        let mut inbox = inbox();
        inbox.items.insert("a1".into(), vec![]);
        let items = (1..=5).map(|i| item(&i.to_string(), InboxBundle::reviews())).collect();
        let found = inbox.apply(snapshot(items), now());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].item_id, "a1/while-closed");
        assert_eq!(found[0].subtitle, "GitHub");
        assert_eq!(found[0].count, Some(5));
        // Later syncs announce one by one again.
        let more = (6..=9).map(|i| item(&i.to_string(), InboxBundle::reviews()));
        let all = (1..=5).map(|i| item(&i.to_string(), InboxBundle::reviews())).chain(more).collect();
        assert_eq!(inbox.apply(snapshot(all), now()).len(), 4);
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
        let id = inbox
            .add_reminder("Call the bank", now() - Duration::minutes(1), None, None, now() - Duration::hours(1))
            .unwrap();
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
        let reminder = inbox
            .add_reminder("Call the bank", now() - Duration::minutes(1), None, None, now() - Duration::hours(1))
            .unwrap();
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

    /// Clear all also clears what the tool put in Done (your ✅, a finished conversation); a new message brings it
    /// back.
    #[test]
    fn clear_all_clears_what_the_tool_put_in_done() {
        let mut inbox = inbox();
        let mut handled = item("1", InboxBundle::reply());
        handled.reactions = Some(vec![remora_core::Reaction::new("white_check_mark", 1, true)]);
        let finished = item("2", InboxBundle::finished());
        // Marked done days ago, on an older message: a stale mark doesn't keep it from clearing.
        let stale = Mark { at: now(), fingerprint: "0|".into(), cleared_at: Some(now()) };
        inbox.states.insert("2".into(), ItemState { done: Some(stale), ..Default::default() });
        inbox.apply(snapshot(vec![handled, finished]), now());
        assert_eq!(inbox.layout(now(), "").done.len(), 2);

        inbox.clear_done(now());
        assert!(inbox.layout(now(), "").done.is_empty());

        let mut newer = item("1", InboxBundle::reply());
        newer.date = now() + Duration::minutes(5);
        inbox.apply(snapshot(vec![newer]), now());
        assert_eq!(inbox.layout(now(), "").my_turn.len(), 1, "a new message brings it back");
    }

    #[test]
    fn a_reminder_needs_a_title() {
        let mut inbox = inbox();
        assert_eq!(inbox.add_reminder("   ", now(), None, None, now()), Err("Type what to remember.".into()));
        assert!(inbox.reminders.is_empty());
    }

    /// Done on a repeating reminder hides it until the next occurrence, undoably; past the last day it ends.
    #[test]
    fn a_repeating_reminder_comes_back_after_done() {
        let mut inbox = inbox();
        let at = now() - Duration::minutes(1);
        let id =
            inbox.add_reminder("Stand-up", at, Some(RecurrenceRule::Daily), None, now() - Duration::hours(1)).unwrap();
        inbox.tick(now());

        let next = inbox.toggle_done(&id, now()).expect("a next occurrence");
        assert!((23..=25).contains(&(next - at).num_hours()), "the same time the next day");
        assert!(inbox.item(&id).is_some(), "still there");
        let state = &inbox.states[&id];
        assert_eq!(state.snooze.as_ref().map(|s| s.until), Some(next));
        assert!(state.reminded_at.is_none() && state.done.is_none());
        assert!(inbox.snoozes.history().is_empty(), "a reminder isn't counted as a snooze");

        assert!(inbox.undo());
        assert!(inbox.states[&id].reminded_at.is_some() && inbox.states[&id].snooze.is_none());

        let last =
            inbox.add_reminder("Once more", at, Some(RecurrenceRule::Daily), Some(at), now() - Duration::hours(1));
        let last = last.unwrap();
        inbox.tick(now());
        assert_eq!(inbox.toggle_done(&last, now()), None);
        assert!(inbox.item(&last).is_none(), "past its last day, it ends");
    }

    #[test]
    fn stop_repeating_keeps_the_reminder() {
        let mut inbox = inbox();
        let at = now() - Duration::minutes(1);
        let id =
            inbox.add_reminder("Stand-up", at, Some(RecurrenceRule::Weekly), None, now() - Duration::hours(1)).unwrap();
        inbox.tick(now());
        assert!(inbox.stop_repeating(&id));
        assert!(!inbox.stop_repeating(&id), "already stopped");
        assert_eq!(inbox.item(&id).map(|i| i.recurrence), Some(None));
        assert!(inbox.states[&id].reminded_at.is_some(), "nothing else changes");
        assert_eq!(inbox.toggle_done(&id, now()), None);
        assert!(inbox.item(&id).is_none());
    }

    #[test]
    fn a_repeating_reminder_cannot_end_before_it_starts() {
        let mut inbox = inbox();
        let at = now() + Duration::days(3);
        let ended = inbox.add_reminder("Stand-up", at, Some(RecurrenceRule::Daily), Some(now()), now());
        assert_eq!(ended, Err("The end is before the first reminder.".into()));
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

    /// Grouped at set times, arrivals wait for the next one, then come as they are or as one; reminders
    /// don't wait, and turning grouping off sends what waited.
    #[test]
    fn notifications_grouped_at_set_times_wait_for_the_next_one() {
        let mut inbox = inbox();
        inbox.preferences.batch_notifications = true;
        let at = |minutes: i64| now() + Duration::minutes(minutes);
        let next_time = (now() + Duration::hours(1)).with_timezone(&chrono::Local).format("%H:%M").to_string();
        inbox.preferences.notification_times = next_time;
        inbox.sync.batched(now());
        inbox.apply(snapshot(vec![item("1", InboxBundle::reviews())]), now());
        let arrived: Vec<InboxItem> = (1..=5).map(|i| item(&i.to_string(), InboxBundle::reviews())).collect();
        assert!(inbox.apply(snapshot(arrived), at(1)).is_empty(), "held until the set time");
        assert!(inbox.tick(at(30)).is_empty(), "not yet");
        let batch = inbox.tick(at(61));
        assert_eq!((batch.len(), batch[0].count), (1, Some(4)), "four arrivals, as one");
        assert!(inbox.tick(at(62)).is_empty(), "sent once");

        let more = (1..=6).map(|i| item(&i.to_string(), InboxBundle::reviews())).collect();
        assert!(inbox.apply(snapshot(more), at(63)).is_empty());
        inbox.preferences.batch_notifications = false;
        assert_eq!(inbox.tick(at(64)).len(), 1, "turned off: what waited goes at once");
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

    /// A notification's buttons: Done (once), snooze for an hour or until tomorrow morning, Start.
    #[test]
    fn notification_buttons_act_on_their_item() {
        use crate::tray::NotificationAction;
        let mut inbox = inbox();
        inbox.apply(
            snapshot(vec![
                item("1", InboxBundle::reviews()),
                item("2", InboxBundle::reviews()),
                item("3", InboxBundle::reviews()),
            ]),
            now(),
        );
        inbox.perform(NotificationAction::Done, "1", now());
        inbox.perform(NotificationAction::Done, "1", now());
        assert!(inbox.states["1"].done.is_some(), "Done twice stays done");
        inbox.perform(NotificationAction::SnoozeHour, "2", now());
        assert_eq!(inbox.states["2"].snooze.as_ref().map(|s| s.until), Some(now() + Duration::hours(1)));
        inbox.perform(NotificationAction::SnoozeTomorrow, "3", now());
        let back = inbox.states["3"].snooze.as_ref().unwrap().until.with_timezone(&chrono::Local);
        assert_eq!(
            (back.date_naive(), back.format("%H:%M").to_string()),
            ((now() + Duration::days(1)).with_timezone(&chrono::Local).date_naive(), "09:00".into())
        );
        inbox.perform(NotificationAction::Start, "3", now());
        assert!(inbox.states["3"].started_at.is_some() && inbox.states["3"].snooze.is_none());
        inbox.perform(NotificationAction::Done, "gone", now());
        assert!(!inbox.states.contains_key("gone"), "an item no longer there is left alone");
    }

    /// *Snooze all…* on a group: the same date and reason for each, and "until there's news" kept.
    #[test]
    fn a_whole_group_is_snoozed_at_once() {
        let mut inbox = inbox();
        inbox.apply(snapshot(vec![item("1", InboxBundle::reviews()), item("2", InboxBundle::reviews())]), now());
        let ids = ["1".to_string(), "2".to_string()];
        inbox.snooze_many(&ids, now() + Duration::days(1), Some(SnoozeReason::Waiting), true);
        assert_eq!(inbox.layout(now(), "").snoozed.len(), 2);
        assert!(ids.iter().all(|id| inbox.states[id]
            .snooze
            .as_ref()
            .is_some_and(|s| s.until_news == Some(true) && s.reason == Some(SnoozeReason::Waiting))));
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
        let id = inbox.add_reminder("  Call the bank ", now() + Duration::minutes(10), None, None, now()).unwrap();
        assert_eq!(inbox.layout(now(), "").snoozed.len(), 1);
        let notices = inbox.tick(now() + Duration::minutes(10));
        assert_eq!((notices[0].title.as_str(), notices[0].body.as_str()), ("Reminder", "Call the bank"));
        assert_eq!(notices[0].subtitle, "", "not \"Reminder\" twice");
        assert_eq!(inbox.layout(now() + Duration::minutes(10), "").my_turn_items().count(), 1);
        inbox.toggle_done(&id, now());
        assert!(inbox.all_items().is_empty());
    }

    #[test]
    fn the_brief_sees_the_task_in_progress_marked_so() {
        let mut inbox = inbox();
        inbox.apply(snapshot(vec![item("1", InboxBundle::reviews()), item("2", InboxBundle::reviews())]), now());
        inbox.start("1", now());
        let items = inbox.brief_items(now());
        assert_eq!(items.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(), ["1", "2"], "in progress first");
        assert!(items[0].has_badge("inProgress") && !items[1].has_badge("inProgress"));
    }

    #[test]
    fn the_policy_refuses_accounts_and_connections() {
        let mut inbox = inbox();
        inbox.apply(snapshot(vec![item("1", InboxBundle::reviews())]), now());
        inbox.managed = Managed { allowed_plugins: Some(vec!["slack".into()]), ..Managed::default() };
        assert!(inbox.all_items().is_empty(), "items of a refused tool disappear");
        inbox.provide_secrets(Secrets::new());
        assert!(inbox.fetch_jobs(Arc::new(NoNetwork), now(), false).is_empty());
        assert_eq!(inbox.errors(), ["Not allowed by your privacy policy."]);
        let refused = inbox.prepare_connect("github", None, HashMap::new(), &HashMap::new(), Arc::new(NoNetwork));
        assert_eq!(refused.err().map(|e| e.text).as_deref(), Some("Not allowed by your privacy policy."));
        assert!(!inbox.sources().iter().find(|s| s.manifest.id == "github").unwrap().refusal.is_none());
    }

    #[test]
    fn only_a_plugin_with_a_sign_in_signs_in_through_the_browser() {
        let inbox = inbox();
        let refused = inbox.prepare_sign_in("linear", None, HashMap::new(), Arc::new(NoNetwork));
        assert_eq!(refused.err().map(|e| e.text).as_deref(), Some("Linear has no sign-in."));
        let gone = inbox.prepare_sign_in("linear", Some("gone"), HashMap::new(), Arc::new(NoNetwork));
        assert!(gone.is_err());
    }

    #[test]
    fn the_organizations_tenant_and_app_replace_what_was_typed() {
        let mut inbox = inbox();
        let typed = HashMap::from([("tenant".to_string(), "evil.com".to_string()), ("clientId".into(), "x".into())]);
        let plan = inbox.prepare_sign_in("microsoft", None, typed.clone(), Arc::new(NoNetwork)).unwrap();
        assert_eq!(plan.settings["tenant"], "evil.com", "nothing managed: the user's");
        inbox.managed.settings = std::collections::BTreeMap::from([
            ("MicrosoftTenant".to_string(), "acme.onmicrosoft.com".to_string()),
            ("MicrosoftClientID".to_string(), "app-1".to_string()),
        ]);
        let plan = inbox.prepare_sign_in("microsoft", None, typed, Arc::new(NoNetwork)).unwrap();
        assert_eq!(
            (plan.settings["tenant"].as_str(), plan.settings["clientId"].as_str()),
            ("acme.onmicrosoft.com", "app-1")
        );
        let blocked = inbox.prepare_sign_in(
            "microsoft",
            None,
            HashMap::from([("tenant".to_string(), "a/b".to_string())]),
            Arc::new(NoNetwork),
        );
        assert!(blocked.is_ok(), "the organization's tenant, not the typed one");
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
        let snapshot = SourceSnapshot { identity: "Alice".into(), items: vec![], remarks: vec![] };
        saved!(inbox, |tokens, shared| tokens.connect(&shared, account, secrets, snapshot, now())).unwrap();
        assert_eq!(vault.load().unwrap()[&id]["token"], "lin_api_k");
        assert_eq!(inbox.account_infos()[0].hosts, ["api.linear.app", "public.linear.app"]);

        saved!(inbox, |tokens, shared| tokens.disconnect(&shared, &id)).unwrap();
        assert!(inbox.accounts.is_empty() && vault.load().unwrap().is_empty());
    }

    /// A GitLab that counts how often it is asked who the token belongs to.
    #[derive(Default)]
    struct CountingGitLab {
        identity_calls: std::sync::atomic::AtomicUsize,
    }

    #[async_trait::async_trait]
    impl HttpClient for CountingGitLab {
        async fn send(&self, request: remora_plugins::Request) -> Result<remora_plugins::Response, PluginError> {
            let body = if request.url.contains("/api/v4/user") {
                self.identity_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                r#"{"username": "alice"}"#
            } else {
                "[]"
            };
            Ok(remora_plugins::Response { status: 200, body: body.into(), etag: None, reset: None })
        }
    }

    impl CountingGitLab {
        fn calls(&self) -> usize {
            self.identity_calls.load(std::sync::atomic::Ordering::SeqCst)
        }
    }

    async fn refresh(inbox: &mut Inbox, http: &Arc<CountingGitLab>) {
        for job in inbox.fetch_jobs(http.clone(), now(), true) {
            job.plugin.fetch().await.unwrap();
        }
    }

    /// The identity a session learned is reused across refreshes, and asked again where it may have
    /// changed: connecting always tests the token, a new token starts over, and so does connecting again after a
    /// disconnect.
    #[tokio::test]
    async fn the_identity_is_asked_once_per_session() {
        let http = Arc::new(CountingGitLab::default());
        let mut inbox = Inbox::in_memory(Arc::new(MemoryVault::default()), Managed::default());
        let settings = HashMap::from([("host".to_string(), "gitlab.acme.io".to_string())]);
        let token = |t: &str| HashMap::from([("token".to_string(), t.to_string())]);
        let connect = |inbox: &mut Inbox, http: &Arc<CountingGitLab>| {
            inbox.prepare_connect("gitlab", None, settings.clone(), &token("t1"), http.clone()).unwrap()
        };

        let (account, plugin) = connect(&mut inbox, &http);
        let snapshot = plugin.fetch().await.unwrap();
        assert_eq!(http.calls(), 1, "connecting asks: it tests the token");
        let id = account.id.clone();
        saved!(inbox, |tokens, shared| tokens.connect(&shared, account, token("t1"), snapshot, now())).unwrap();
        refresh(&mut inbox, &http).await;
        refresh(&mut inbox, &http).await;
        assert_eq!(http.calls(), 2, "the first refresh asks, the next ones remember");

        let plugin = inbox.prepare_reconnect(&id, &token("t2"), http.clone()).unwrap();
        let snapshot = plugin.fetch().await.unwrap();
        saved!(inbox, |tokens, shared| tokens.reconnect(&shared, &id, token("t2"), Some(snapshot), now())).unwrap();
        refresh(&mut inbox, &http).await;
        assert_eq!(http.calls(), 4, "a new token: reconnecting asks, and the session starts over");

        saved!(inbox, |tokens, shared| tokens.disconnect(&shared, &id)).unwrap();
        let (account, plugin) = connect(&mut inbox, &http);
        let snapshot = plugin.fetch().await.unwrap();
        saved!(inbox, |tokens, shared| tokens.connect(&shared, account, token("t1"), snapshot, now())).unwrap();
        refresh(&mut inbox, &http).await;
        assert_eq!(http.calls(), 6, "after a disconnect, nothing was kept");
    }

    /// What a plugin renewed reaches the vault through the app, and only for accounts still here.
    #[test]
    fn renewed_secrets_are_saved_for_accounts_still_here() {
        let vault = Arc::new(MemoryVault::default());
        let mut inbox = Inbox::in_memory(vault.clone(), Managed::default());
        let secrets = HashMap::from([("token".to_string(), "lin_k".to_string())]);
        let (account, _) =
            inbox.prepare_connect("linear", None, HashMap::new(), &secrets, Arc::new(NoNetwork)).unwrap();
        let id = account.id.clone();
        let snapshot = SourceSnapshot { identity: "Alice".into(), items: vec![], remarks: vec![] };
        saved!(inbox, |tokens, shared| tokens.connect(&shared, account, secrets.clone(), snapshot, now())).unwrap();
        let settings = HashMap::new();
        let (cache, _) = inbox.accounts.session(&id, &settings, &secrets);
        cache.renew("refreshToken", "r2".into());
        let (gone, _) = inbox.accounts.session("gone", &settings, &secrets);
        gone.renew("refreshToken", "x".into());
        saved!(inbox, |tokens, shared| tokens.save_renewed(&shared)).unwrap();
        let saved = vault.load().unwrap();
        assert_eq!(saved.keys().collect::<Vec<_>>(), [&id], "nothing for an account no longer here");
        assert_eq!((saved[&id]["token"].as_str(), saved[&id]["refreshToken"].as_str()), ("lin_k", "r2"));
        assert_eq!(inbox.accounts.secrets().unwrap(), &saved, "the inbox uses them from now on");
    }

    /// Connecting the same account again (same person, tool and server) updates it: no second account, no
    /// duplicate items, its states and new name kept, one token in the vault. As on macOS.
    #[test]
    fn connecting_the_same_account_again_updates_it() {
        let vault = Arc::new(MemoryVault::default());
        let mut inbox = Inbox::in_memory(vault.clone(), Managed::default());
        let connect = |inbox: &mut Inbox, name: Option<&str>, token: &str| {
            let secrets = HashMap::from([("token".to_string(), token.to_string())]);
            let (account, _) = inbox
                .prepare_connect("linear", name.map(Into::into), HashMap::new(), &secrets, Arc::new(NoNetwork))
                .unwrap();
            let mut issue = item("x", InboxBundle::tasks());
            issue.id = InboxItem::make_id(&account.id, "issue-1");
            issue.account_id = account.id.clone();
            let snapshot = SourceSnapshot { identity: "Alice".into(), items: vec![issue], remarks: vec![] };
            saved!(*inbox, |tokens, shared| tokens.connect(&shared, account, secrets, snapshot, now())).unwrap();
        };
        connect(&mut inbox, None, "lin_old");
        let id = inbox.accounts[0].id.clone();
        let issue = inbox.all_items()[0].id.clone();
        inbox.toggle_pin(&issue);

        connect(&mut inbox, Some("Work"), "lin_new");
        assert_eq!(
            inbox.accounts.iter().map(|a| a.id.as_str()).collect::<Vec<_>>(),
            [id.as_str()],
            "no second account"
        );
        assert_eq!(inbox.accounts[0].name.as_deref(), Some("Work"));
        assert_eq!(
            inbox.all_items().iter().map(|i| i.id.as_str()).collect::<Vec<_>>(),
            [issue.as_str()],
            "no duplicate"
        );
        assert!(inbox.states[&issue].pinned, "states kept");
        let saved = vault.load().unwrap();
        assert_eq!((saved.len(), saved[&id]["token"].as_str()), (1, "lin_new"));
    }

    /// Same person and tool, another server: another account. Another person: another account.
    #[test]
    fn the_same_tool_on_another_host_is_another_account() {
        let mut inbox = inbox();
        let base = inbox.accounts[0].clone();
        let account = |host: &str| Account {
            plugin_id: "gitlab".into(),
            settings: HashMap::from([("host".to_string(), host.to_string())]),
            ..base.clone()
        };
        let mut saved = account("gitlab.acme.io");
        saved.id = "saved".into();
        saved.identity = Some("alice".into());
        inbox.accounts_mut().push(saved);
        assert!(inbox.existing_account(&account("https://gitlab.acme.io/"), "Alice").is_some());
        assert!(inbox.existing_account(&account("gitlab.other.io"), "alice").is_none());
        assert!(inbox.existing_account(&account("gitlab.acme.io"), "bob").is_none());
        assert!(inbox.existing_account(&account("gitlab.acme.io"), "").is_none());
        // Two Azure DevOps organizations on one host are two accounts.
        let org = |host: &str| Account { plugin_id: "azuredevops".into(), ..account(host) };
        let mut acme = org("https://dev.azure.com/acme/");
        acme.id = "acme".into();
        acme.identity = Some("alice".into());
        inbox.accounts_mut().push(acme);
        assert!(inbox.existing_account(&org("dev.azure.com/Acme"), "alice").is_some());
        assert!(inbox.existing_account(&org("dev.azure.com/globex"), "alice").is_none());
    }

    #[test]
    fn everything_is_saved_and_erasing_removes_it() {
        let dir = tempfile::tempdir().unwrap();
        let store = JsonStore::new(dir.path().join("Remora"));
        let vault: Arc<dyn Vault> = Arc::new(MemoryVault::default());
        {
            let mut saved = Inbox::open(store.clone(), vault.clone(), Managed::default());
            *saved.accounts_mut() = inbox().accounts().to_vec();
            saved.apply(snapshot(vec![item("1", InboxBundle::reviews())]), now());
            saved.toggle_pin("1");
            saved.add_reminder("Stand-up notes", now() + Duration::hours(1), None, None, now()).unwrap();
            saved.set_preferences(Preferences { refresh_minutes: 15, ..Preferences::default() });
        }
        let mut reopened = Inbox::open(store.clone(), vault, Managed::default());
        assert_eq!(reopened.all_items().len(), 2);
        assert!(reopened.states["1"].pinned);
        assert_eq!(reopened.preferences.refresh_minutes, 15);

        saved!(reopened, |tokens, shared| tokens.erase(&shared)).unwrap();
        assert!(reopened.all_items().is_empty());
        assert!(store.load::<Vec<Account>>(ACCOUNTS).is_none());
    }

    /// A refresh that brings nothing new writes no file.
    #[test]
    fn a_refresh_with_nothing_new_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("Remora");
        let store = JsonStore::new(folder.clone());
        let mut inbox = Inbox::open(store.clone(), Arc::new(MemoryVault::default()), Managed::default());
        *inbox.accounts_mut() = self::inbox().accounts().to_vec();
        inbox.apply(snapshot(vec![item("1", InboxBundle::reviews())]), now());
        store.flush();
        for name in [ACCOUNTS, CACHE, STATES] {
            std::fs::write(folder.join(format!("{name}.json")), "untouched").unwrap();
        }

        inbox.apply(snapshot(vec![item("1", InboxBundle::reviews())]), now());
        store.flush();
        for name in [ACCOUNTS, CACHE, STATES] {
            assert_eq!(std::fs::read_to_string(folder.join(format!("{name}.json"))).unwrap(), "untouched", "{name}");
        }

        inbox.toggle_pin("1");
        store.flush();
        assert_ne!(std::fs::read_to_string(folder.join(format!("{STATES}.json"))).unwrap(), "untouched");
    }

    /// The app reads and saves tokens through `Tokens`, outside the lock: the other accounts' tokens stay.
    #[test]
    fn tokens_are_read_and_saved_outside_the_inbox() {
        let vault = Arc::new(MemoryVault::default());
        vault.save(&Secrets::from([("old".to_string(), HashMap::new())])).unwrap();
        let mut inbox = Inbox::in_memory(vault.clone(), Managed::default());
        assert!(inbox.needs_secrets());
        saved!(inbox, |tokens, shared| async {
            tokens.load_once(&shared).await?;
            Ok::<_, Message>(())
        })
        .unwrap();
        assert!(!inbox.needs_secrets());

        let secrets = HashMap::from([("token".to_string(), "lin_api_k".to_string())]);
        let (account, _) =
            inbox.prepare_connect("linear", None, HashMap::new(), &secrets, Arc::new(NoNetwork)).unwrap();
        let id = account.id.clone();
        let identity = SourceSnapshot { identity: "Alice".into(), items: vec![], remarks: vec![] };
        saved!(inbox, |tokens, shared| tokens.connect(&shared, account, secrets, identity, now())).unwrap();
        assert_eq!(vault.load().unwrap()[&id]["token"], "lin_api_k");
        assert!(vault.load().unwrap().contains_key("old"));
        assert_eq!(inbox.accounts.len(), 1);

        saved!(inbox, |tokens, shared| tokens.disconnect(&shared, &id)).unwrap();
        assert!(inbox.accounts.is_empty() && !vault.load().unwrap().contains_key(&id));
        assert!(vault.load().unwrap().contains_key("old"));
    }

    /// A refresh started before Erase finishes after it, and used to write the cache back.
    #[test]
    fn a_refresh_that_finishes_after_erase_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let store = JsonStore::new(dir.path().join("Remora"));
        let mut inbox = Inbox::open(store.clone(), Arc::new(MemoryVault::default()), Managed::default());
        *inbox.accounts_mut() = self::inbox().accounts().to_vec();
        let in_flight = snapshot(vec![item("1", InboxBundle::reviews())]);

        saved!(inbox, |tokens, shared| tokens.erase(&shared)).unwrap();
        inbox.apply(in_flight, now());

        assert!(inbox.all_items().is_empty());
        assert_eq!(crate::store::json_files(&dir.path().join("Remora")).map(|files| files.count()).unwrap_or(0), 0);
    }

    #[test]
    fn results_for_a_disconnected_account_are_dropped() {
        let mut inbox = inbox();
        let mut in_flight = snapshot(vec![item("1", InboxBundle::reviews())]);
        in_flight.push(("a1".into(), Err(PluginError::Network("timeout".into()))));

        saved!(inbox, |tokens, shared| tokens.disconnect(&shared, "a1")).unwrap();
        inbox.apply(in_flight, now());

        assert!(inbox.all_items().is_empty());
        assert!(inbox.errors().is_empty());
    }

    /// An account that keeps failing is asked less often; a rate limit holds even for a manual refresh.
    #[test]
    fn failing_and_rate_limited_accounts_sit_refreshes_out() {
        let mut inbox = inbox();
        inbox.accounts_mut()[0].settings.insert("host".into(), "https://github.com".into());
        inbox.provide_secrets(HashMap::from([(
            "a1".to_string(),
            HashMap::from([("token".to_string(), "t".to_string())]),
        )]));
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
        inbox.accounts_mut().push(Account {
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
        assert!(!inbox.sync.slows_down(), "no slow-down for being offline");
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
        inbox.accounts_mut()[0].settings.insert("host".into(), "https://github.com".into());
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
        saved!(inbox, |tokens, shared| tokens.reconnect(&shared, "a1", token, Some(fetched), now())).unwrap();
        assert_eq!(inbox.accounts.len(), 1, "the same account");
        assert_eq!(inbox.health(), SourcesHealth::Fine);
        assert!(inbox.states.get("x").is_some_and(|s| s.pinned), "its states stay");
        assert_eq!(
            inbox.accounts.secrets().unwrap().get("a1").and_then(|s| s.get("token")).map(String::as_str),
            Some("new")
        );
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
        assert!(inbox.snoozes.history().last().is_some_and(|r| r.done_at.is_some()), "Done closes the last snooze");
    }

    /// A reminder isn't a habit: snoozing one teaches nothing.
    #[test]
    fn reminders_are_not_learned_from() {
        let mut inbox = inbox();
        let id = inbox.add_reminder("Call the bank", Utc::now() + Duration::hours(1), None, None, Utc::now()).unwrap();
        inbox.snooze(&id, Utc::now() + Duration::hours(2), SnoozeMode::Hide, None, false);
        assert!(inbox.snoozes.history().is_empty() && inbox.learning.records().is_empty());
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
        assert_eq!(inbox.learning.review_timings().len(), 1);
        assert_eq!(inbox.learning.review_timings()[0].actual, 12);
        assert_eq!(inbox.review_pace(), 1.0, "not before five");
    }

    fn connect_openai(inbox: &Inbox, host: &str) -> Result<Account, Message> {
        let settings = HashMap::from([("host".to_string(), host.to_string()), ("model".to_string(), "m".to_string())]);
        let secrets = HashMap::from([("token".to_string(), "sk".to_string())]);
        inbox.prepare_assistant_connect("openai", None, settings, &secrets, Arc::new(NoNetwork), "en").map(|(a, _)| a)
    }

    /// The assistant's key can be replaced in place: same account, new key, still under the policy.
    #[test]
    fn the_assistants_key_is_updated_in_place() {
        let mut inbox = inbox();
        inbox.preferences.allow_external_ai = true;
        let account = connect_openai(&inbox, "https://api.openai.com/v1").unwrap();
        let old = HashMap::from([("token".into(), "old".into())]);
        saved!(inbox, |tokens, shared| tokens.connect_assistant(&shared, account.clone(), old, None)).unwrap();
        let key = HashMap::from([("token".to_string(), "new".to_string())]);
        assert!(inbox.prepare_assistant_reconnect(&account.id, &key, Arc::new(NoNetwork), "en").is_ok());
        assert!(inbox.prepare_assistant_reconnect("a1", &key, Arc::new(NoNetwork), "en").is_err(), "not an assistant");
        saved!(inbox, |tokens, shared| tokens.reconnect(&shared, &account.id, key.clone(), None, now())).unwrap();
        assert_eq!(inbox.assistant_account().map(|a| a.id), Some(account.id.clone()), "the same account");
        assert_eq!(inbox.accounts.secrets().unwrap()[&account.id]["token"], "new");
        inbox.preferences.allow_external_ai = false;
        assert!(
            inbox.prepare_assistant_reconnect(&account.id, &key, Arc::new(NoNetwork), "en").is_err(),
            "the policy still applies"
        );
    }

    /// The organization's server replaces the user's, everywhere the account is used or shown.
    #[test]
    fn the_organizations_ai_server_is_locked_in() {
        let mut inbox = inbox();
        inbox.managed = Managed {
            allow_external_ai: Some(true),
            ai_server: Some("https://acme.openai.azure.com/openai/v1".into()),
            ..Managed::default()
        };
        let account = connect_openai(&inbox, "https://evil.example/v1").unwrap();
        assert_eq!(account.settings["host"], "https://acme.openai.azure.com/openai/v1");
        inbox
            .accounts_mut()
            .push(Account { settings: HashMap::from([("host".into(), "https://evil.example/v1".into())]), ..account });
        let info = inbox.account_infos().into_iter().find(|a| a.account.plugin_id == "openai").unwrap();
        assert_eq!(info.hosts, ["acme.openai.azure.com"], "the user's old server is never reached");
        assert!(info.allowed && inbox.assistant_account().is_some());

        inbox.managed.allowed_ai_servers = Some(vec!["https://api.openai.com/v1".into()]);
        assert_eq!(
            connect_openai(&inbox, "https://api.openai.com/v1").err().map(|e| e.text).as_deref(),
            Some(SERVER_NOT_ALLOWED)
        );
        let info = inbox.account_infos().into_iter().find(|a| a.account.plugin_id == "openai").unwrap();
        assert_eq!(info.refusal.as_deref(), Some(SERVER_NOT_ALLOWED));
        assert!(inbox.assistant_account().is_none(), "a forced server outside the list turns the assistant off");
    }

    /// A server on this computer works with external AI off, unless the organization says otherwise.
    #[test]
    fn local_ai_servers_follow_the_policy() {
        let mut inbox = inbox();
        assert!(connect_openai(&inbox, "http://localhost:11434/v1").is_ok());
        assert_eq!(
            connect_openai(&inbox, "https://api.openai.com/v1").err().map(|e| e.text).as_deref(),
            Some(EXTERNAL_AI_OFF)
        );
        inbox.managed = Managed { allow_external_ai: Some(false), ..Managed::default() };
        assert_eq!(
            connect_openai(&inbox, "http://localhost:11434/v1").err().map(|e| e.text).as_deref(),
            Some(EXTERNAL_AI_OFF)
        );
        inbox.managed.allow_local_ai = Some(true);
        assert!(connect_openai(&inbox, "http://localhost:11434/v1").is_ok());
        inbox.managed = Managed { allow_external_ai: Some(true), allow_local_ai: Some(false), ..Managed::default() };
        assert_eq!(
            connect_openai(&inbox, "http://127.0.0.1:1234/v1").err().map(|e| e.text).as_deref(),
            Some(LOCAL_AI_NOT_ALLOWED)
        );
        assert!(connect_openai(&inbox, "https://api.openai.com/v1").is_ok());
        assert_eq!(
            connect_openai(&inbox, "http://llm.lan/v1").err().map(|e| e.text).as_deref(),
            Some("Only a server on this computer can use http: use https."),
        );
    }

    /// The organization's Claude Code path wins; one it can't read turns Claude Code off.
    #[test]
    fn the_organizations_claude_code_path() {
        let mut inbox = inbox();
        inbox.preferences.allow_external_ai = true;
        let connect = |inbox: &Inbox| {
            inbox
                .prepare_assistant_connect(
                    "claude-code",
                    None,
                    HashMap::new(),
                    &HashMap::new(),
                    Arc::new(NoNetwork),
                    "en",
                )
                .map(|(account, _)| account)
        };
        inbox.managed.claude_code_path = Some(String::new());
        assert_eq!(connect(&inbox).err().map(|e| e.text).as_deref(), Some(CLAUDE_CODE_PATH_UNREADABLE));
    }

    /// `AllowedAIModels` keeps both of its models within the list; an empty summary model too.
    #[test]
    fn allowed_models_cover_both_openai_models() {
        let manifest = remora_plugins::openai::manifest();
        let defaults: HashMap<String, String> =
            manifest.fields.iter().map(|f| (f.key.clone(), f.default_value.clone())).collect();
        let policy = AssistantPolicy::new(HashSet::new(), Some(vec!["gpt-a".into(), "gpt-b".into()]));
        let set = |model: &str, digest: &str| {
            HashMap::from([("model".to_string(), model.to_string()), ("digestModel".to_string(), digest.to_string())])
        };
        let kept = policy.constrained(&set("gpt-b", "gpt-a"), &defaults);
        assert_eq!((kept["model"].as_str(), kept["digestModel"].as_str()), ("gpt-b", "gpt-a"));
        // A refused brief model becomes the first allowed; the empty summary model follows it.
        let fixed = policy.constrained(&set("gpt-x", ""), &defaults);
        assert_eq!((fixed["model"].as_str(), fixed["digestModel"].as_str()), ("gpt-a", ""));
        // An empty summary model follows an allowed brief model, and stays empty.
        let same = policy.constrained(&set("gpt-b", ""), &defaults);
        assert_eq!((same["model"].as_str(), same["digestModel"].as_str()), ("gpt-b", ""));
    }

    /// Without an allowed assistant, what it wrote goes.
    #[test]
    fn assistant_output_goes_without_an_assistant() {
        let mut inbox = inbox();
        inbox.store_brief(Some(Brief::new("Two reviews wait.", vec![], Utc::now())));
        inbox.assistant.store_triage(vec![TriageSuggestion {
            id: "x".into(),
            action: TriageAction::Done,
            until: None,
            reason: "old".into(),
        }]);
        inbox.enforce_ai_policy();
        assert!(inbox.brief().is_none() && inbox.triage().is_empty());
    }

    /// A policy read again goes through `set_managed`, which applies it at once: the shell has nothing to remember.
    #[test]
    fn a_new_policy_applies_at_once() {
        let mut inbox = inbox();
        inbox.preferences.allow_external_ai = true;
        let account = connect_openai(&inbox, "https://api.openai.com/v1").unwrap();
        inbox.accounts_mut().push(account);
        inbox.store_brief(Some(Brief::new("Two reviews wait.", vec![], Utc::now())));
        inbox.set_managed(Managed::default());
        assert!(inbox.brief().is_some(), "still allowed");
        inbox.set_managed(Managed { allow_external_ai: Some(false), ..Managed::default() });
        assert!(inbox.brief().is_none(), "what the assistant wrote goes with it");
    }

    /// The assistant is asked without the lock; what it answers after Erase, a disconnect or AI being turned
    /// off is dropped, on screen and on disk.
    #[test]
    fn an_assistant_answer_that_outlives_its_account_is_dropped() {
        let dir = tempfile::tempdir().unwrap();
        // Each in its own folder: a dropped inbox may still be writing to its own.
        let connected = |name: &str| {
            let store = JsonStore::new(dir.path().join(name));
            let mut inbox = Inbox::open(store, Arc::new(MemoryVault::default()), Managed::default());
            inbox.preferences.allow_external_ai = true;
            let account = connect_openai(&inbox, "https://api.openai.com/v1").unwrap();
            let key = HashMap::from([("token".into(), "sk".into())]);
            saved!(inbox, |tokens, shared| tokens.connect_assistant(&shared, account.clone(), key, None)).unwrap();
            (inbox, account.id)
        };
        let brief = || Brief::new("Two reviews wait.", vec![], Utc::now());
        let summary = || BundleSummary::new("Two reviews wait.", &[item("r", InboxBundle::reviews())], Utc::now());
        let suggestions =
            || vec![TriageSuggestion { id: "x".into(), action: TriageAction::Done, until: None, reason: "r".into() }];

        let (mut inbox, from) = connected("kept");
        inbox.keep_brief(&from, brief());
        inbox.keep_summary(&from, "b", summary());
        inbox.keep_triage(&from, suggestions());
        assert!(
            inbox.brief().is_some() && inbox.bundle_summaries().len() == 1 && inbox.triage().len() == 1,
            "still wanted"
        );

        let (mut inbox, from) = connected("erased");
        saved!(inbox, |tokens, shared| tokens.erase(&shared)).unwrap();
        inbox.keep_brief(&from, brief());
        inbox.keep_summary(&from, "b", summary());
        inbox.keep_triage(&from, suggestions());
        assert!(inbox.brief().is_none() && inbox.bundle_summaries().is_empty() && inbox.triage().is_empty());
        assert_eq!(
            crate::store::json_files(&dir.path().join("erased")).map(|files| files.count()).unwrap_or(0),
            0,
            "nothing on disk"
        );

        let (mut inbox, from) = connected("disconnected");
        saved!(inbox, |tokens, shared| tokens.disconnect(&shared, &from)).unwrap();
        inbox.keep_summary(&from, "b", summary());
        assert!(inbox.bundle_summaries().is_empty());

        let (mut inbox, from) = connected("ai-off");
        inbox.preferences.allow_external_ai = false;
        inbox.keep_brief(&from, brief());
        assert!(inbox.brief().is_none());
    }

    /// Every request is counted with its tokens and kept on disk, the daily limit refuses the next one, and
    /// Erase forgets it.
    #[test]
    fn the_assistants_usage_is_recorded_limited_and_erased() {
        let dir = tempfile::tempdir().unwrap();
        let open = || {
            Inbox::open(JsonStore::new(dir.path().to_path_buf()), Arc::new(MemoryVault::default()), Managed::default())
        };
        let mut inbox = open();
        inbox.preferences.allow_external_ai = true;
        let account = connect_openai(&inbox, "https://api.openai.com/v1").unwrap();
        let key = HashMap::from([("token".into(), "sk".into())]);
        saved!(inbox, |tokens, shared| tokens.connect_assistant(&shared, account.clone(), key, None)).unwrap();

        assert!(inbox.assistant(Arc::new(NoNetwork), "en").is_some_and(|(_, plugin)| plugin.is_ok()));
        inbox.record_usage(Some(remora_core::TokenUsage { input: 120, output: 30 }), Utc::now());
        inbox.record_usage(None, Utc::now());
        let (today, month) = inbox.assistant_usage(Utc::now());
        assert_eq!((today.requests, today.input_tokens, today.output_tokens), (2, 120, 30));
        assert_eq!(month.tokens(), 150);

        inbox.preferences.assistant_daily_limit = 2;
        let refused = inbox.assistant(Arc::new(NoNetwork), "en").map(|(_, plugin)| plugin.err());
        assert_eq!(refused, Some(Some(super::assistant::LIMIT_REACHED.into())));
        inbox.preferences.assistant_daily_limit = 3;
        assert!(inbox.assistant(Arc::new(NoNetwork), "en").is_some_and(|(_, plugin)| plugin.is_ok()));

        inbox.persistence.store().unwrap().flush();
        assert_eq!(open().assistant_usage(Utc::now()).0.requests, 2, "kept on disk");
        saved!(inbox, |tokens, shared| tokens.erase(&shared)).unwrap();
        assert_eq!(inbox.assistant_usage(Utc::now()).0.requests, 0);
        assert_eq!(open().assistant_usage(Utc::now()).0.requests, 0, "nothing left on disk");
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
        inbox.assistant.store_triage(selected.clone());
        let open = inbox.apply_triage(&selected, Utc::now());
        assert_eq!(open.as_deref(), Some("c"));
        assert_eq!(inbox.states["a"].snooze.as_ref().map(|s| s.until), Some(later));
        assert!(inbox.states["b"].done.is_some());
        assert!(inbox.states.get("c").and_then(|s| s.snooze.as_ref()).is_none(), "back in the inbox");
        assert!(inbox.triage().is_empty());

        // One Undo puts the whole triage back, not just the last Done.
        assert!(inbox.undo());
        assert!(["a", "b", "c"].iter().all(|id| inbox.states[*id].snooze.as_ref().is_some_and(|s| s.until < later)));
        assert!(inbox.states["b"].done.is_none());
    }

    /// Snooze all and the insight cards' Spread and Same time for all can be undone as a whole.
    #[test]
    fn bulk_snoozes_and_moves_are_undone_together() {
        let mut inbox = inbox();
        inbox.apply(snapshot(vec![item("a", InboxBundle::reviews()), item("b", InboxBundle::reviews())]), now());
        let ids = ["a".to_string(), "b".to_string()];
        let soon = now() + Duration::hours(1);
        inbox.snooze_many(&ids, soon, None, false);
        inbox.spread(&ids, now() + Duration::days(2));
        assert!(inbox.states["a"].snooze.as_ref().unwrap().until > soon);
        assert!(inbox.undo());
        assert!(ids.iter().all(|id| inbox.states[id].snooze.as_ref().map(|s| s.until) == Some(soon)));
        inbox.align_returns(&ids);
        assert!(inbox.undo());
        assert!(!inbox.undo(), "one level, as before");
        assert!(inbox.states["a"].snooze.is_some());
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
        mine.participants = vec![remora_core::Person { name: "erin".into(), avatar_url: None, review: None }];
        inbox.apply(snapshot(vec![mine]), Utc::now());
        let draft = inbox.waiting_draft("m", Utc::now(), &|key| key.to_string()).expect("a nudge");
        assert!(draft.contains("@erin") && draft.contains("3 days"), "{draft}");
    }
}

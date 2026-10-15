//! The sync engine's bookkeeping: what each account's last fetch said, which accounts wait before being asked again,
//! and the notifications held for the next set time. Ported from the macOS app's `SyncEngine`.

use super::persistence::{CACHE, STATES};
use super::{FetchJob, FetchResult, Inbox};
use crate::Preferences;
use chrono::{DateTime, Utc};
use remora_core::{
    grouped, is_due, notices, parse_times, waits, while_closed, Backoff, Failure, FailureKind, InboxBundle, InboxItem,
    Message, Notice, NoticeKind, SnoozeMode, SourcesHealth, VerbClassifier,
};
use remora_plugins::{registry, ConditionalHttpClient, GuardedHttpClient, HttpClient, PluginError};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

pub struct SyncEngine {
    /// Each failing account's last failure, by kind.
    errors: HashMap<String, Failure>,
    /// What each account's last fetch said besides its items.
    remarks: HashMap<String, Vec<Message>>,
    /// Accounts that failed lately, and when to ask them again.
    backoff: HashMap<String, Backoff>,
    /// Accounts fetched since launch: the first fetch announces what arrived while the app was closed.
    synced: HashSet<String>,
    /// Notifications waiting for the next set time (IDEA-26), and when the last batch went out.
    waiting: Vec<Notice>,
    batched_at: DateTime<Utc>,
    last_refresh: Option<DateTime<Utc>>,
}

impl Default for SyncEngine {
    fn default() -> Self {
        SyncEngine {
            errors: HashMap::new(),
            remarks: HashMap::new(),
            backoff: HashMap::new(),
            synced: HashSet::new(),
            waiting: vec![],
            batched_at: Utc::now(),
            last_refresh: None,
        }
    }
}

impl SyncEngine {
    // MARK: Reading

    pub fn failure(&self, account_id: &str) -> Option<&Failure> {
        self.errors.get(account_id)
    }

    pub fn errors(&self) -> Vec<Message> {
        self.errors.values().map(|f| f.message.clone()).collect()
    }

    /// What the footer says about the sources.
    pub fn health(&self) -> SourcesHealth {
        SourcesHealth::of(&self.errors)
    }

    pub fn remarks(&self) -> Vec<Message> {
        self.remarks.values().flatten().cloned().collect()
    }

    pub fn remarks_of(&self, account_id: &str) -> Vec<Message> {
        self.remarks.get(account_id).cloned().unwrap_or_default()
    }

    pub fn last_refresh(&self) -> Option<DateTime<Utc>> {
        self.last_refresh
    }

    /// An account that keeps failing, or is rate-limited, sits this refresh out. `manual`: asked for by the user,
    /// which doesn't wait out a slow-down; a rate limit still holds.
    pub fn waits(&self, account_id: &str, now: DateTime<Utc>, manual: bool) -> bool {
        self.backoff.get(account_id).is_some_and(|b| b.waits(now, manual))
    }

    // MARK: Results

    pub fn refreshed(&mut self, now: DateTime<Utc>) {
        self.last_refresh = Some(now);
    }

    /// An account that couldn't even be asked: refused by the policy, or its plugin couldn't be built.
    pub fn refused(&mut self, account_id: &str, message: impl Into<Message>) {
        self.errors.insert(account_id.to_string(), Failure::new(FailureKind::Other, message.into()));
    }

    /// A failed fetch. `offline`: every source failed to connect, which is the network, not the tools: said once,
    /// and no slow-down for it. Otherwise the account is asked less often, and a rate limit is waited out.
    pub fn failed(
        &mut self,
        account_id: String,
        error: PluginError,
        offline: bool,
        now: DateTime<Utc>,
        interval: chrono::Duration,
    ) {
        let kind = if offline { FailureKind::Offline } else { error.kind() };
        if kind != FailureKind::Offline {
            let resume = match error {
                // No reset time given: wait at least one refresh.
                PluginError::RateLimited(at) => {
                    Some(at.and_then(|s| DateTime::from_timestamp(s, 0)).unwrap_or(now + interval))
                }
                _ => None,
            };
            self.backoff.entry(account_id.clone()).or_default().failed(now, interval, resume);
        }
        self.errors.insert(account_id, Failure::new(kind, error.message()));
    }

    /// A fetch that worked: the account's failure and slow-down end, its remarks are the new ones. True when it is
    /// the account's first since launch.
    pub fn succeeded(&mut self, account_id: &str, remarks: Vec<Message>) -> bool {
        self.backoff.remove(account_id);
        self.errors.remove(account_id);
        if remarks.is_empty() {
            self.remarks.remove(account_id);
        } else {
            self.remarks.insert(account_id.to_string(), remarks);
        }
        self.synced.insert(account_id.to_string())
    }

    /// Asked again at once: a new token, or the same account connected again.
    pub fn retry(&mut self, account_id: &str) {
        self.backoff.remove(account_id);
    }

    /// A disconnected account: nothing it said stays.
    pub fn forget(&mut self, account_id: &str) {
        self.errors.remove(account_id);
        self.remarks.remove(account_id);
        self.synced.remove(account_id);
    }

    pub fn forget_remarks(&mut self) {
        self.remarks.clear();
    }

    /// The demo's accounts, as if they had synced.
    pub(super) fn synced_all(&mut self, account_ids: impl IntoIterator<Item = String>, now: DateTime<Utc>) {
        self.synced = account_ids.into_iter().collect();
        self.last_refresh = Some(now);
    }

    // MARK: Notifications

    /// With notifications grouped at set times, what arrived or changed waits for `release`; the rest goes now.
    pub fn hold(&mut self, notices: Vec<Notice>, preferences: &Preferences) -> Vec<Notice> {
        if !preferences.batch_notifications {
            return notices;
        }
        let (waiting, now): (Vec<Notice>, Vec<Notice>) = notices.into_iter().partition(waits);
        self.waiting.extend(waiting);
        now
    }

    /// What waited, at the first set time since the last batch, or at once when grouping was turned off.
    pub fn release(&mut self, now: DateTime<Utc>, preferences: &Preferences) -> Vec<Notice> {
        let due = !preferences.batch_notifications
            || is_due(&parse_times(&preferences.notification_times), self.batched_at, now);
        if !due {
            return vec![];
        }
        self.batched_at = now;
        grouped(std::mem::take(&mut self.waiting))
    }

    #[cfg(test)]
    pub(crate) fn batched(&mut self, at: DateTime<Utc>) {
        self.batched_at = at;
    }

    #[cfg(test)]
    pub(crate) fn slows_down(&self) -> bool {
        !self.backoff.is_empty()
    }
}

// MARK: The inbox's refresh

impl Inbox {
    /// One plugin per allowed account, each with an HTTP client limited to its declared hosts.
    /// Refused accounts get an error instead.
    /// `manual`: asked for by the user, which doesn't wait out a slow-down (`Backoff`). An account that keeps failing,
    /// or is rate-limited, sits this refresh out and keeps its items and its error.
    pub fn fetch_jobs(&mut self, http: Arc<dyn HttpClient>, now: DateTime<Utc>, manual: bool) -> Vec<FetchJob> {
        let secrets = match self.accounts.secrets() {
            Ok(secrets) => secrets.clone(),
            Err(error) => {
                for account in self.accounts.iter() {
                    self.sync.refused(&account.id, error.clone());
                }
                return vec![];
            }
        };
        let policy = self.policy();
        let mut jobs = vec![];
        // A copy: each account's session is updated on the way.
        let accounts = self.accounts.to_vec();
        for account in &accounts {
            let account = self.with_policy_settings(account);
            if self.sync.waits(&account.id, now, manual) {
                continue;
            }
            let Some(manifest) = registry::manifest(&account.plugin_id) else { continue };
            if let Some(refusal) = policy.refusal(&manifest) {
                self.sync.refused(&account.id, refusal);
                continue;
            }
            let account_secrets = secrets.get(&account.id).cloned().unwrap_or_default();
            let (cache, etags) = self.accounts.session(&account.id, &account.settings, &account_secrets);
            let conditional: Arc<dyn HttpClient> = Arc::new(ConditionalHttpClient::new(http.clone(), etags));
            let guarded: Arc<dyn HttpClient> =
                Arc::new(GuardedHttpClient::new(conditional, registry::allowed_hosts(&account, &manifest)));
            match registry::make(&account, &account_secrets, guarded, cache) {
                Ok(plugin) => jobs.push(FetchJob { account_id: account.id.clone(), plugin }),
                Err(error) => self.sync.refused(&account.id, error.message()),
            }
        }
        jobs
    }

    /// Stores what came back and says what deserves a notification. The first sync of an account is
    /// silent; failed accounts keep their last items.
    pub fn apply(&mut self, results: Vec<FetchResult>, now: DateTime<Utc>) -> Vec<Notice> {
        // Fetches run without the lock: an account disconnected, or everything erased, while they ran must not come
        // back. A reconnected account has a new id, so its old results are dropped too.
        let results: Vec<FetchResult> = results.into_iter().filter(|(id, _)| self.accounts.contains(id)).collect();
        self.sync.refreshed(now);
        if results.is_empty() {
            // Nothing to store: in particular, nothing written back after Erase.
            return vec![];
        }
        let mut out = vec![];
        let interval = chrono::Duration::minutes(i64::from(self.effective_preferences().refresh_minutes.max(1)));
        // Every source failed to connect: that's the network, not the tools. Said once, and no slow-down for it. With a
        // single source, an unreachable server and no network look the same: it says it can't reach the server.
        let offline = results.len() > 1
            && results.iter().all(|(_, result)| matches!(result, Err(e) if e.kind() == FailureKind::Unreachable));
        for (account_id, result) in results {
            match result {
                Err(error) => self.sync.failed(account_id, error, offline, now, interval),
                Ok(snapshot) => {
                    let first = self.sync.succeeded(&account_id, snapshot.remarks.clone());
                    let items: Vec<InboxItem> = snapshot.items.into_iter().map(VerbClassifier::classify).collect();
                    // The first sync after launch compares with the saved copy, so what arrived while the app
                    // was closed is announced; an account with no saved copy (just connected) stays silent.
                    let previous = if first {
                        self.items.get(&account_id).cloned()
                    } else {
                        Some(self.items.get(&account_id).cloned().unwrap_or_default())
                    };
                    let account = self.accounts.find(&account_id);
                    let plugin_id = account.map(|a| a.plugin_id.clone());
                    let wanted: Vec<Notice> = notices(previous.as_deref(), &items)
                        .into_iter()
                        .filter(|notice| {
                            if notice.kind == NoticeKind::Arrival {
                                self.preferences.notify_arrivals
                            } else {
                                self.preferences.notify_status_changes
                            }
                        })
                        .map(|notice| self.respecting_privacy(notice, plugin_id.as_deref()))
                        .collect();
                    if first {
                        let name = account
                            .and_then(|a| a.name.clone().or_else(|| registry::manifest(&a.plugin_id).map(|m| m.name)))
                            .unwrap_or_default();
                        out.extend(while_closed(&account_id, &name, wanted));
                    } else {
                        out.extend(wanted);
                    }
                    self.wake_touched(&items);
                    self.accounts.set_identity(&account_id, snapshot.identity);
                    self.items.insert(account_id, items);
                }
            }
        }
        self.prune();
        self.recompute_links();
        self.persistence.save(CACHE, &self.items);
        self.persistence.save(STATES, &self.states);
        let preferences = self.effective_preferences();
        self.sync.hold(out, &preferences)
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
        let preferences = self.effective_preferences();
        let mut out = vec![];
        for (id, state) in self.states.iter_mut() {
            let Some(snooze) = state.snooze.clone().filter(|s| s.until <= now) else { continue };
            state.snooze = None;
            state.reminded_at = Some(now);
            state.done = None;
            if let Some(item) = items.iter().find(|i| &i.id == id) {
                let reminder = item.bundle == InboxBundle::reminders();
                let hidden = preferences.hides_content(&item.plugin_id);
                let notice = Notice {
                    kind: NoticeKind::Reminder,
                    item_id: id.clone(),
                    title: if reminder { "Reminder" } else { "Back in your inbox" }.into(),
                    // A reminder's context is "Reminder" again: the title says it already.
                    subtitle: if reminder { String::new() } else { item.context.clone() },
                    body: [Some(item.title.clone()), snooze.note].into_iter().flatten().collect::<Vec<_>>().join("\n"),
                    url: item.url.clone(),
                    count: None,
                };
                out.push(if hidden { notice.redacted() } else { notice });
            }
        }
        if !out.is_empty() {
            self.persistence.save(STATES, &self.states);
        }
        out.extend(self.sync.release(now, &preferences));
        out
    }

    /// Hides what was written when the user doesn't want that integration's content on screen.
    fn respecting_privacy(&self, notice: Notice, plugin_id: Option<&str>) -> Notice {
        match plugin_id {
            Some(id) if self.effective_preferences().hides_content(id) => notice.redacted(),
            _ => notice,
        }
    }

    /// Forgets states of items that no longer exist, except snoozes (their item may come back).
    pub(super) fn prune(&mut self) {
        let known: HashSet<String> = self.all_items().into_iter().map(|i| i.id).collect();
        self.states.retain(|id, state| known.contains(id) || state.snooze.is_some());
    }
}

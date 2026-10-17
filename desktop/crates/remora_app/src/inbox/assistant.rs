//! The assistant coordinator: what the assistant wrote (the brief, each group's summary, the triage of the snoozed
//! pile), whether it is still fresh, and its files. The inbox decides who may be asked and whether an answer may
//! still be kept (`Inbox::still_answers`). Ported from the macOS app's `AssistantCoordinator`.
use remora_core::badge;

use super::persistence::{Persistence, BRIEF, SUMMARIES, USAGE};
use super::{AssistantJob, Inbox};
use chrono::{DateTime, Local, NaiveDate, Utc};
use remora_core::{
    Account, AssistantPolicy, AssistantUsage, Brief, BundleSummary, InboxItem, Message, SnoozedItem, TokenUsage,
    TriageAction, TriageSuggestion, UsageDay,
};
use remora_plugins::{claude, registry, AssistantPlugin, GuardedHttpClient, HttpClient};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

pub struct AssistantCoordinator {
    brief: Option<Brief>,
    summaries: HashMap<String, BundleSummary>,
    /// Claude's proposal for the snoozed pile, until applied or discarded.
    triage: Vec<TriageSuggestion>,
    /// What the assistant used, per day (AI-03).
    usage: AssistantUsage,
    persistence: Persistence,
}

/// Said when today's limit is reached. Same text on macOS.
pub const LIMIT_REACHED: &str =
    "Today’s assistant limit is reached. It starts again tomorrow, or raise it in Settings.";

impl AssistantCoordinator {
    pub(super) fn new(
        brief: Option<Brief>,
        summaries: HashMap<String, BundleSummary>,
        usage: AssistantUsage,
        persistence: Persistence,
    ) -> Self {
        AssistantCoordinator { brief, summaries, triage: vec![], usage, persistence }
    }

    /// One request sent, with the tokens its answers reported.
    pub fn record(&mut self, tokens: Option<TokenUsage>, today: NaiveDate) {
        self.usage.record(tokens, today);
        self.persistence.save(USAGE, &self.usage);
    }

    pub fn brief(&self) -> Option<&Brief> {
        self.brief.as_ref()
    }

    pub fn summaries(&self) -> &HashMap<String, BundleSummary> {
        &self.summaries
    }

    pub fn triage(&self) -> &[TriageSuggestion] {
        &self.triage
    }

    /// Whether the brief was written from these items, recently enough.
    pub fn brief_is_fresh(&self, items: &[InboxItem], cache_minutes: i64, now: DateTime<Utc>) -> bool {
        self.brief.as_ref().is_some_and(|b| b.is_fresh(items, cache_minutes, now))
    }

    /// Whether this group's summary was written from these items, recently enough.
    pub fn summary_is_fresh(
        &self,
        bundle_id: &str,
        items: &[InboxItem],
        cache_minutes: i64,
        now: DateTime<Utc>,
    ) -> bool {
        self.summaries.get(bundle_id).is_some_and(|s| s.is_fresh(items, cache_minutes, now))
    }

    pub fn store_brief(&mut self, brief: Option<Brief>) {
        self.brief = brief;
        self.persistence.save(BRIEF, &self.brief);
    }

    pub fn store_summary(&mut self, bundle_id: &str, summary: BundleSummary) {
        self.summaries.insert(bundle_id.to_string(), summary);
        self.persistence.save(SUMMARIES, &self.summaries);
    }

    pub fn store_triage(&mut self, suggestions: Vec<TriageSuggestion>) {
        self.triage = suggestions;
    }

    /// The suggestions applied leave the proposal; the others stay to be ticked.
    pub fn applied(&mut self, ids: &HashSet<&str>) {
        self.triage.retain(|s| !ids.contains(s.id.as_str()));
    }

    pub fn discard_triage(&mut self) {
        self.triage.clear();
    }

    /// Without an allowed assistant, nothing it wrote is kept or shown, on screen or on disk.
    pub fn forget(&mut self) {
        if self.brief.is_some() || !self.summaries.is_empty() || !self.triage.is_empty() {
            self.store_brief(None);
            self.summaries.clear();
            self.persistence.save(SUMMARIES, &self.summaries);
            self.triage.clear();
        }
    }
}

/// The day on this computer's clock: a new one starts at local midnight.
fn local_day(now: DateTime<Utc>) -> NaiveDate {
    now.with_timezone(&Local).date_naive()
}

// MARK: The inbox's assistant

impl Inbox {
    pub fn brief(&self) -> Option<&Brief> {
        self.assistant.brief()
    }

    pub fn bundle_summaries(&self) -> &HashMap<String, BundleSummary> {
        self.assistant.summaries()
    }

    /// Claude's proposal for the snoozed pile, until applied or discarded.
    pub fn triage(&self) -> &[TriageSuggestion] {
        self.assistant.triage()
    }

    /// The connected assistant, only when the privacy policy allows it.
    pub fn assistant_account(&self) -> Option<Account> {
        let privacy = self.privacy();
        let policy = privacy.compliance();
        self.accounts
            .iter()
            .find(|a| {
                let a = privacy.with_policy_settings(a);
                claude::assistant_manifests()
                    .iter()
                    .any(|m| m.id == a.plugin_id && policy.account_refusal(m, &a.settings).is_none())
            })
            .cloned()
    }

    /// What the assistant may read and use: everything but the sources you or your organization keep from it, with the
    /// organization's models.
    pub fn assistant_policy(&self) -> AssistantPolicy {
        self.privacy().assistant_policy()
    }

    /// The assistant for an account, through a client limited to its declared hosts.
    fn assistant_for(
        &mut self,
        account: &Account,
        http: Arc<dyn HttpClient>,
        language: &str,
    ) -> Result<Box<dyn AssistantPlugin>, Message> {
        let secrets = self.accounts.secrets()?.get(&account.id).cloned().unwrap_or_default();
        self.assistant_with(account, &secrets, http, language)
    }

    /// The assistant for an account with these secrets: allowed by the policy, limited to its declared hosts,
    /// its models kept within the allowed ones.
    pub(super) fn assistant_with(
        &self,
        account: &Account,
        secrets: &HashMap<String, String>,
        http: Arc<dyn HttpClient>,
        language: &str,
    ) -> Result<Box<dyn AssistantPlugin>, Message> {
        let manifest = claude::assistant_manifests()
            .into_iter()
            .find(|m| m.id == account.plugin_id)
            .ok_or("Unknown assistant.")?;
        let privacy = self.privacy();
        let account = &privacy.with_policy_settings(account);
        if let Some(refusal) = privacy.compliance().account_refusal(&manifest, &account.settings) {
            return Err(refusal.into());
        }
        let guarded: Arc<dyn HttpClient> =
            Arc::new(GuardedHttpClient::new(http, registry::allowed_hosts(account, &manifest)));
        let constrained = privacy.constrained(account, &manifest)?;
        claude::make_assistant(&constrained, secrets, guarded, language).map_err(|e| e.message())
    }

    /// The assistant to ask now, with its account's id for `keep_*`, or None when there is none or it isn't allowed.
    /// Past today's limit, it refuses (AI-03). The request is then measured (`AssistantMeter`) and `record_usage`d.
    pub fn assistant(&mut self, http: Arc<dyn HttpClient>, language: &str) -> Option<AssistantJob> {
        let account = self.assistant_account()?;
        let limit = self.effective_preferences().assistant_daily_limit;
        if !self.assistant.usage.allows(limit, local_day(Utc::now())) {
            return Some((account.id, Err(LIMIT_REACHED.into())));
        }
        let plugin = self.assistant_for(&account, http, language);
        Some((account.id, plugin))
    }

    /// One request sent to the assistant, answered or not, with the tokens it reported.
    pub fn record_usage(&mut self, tokens: Option<TokenUsage>, now: DateTime<Utc>) {
        self.assistant.record(tokens, local_day(now));
    }

    /// What the assistant used today and this month, for Settings.
    pub fn assistant_usage(&self, now: DateTime<Utc>) -> (UsageDay, UsageDay) {
        let today = local_day(now);
        (self.assistant.usage.today(today), self.assistant.usage.this_month(today))
    }

    /// Whether an answer from this assistant account may still be kept. It is asked without the lock and can take a
    /// minute or more: an answer that comes back after Erase, a disconnect or a policy that turns AI off is dropped.
    fn still_answers(&self, account_id: &str) -> bool {
        self.assistant_account().is_some_and(|a| a.id == account_id)
    }

    /// Keeps a brief, unless its assistant is gone or no longer allowed.
    pub fn keep_brief(&mut self, from: &str, brief: Brief) {
        if self.still_answers(from) {
            self.assistant.store_brief(Some(brief));
        }
    }

    /// What a brief is written from: the task in progress (marked so), the pinned items and My turn, as the
    /// assistant may see them (AI-20).
    pub fn brief_items(&self, now: DateTime<Utc>) -> Vec<InboxItem> {
        let layout = self.layout(now, "");
        let started = layout.in_progress.into_iter().map(|mut item| {
            item.badges.push(remora_core::Badge::new(badge::IN_PROGRESS, "In progress", remora_core::Tone::Accent));
            item
        });
        let items: Vec<InboxItem> =
            started.chain(layout.pinned).chain(layout.my_turn.into_iter().flat_map(|g| g.items)).collect();
        self.assistant_policy().items(&items)
    }

    pub fn brief_is_fresh(&self, now: DateTime<Utc>) -> bool {
        self.assistant.brief_is_fresh(&self.brief_items(now), self.preferences.brief_cache_minutes, now)
    }

    /// The brief the assistant wrote as its connection test (`Tokens::connect_assistant`).
    pub(crate) fn store_brief(&mut self, brief: Option<Brief>) {
        self.assistant.store_brief(brief);
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
        let fresh = self.assistant.summary_is_fresh(bundle_id, &items, self.preferences.brief_cache_minutes, now);
        (items, fresh)
    }

    /// Keeps a group's summary, unless its assistant is gone or no longer allowed.
    pub fn keep_summary(&mut self, from: &str, bundle_id: &str, summary: BundleSummary) {
        if self.still_answers(from) {
            self.assistant.store_summary(bundle_id, summary);
        }
    }

    /// Keeps triage suggestions, unless their assistant is gone or no longer allowed.
    pub fn keep_triage(&mut self, from: &str, suggestions: Vec<TriageSuggestion>) {
        if self.still_answers(from) {
            self.assistant.store_triage(suggestions);
        }
    }

    pub fn discard_triage(&mut self) {
        self.assistant.discard_triage();
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
    /// The ticked suggestions, undone together by one Undo (each Done would otherwise replace it).
    pub fn apply_triage(&mut self, selected: &[TriageSuggestion], now: DateTime<Utc>) -> Option<String> {
        let changes = selected.iter().any(|s| s.action != TriageAction::Keep);
        if changes {
            self.remember();
        }
        let undo = self.undo_point.clone();
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
                TriageAction::Done => {
                    self.toggle_done(&suggestion.id, now);
                }
                TriageAction::Now => {
                    self.unsnooze(&suggestion.id);
                    open.get_or_insert_with(|| suggestion.id.clone());
                }
            }
        }
        if changes {
            self.undo_point = undo;
        }
        let applied: HashSet<&str> = selected.iter().map(|s| s.id.as_str()).collect();
        self.assistant.applied(&applied);
        open
    }

    /// Without an allowed assistant, nothing it wrote is kept or shown. `set_preferences`, `set_managed` and a
    /// disconnect run it: no caller has to.
    pub(super) fn enforce_ai_policy(&mut self) {
        if self.assistant_account().is_none() {
            self.assistant.forget();
        }
    }
}

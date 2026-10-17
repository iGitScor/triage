//! The snooze service: every snooze with its reason, and what the advisor learns from them (when to come back, the
//! usual reason, patterns in the pile). The history is saved here on every change. Ported from the macOS app's
//! `SnoozeService`.

use super::persistence::{Persistence, DISMISSALS, HISTORY};
use super::Inbox;
use chrono::{DateTime, Utc};
use remora_core::{InboxItem, InsightDismissal, ItemState, SnoozeAdvisor, SnoozeInsight, SnoozeReason, SnoozeRecord};
use std::collections::HashMap;

/// The history kept: the last 500 snoozes.
const KEPT: usize = 500;

pub struct SnoozeService {
    /// Every snooze with its reason and, once done, when: the advisor's input.
    history: Vec<SnoozeRecord>,
    /// Insights the user set aside ("Not now"), saved, each for `InsightDismissal::DAYS`.
    dismissals: Vec<InsightDismissal>,
    persistence: Persistence,
}

impl SnoozeService {
    pub(super) fn new(history: Vec<SnoozeRecord>, dismissals: Vec<InsightDismissal>, persistence: Persistence) -> Self {
        SnoozeService { history, dismissals, persistence }
    }

    pub fn history(&self) -> &[SnoozeRecord] {
        &self.history
    }

    /// A snooze of an item (never a reminder: it isn't a habit).
    pub fn record(&mut self, item: &InboxItem, reason: Option<SnoozeReason>, now: DateTime<Utc>, until: DateTime<Utc>) {
        self.history.push(SnoozeRecord::new(item, reason, now, until));
        let excess = self.history.len().saturating_sub(KEPT);
        self.history.drain(..excess);
        self.save();
    }

    /// The snooze that ended in Done: when you finally do things teaches your best hour.
    pub fn done(&mut self, item_id: &str, now: DateTime<Utc>) {
        if let Some(record) = self.history.iter_mut().rev().find(|r| r.item_id == item_id && r.done_at.is_none()) {
            record.done_at = Some(now);
            self.save();
        }
    }

    /// Snoozes of this item in the last 30 days.
    pub fn count(&self, item_id: &str, now: DateTime<Utc>) -> usize {
        let since = now - chrono::Duration::days(30);
        self.history.iter().filter(|r| r.item_id == item_id && r.at > since).count()
    }

    /// When to come back for this reason, from your habits. Weekends are skipped.
    pub fn suggested_return(&self, reason: SnoozeReason, now: DateTime<Utc>) -> DateTime<Utc> {
        SnoozeAdvisor::new().suggested_return(reason, &self.history, now)
    }

    /// The reason usually given in this item's repo or channel, preselected in the snooze sheet.
    pub fn usual_reason(&self, item: &InboxItem) -> Option<SnoozeReason> {
        SnoozeAdvisor::new().usual_reason(item, &self.history)
    }

    /// Patterns in the snoozed pile, without the ones set aside.
    pub fn insights(
        &self,
        snoozed: &[InboxItem],
        states: &HashMap<String, ItemState>,
        now: DateTime<Utc>,
    ) -> Vec<SnoozeInsight> {
        SnoozeAdvisor::new()
            .insights(snoozed, states, &self.history, now)
            .into_iter()
            .filter(|insight| !self.dismissals.iter().any(|d| d.covers(insight, now)))
            .collect()
    }

    /// Set aside until `InsightDismissal::DAYS` from now, across launches; the expired ones go.
    pub fn dismiss(&mut self, insight: &SnoozeInsight, now: DateTime<Utc>) {
        self.dismissals.retain(|d| d.until > now);
        self.dismissals.push(InsightDismissal::new(insight, now));
        self.persistence.save(DISMISSALS, &self.dismissals);
    }

    fn save(&self) {
        self.persistence.save(HISTORY, &self.history);
    }
}

// MARK: The inbox's smart snooze

impl Inbox {
    /// Snoozes of this item in the last 30 days.
    pub fn snooze_count(&self, id: &str, now: DateTime<Utc>) -> usize {
        self.snoozes.count(id, now)
    }

    /// The reason usually given in this item's repo or channel, preselected in the snooze sheet.
    pub fn usual_reason(&self, id: &str) -> Option<SnoozeReason> {
        self.snoozes.usual_reason(&self.item(id)?)
    }

    /// When to come back for this reason, from your habits. Weekends are skipped.
    pub fn suggested_return(&self, reason: SnoozeReason, now: DateTime<Utc>) -> DateTime<Utc> {
        self.snoozes.suggested_return(reason, now)
    }

    /// Patterns in the snoozed pile, without the ones set aside.
    pub fn insights(&self, now: DateTime<Utc>) -> Vec<SnoozeInsight> {
        self.snoozes.insights(&self.layout(now, "").snoozed, &self.states, now)
    }

    pub fn dismiss_insight(&mut self, id: &str, now: DateTime<Utc>) {
        if let Some(insight) = self.insights(now).into_iter().find(|i| i.id() == id) {
            self.snoozes.dismiss(&insight, now);
        }
    }

    /// A pile-up, 30 minutes apart from its slot.
    pub fn spread(&mut self, ids: &[String], from: DateTime<Utc>) {
        self.remember();
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
        self.remember();
        for id in ids {
            self.reschedule(id, earliest);
        }
    }
}

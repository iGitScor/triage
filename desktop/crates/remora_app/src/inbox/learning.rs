//! The learning service: what Remora learns from your actions on this computer, and only here. The ranker (what you
//! handle quickly or put off) and how long your timed reviews really took. Both files are saved here on every change.
//! Ported from the macOS app's `LearningService`.

use super::persistence::{Persistence, LEARNING, REVIEW_TIMES};
use super::Inbox;
use chrono::{DateTime, Utc};
use remora_core::{
    ActionOutcome, ActionRecord, InboxBundle, InboxItem, PersonalRanker, ReviewPace, ReviewPrep, ReviewTiming,
};

pub struct LearningService {
    records: Vec<ActionRecord>,
    /// Silent until it has enough of your actions.
    ranker: PersonalRanker,
    /// Reviews you timed (Start, then Done): how long they really took.
    review_timings: Vec<ReviewTiming>,
    persistence: Persistence,
}

impl LearningService {
    pub(super) fn new(records: Vec<ActionRecord>, review_timings: Vec<ReviewTiming>, persistence: Persistence) -> Self {
        LearningService { ranker: PersonalRanker::new(&records), records, review_timings, persistence }
    }

    /// Your habits, to break ties inside each group.
    pub fn score(&self, item: &InboxItem) -> f64 {
        self.ranker.score(item)
    }

    /// Why an item ranks higher than its priority alone would put it.
    pub fn reason(&self, item: &InboxItem, tr: &dyn Fn(&str) -> String) -> Option<String> {
        self.ranker.reason(item, tr)
    }

    /// How your reviews compare with the estimate; 1 until five were timed.
    pub fn review_pace(&self) -> f64 {
        ReviewPace::factor(&self.review_timings)
    }

    /// Opened or finished within a day of its last activity: something you handle quickly.
    pub fn handled(&mut self, item: &InboxItem, now: DateTime<Utc>) {
        if item.bundle != InboxBundle::reminders() && now - item.date < chrono::Duration::days(1) {
            self.learn(ActionRecord::new(item, ActionOutcome::Quick, now));
        }
    }

    /// Snoozed: something you put off. Never a reminder: it isn't a habit.
    pub fn deferred(&mut self, item: &InboxItem, now: DateTime<Utc>) {
        self.learn(ActionRecord::new(item, ActionOutcome::Deferred, now));
    }

    fn learn(&mut self, record: ActionRecord) {
        self.records.push(record);
        let excess = self.records.len().saturating_sub(1_000);
        self.records.drain(..excess);
        self.ranker = PersonalRanker::new(&self.records);
        self.persistence.save(LEARNING, &self.records);
    }

    /// A review started then done: its real duration against the plain estimate, the last 50 kept.
    pub fn reviewed(&mut self, item: &InboxItem, started: DateTime<Utc>, now: DateTime<Utc>) {
        let Some(prep) = ReviewPrep::new(item, 1.0) else { return };
        self.review_timings
            .push(ReviewTiming { estimated: i64::from(prep.estimated_minutes), actual: (now - started).num_minutes() });
        let excess = self.review_timings.len().saturating_sub(50);
        self.review_timings.drain(..excess);
        self.persistence.save(REVIEW_TIMES, &self.review_timings);
    }

    #[cfg(test)]
    pub(crate) fn records(&self) -> &[ActionRecord] {
        &self.records
    }

    #[cfg(test)]
    pub(crate) fn review_timings(&self) -> &[ReviewTiming] {
        &self.review_timings
    }
}

// MARK: The inbox's help on a row

impl Inbox {
    /// How your reviews compare with the estimate; 1 until five were timed.
    pub fn review_pace(&self) -> f64 {
        self.learning.review_pace()
    }

    pub fn review_prep(&self, item: &InboxItem) -> Option<ReviewPrep> {
        (item.bundle == InboxBundle::reviews()).then(|| ReviewPrep::new(item, self.review_pace())).flatten()
    }

    /// Why an item ranks higher than its priority alone would put it.
    pub fn ranking_reason(&self, item: &InboxItem, tr: &dyn Fn(&str) -> String) -> Option<String> {
        self.learning.reason(item, tr)
    }
}

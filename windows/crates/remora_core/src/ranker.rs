//! A small Naive Bayes model over your own actions: how likely you are to handle an item quickly.
//! Transparent (each score has a reason), trained instantly, and silent until it has enough data.
//! Port of the macOS `PersonalRanker.swift`.
use crate::link_finder::{normalized, title_words};
use crate::review_prep::fill;
use crate::snooze_advisor::context_key;
use crate::InboxItem;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ActionOutcome {
    /// Opened or finished within a day of appearing.
    Quick,
    /// Snoozed.
    Deferred,
}

/// What you did with an item, kept on this machine to learn your habits.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionRecord {
    pub features: Vec<String>,
    pub outcome: ActionOutcome,
    pub at: DateTime<Utc>,
}

impl ActionRecord {
    pub fn new(item: &InboxItem, outcome: ActionOutcome, at: DateTime<Utc>) -> Self {
        ActionRecord { features: PersonalRanker::features(item), outcome, at }
    }
}

#[derive(Clone, Debug, Default)]
pub struct PersonalRanker {
    counts: HashMap<ActionOutcome, HashMap<String, f64>>,
    totals: HashMap<ActionOutcome, f64>,
    vocabulary: HashSet<String>,
    is_trained: bool,
}

impl PersonalRanker {
    pub const MINIMUM_RECORDS: usize = 20;

    pub fn new(records: &[ActionRecord]) -> Self {
        let outcomes: HashSet<ActionOutcome> = records.iter().map(|r| r.outcome).collect();
        let mut ranker = PersonalRanker {
            is_trained: records.len() >= Self::MINIMUM_RECORDS && outcomes.len() == 2,
            ..Default::default()
        };
        for record in records {
            *ranker.totals.entry(record.outcome).or_default() += 1.0;
            for feature in &record.features {
                *ranker.counts.entry(record.outcome).or_default().entry(feature.clone()).or_default() += 1.0;
                ranker.vocabulary.insert(feature.clone());
            }
        }
        ranker
    }

    pub fn is_trained(&self) -> bool {
        self.is_trained
    }

    /// Log-odds of handling the item quickly; 0 when untrained.
    pub fn score(&self, item: &InboxItem) -> f64 {
        if !self.is_trained {
            return 0.0;
        }
        Self::features(item).iter().fold(self.prior(), |sum, f| sum + self.contribution(f))
    }

    /// The feature that pushes the item up the most, when it clearly does. `tr` maps the English key (the Mac's) to
    /// the UI language's format string; the placeholder is filled here.
    pub fn reason(&self, item: &InboxItem, tr: &dyn Fn(&str) -> String) -> Option<String> {
        if !self.is_trained || self.score(item) <= 1.0 {
            return None;
        }
        // The first of equal weights wins, as Swift's `max(by:)`.
        let mut strongest: Option<(String, f64)> = None;
        for feature in Self::features(item).into_iter().filter(|f| !f.starts_with("word:")) {
            let weight = self.contribution(&feature);
            if strongest.as_ref().is_none_or(|(_, best)| *best < weight) {
                strongest = Some((feature, weight));
            }
        }
        let (feature, weight) = strongest?;
        (weight > 0.5).then(|| describe(&feature, tr))
    }

    fn total(&self, outcome: ActionOutcome) -> f64 {
        self.totals.get(&outcome).copied().unwrap_or(0.0)
    }

    fn count(&self, outcome: ActionOutcome, feature: &str) -> f64 {
        self.counts.get(&outcome).and_then(|c| c.get(feature)).copied().unwrap_or(0.0)
    }

    fn prior(&self) -> f64 {
        (self.total(ActionOutcome::Quick) + 1.0).ln() - (self.total(ActionOutcome::Deferred) + 1.0).ln()
    }

    /// Laplace-smoothed log-likelihood ratio of one feature.
    fn contribution(&self, feature: &str) -> f64 {
        if !self.vocabulary.contains(feature) {
            return 0.0;
        }
        let size = self.vocabulary.len() as f64;
        let quick = (self.count(ActionOutcome::Quick, feature) + 1.0) / (self.total(ActionOutcome::Quick) + size);
        let deferred =
            (self.count(ActionOutcome::Deferred, feature) + 1.0) / (self.total(ActionOutcome::Deferred) + size);
        (quick / deferred).ln()
    }

    fn features(item: &InboxItem) -> Vec<String> {
        let mut features =
            vec![format!("tool:{}", item.plugin_id), format!("verb:{}", item.bundle.id), format!("place:{}", context_key(&item.context))];
        if let Some(author) = &item.author {
            features.push(format!("from:{}", author.name));
        }
        features.extend(item.badges.iter().map(|b| format!("badge:{}", b.id)));
        features.extend(title_words(&normalized(&item.title)).into_iter().take(6).map(|w| format!("word:{w}")));
        features
    }
}

fn describe(feature: &str, tr: &dyn Fn(&str) -> String) -> String {
    let (kind, value) = feature.split_once(':').unwrap_or((feature, ""));
    match kind {
        "from" => fill(&tr("You usually handle items from %@ quickly."), &[value.to_string()]),
        "place" => fill(&tr("You usually handle %@ quickly."), &[value.to_string()]),
        "tool" => fill(&tr("You usually handle %@ items quickly."), &[capitalized(value)]),
        _ => tr("You usually handle items like this quickly."),
    }
}

/// Swift's `capitalized`: each word's first letter in upper case, the rest in lower case.
fn capitalized(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut start = true;
    for c in text.chars() {
        if start {
            out.extend(c.to_uppercase());
        } else {
            out.extend(c.to_lowercase());
        }
        start = c.is_whitespace();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::*;
    use crate::{InboxBundle, Person};

    fn english(key: &str) -> String {
        key.to_string()
    }

    fn review(id: &str, author: &str) -> InboxItem {
        let mut item = item(id, InboxBundle::reviews());
        item.author = Some(Person::named(author));
        item.context = "acme/app #1".into();
        item
    }

    /// Erin's reviews get done quickly, Frank's get snoozed.
    fn records() -> Vec<ActionRecord> {
        let quick = (0..12).map(|i| ActionRecord::new(&review(&format!("l{i}"), "erin"), ActionOutcome::Quick, now()));
        let deferred =
            (0..12).map(|i| ActionRecord::new(&review(&format!("t{i}"), "frank"), ActionOutcome::Deferred, now()));
        quick.chain(deferred).collect()
    }

    #[test]
    fn silent_until_enough_data() {
        let ranker = PersonalRanker::new(&records()[..10]);
        assert!(!ranker.is_trained());
        assert_eq!(ranker.score(&review("x", "erin")), 0.0);
    }

    #[test]
    fn learns_who_you_handle_quickly() {
        let ranker = PersonalRanker::new(&records());
        assert!(ranker.is_trained());
        assert!(ranker.score(&review("x", "erin")) > ranker.score(&review("y", "frank")));
        assert_eq!(ranker.reason(&review("x", "erin"), &english).as_deref(), Some("You usually handle items from erin quickly."));
        assert_eq!(ranker.reason(&review("y", "frank"), &english), None);
    }

    #[test]
    fn features_name_the_place_and_title_words() {
        let mut item = review("x", "erin");
        item.title = "[WIP] feat(api): Add the retry queue".into();
        let features = PersonalRanker::features(&item);
        assert_eq!(features[..4], ["tool:test", "verb:code.review", "place:acme/app", "from:erin"]);
        assert_eq!(features[4..], ["word:add", "word:retry", "word:queue"]);
    }

    #[test]
    fn reasons_are_translated_and_tools_capitalized() {
        let french = |key: &str| if key == "You usually handle %@ items quickly." { "Vite : %@".into() } else { key.into() };
        assert_eq!(describe("tool:github", &french), "Vite : Github");
        assert_eq!(describe("badge:approved", &english), "You usually handle items like this quickly.");
    }

    #[test]
    fn records_are_camel_case_json() {
        let record = ActionRecord::new(&review("x", "erin"), ActionOutcome::Deferred, now());
        let json = serde_json::to_value(&record).unwrap();
        assert_eq!(json["outcome"], "deferred");
        assert_eq!(serde_json::from_value::<ActionRecord>(json).unwrap(), record);
    }

    /// Habits break ties after explicit priority, never before it.
    #[test]
    fn habits_break_ties_but_never_beat_priority() {
        let ranker = PersonalRanker::new(&records());
        let mut urgent_from_frank = review("u", "frank");
        urgent_from_frank.priority = Some(crate::Priority::Urgent);
        let mut items = vec![review("b", "frank"), review("a", "erin"), urgent_from_frank];
        let score = |item: &InboxItem| ranker.score(item);
        crate::Prioritizer::with_personal(now(), Some(&score)).sort(&mut items);
        assert_eq!(items.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(), ["u", "a", "b"]);
    }

    /// A usual reason needs a clear habit in that place.
    #[test]
    fn usual_reason_needs_a_clear_habit() {
        let advisor = crate::SnoozeAdvisor::new();
        let mut target = review("x", "erin");
        target.context = "acme/app #9".into();
        let history = |reasons: &[crate::SnoozeReason]| -> Vec<crate::SnoozeRecord> {
            reasons.iter().map(|r| crate::SnoozeRecord::new(&review("h", "erin"), Some(*r), now(), now() + chrono::Duration::hours(1))).collect()
        };
        use crate::SnoozeReason::*;
        assert_eq!(advisor.usual_reason(&target, &history(&[Waiting, Waiting, Waiting, NoTime])), Some(Waiting));
        assert_eq!(advisor.usual_reason(&target, &history(&[Waiting, NoTime, Focus])), None);
    }

}

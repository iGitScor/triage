//! What the Claude assistant produces (the brief, bundle summaries, triage suggestions) and what it may see and use
//!. A port of the macOS `Assistant.swift` and `AssistantPolicy.swift`: same rules, same tests.

use crate::{InboxItem, Snooze};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// A daily-brief style triage of the inbox.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Brief {
    pub summary: String,
    pub focus: Vec<Focus>,
    pub created_at: DateTime<Utc>,
}

/// One item to handle first, and why.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Focus {
    pub id: String,
    pub reason: String,
}

/// The end of a cache that lasts `cache_minutes` (zero or less: none).
fn cache_end(created_at: DateTime<Utc>, cache_minutes: i64) -> DateTime<Utc> {
    created_at + Duration::minutes(cache_minutes.max(0))
}

impl Brief {
    pub fn new(summary: impl Into<String>, focus: Vec<Focus>, created_at: DateTime<Utc>) -> Self {
        Brief { summary: summary.into(), focus, created_at }
    }

    /// When a new brief may be written; until then the current one is reused.
    pub fn fresh_until(&self, cache_minutes: i64) -> DateTime<Utc> {
        cache_end(self.created_at, cache_minutes)
    }

    pub fn is_fresh(&self, cache_minutes: i64, now: DateTime<Utc>) -> bool {
        now < self.fresh_until(cache_minutes)
    }
}

/// A one- or two-sentence take on a single bundle ("To review", "Mentions"…).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleSummary {
    pub text: String,
    /// Sorted, so the same items in another order still match. Named as the macOS app stores it.
    #[serde(rename = "itemIDs")]
    pub item_ids: Vec<String>,
    pub created_at: DateTime<Utc>,
}

fn sorted_ids(items: &[InboxItem]) -> Vec<String> {
    let mut ids: Vec<String> = items.iter().map(|i| i.id.clone()).collect();
    ids.sort();
    ids
}

impl BundleSummary {
    pub fn new(text: impl Into<String>, items: &[InboxItem], created_at: DateTime<Utc>) -> Self {
        BundleSummary { text: text.into(), item_ids: sorted_ids(items), created_at }
    }

    /// Reused only while recent and while the bundle still holds the same items.
    pub fn is_fresh(&self, items: &[InboxItem], cache_minutes: i64, now: DateTime<Utc>) -> bool {
        now < cache_end(self.created_at, cache_minutes) && self.item_ids == sorted_ids(items)
    }
}

/// A snoozed item as the assistant sees it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnoozedItem {
    pub item: InboxItem,
    pub snooze: Snooze,
    /// How many times it was snoozed lately.
    pub times: u32,
}

/// What the assistant proposes for one snoozed item.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TriageSuggestion {
    pub id: String,
    pub action: TriageAction,
    #[serde(default)]
    pub until: Option<DateTime<Utc>>,
    pub reason: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TriageAction {
    Keep,
    Reschedule,
    Done,
    Now,
}

impl TriageAction {
    /// In the order the macOS app lists them (the answer schema's enum).
    pub const ALL: [TriageAction; 4] =
        [TriageAction::Keep, TriageAction::Reschedule, TriageAction::Done, TriageAction::Now];

    pub fn as_str(self) -> &'static str {
        match self {
            TriageAction::Keep => "keep",
            TriageAction::Reschedule => "reschedule",
            TriageAction::Done => "done",
            TriageAction::Now => "now",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.as_str() == raw)
    }

    /// Ticked before the user looks. Only rescheduling, which keeps the item snoozed and is easy to undo: "done"
    /// hides an item and "now" opens its link, and the suggestions come from a model reading titles that anyone can
    /// write, so those two are the user's own choice.
    pub fn preselected(self) -> bool {
        self == TriageAction::Reschedule
    }
}

impl TriageSuggestion {
    /// What "Apply" would do: the preselected suggestions, plus or minus the ones the user ticked or unticked.
    /// "Keep" is never applied.
    pub fn selection(suggestions: &[TriageSuggestion], toggled: &HashSet<String>) -> Vec<TriageSuggestion> {
        suggestions
            .iter()
            .filter(|s| s.action != TriageAction::Keep && s.action.preselected() != toggled.contains(&s.id))
            .cloned()
            .collect()
    }
}

/// What the assistant may see and use, from the user's choices and the organization's policy keys
/// `AIExcludedSources` and `AllowedAIModels`. Which assistant (Claude Code or the Claude API) is `AllowedPlugins`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AssistantPolicy {
    /// Sources whose items never reach the assistant: plugin IDs, or "reminders" for your own reminders.
    pub excluded_sources: HashSet<String>,
    /// Models the assistant may use. None: any.
    pub allowed_models: Option<Vec<String>>,
}

impl AssistantPolicy {
    /// The assistant settings that name a model.
    pub const MODEL_FIELDS: [&'static str; 2] = ["model", "digestModel"];

    /// Blank model names are dropped: a policy listing only blanks allows any model.
    pub fn new(excluded_sources: HashSet<String>, allowed_models: Option<Vec<String>>) -> Self {
        let allowed_models =
            allowed_models.map(|m| m.iter().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect());
        AssistantPolicy { excluded_sources, allowed_models }
    }

    /// The items the assistant may read.
    pub fn items(&self, items: &[InboxItem]) -> Vec<InboxItem> {
        items.iter().filter(|i| self.allows(i)).cloned().collect()
    }

    pub fn allows(&self, item: &InboxItem) -> bool {
        !self.excluded_sources.contains(&item.plugin_id)
    }

    /// An assistant account's settings with every model field kept within `allowed_models`: a model that isn't
    /// allowed (or an empty field, whose default may not be) becomes the first allowed one.
    pub fn constrained(
        &self,
        settings: &HashMap<String, String>,
        defaults: &HashMap<String, String>,
    ) -> HashMap<String, String> {
        let mut settings = settings.clone();
        let Some(allowed) = &self.allowed_models else { return settings };
        let Some(first) = allowed.first() else { return settings };
        for key in Self::MODEL_FIELDS {
            let value = settings.get(key).map(|v| v.trim().to_string()).unwrap_or_default();
            let mut effective = if value.is_empty() { defaults.get(key).cloned().unwrap_or_default() } else { value };
            // A summaries model left empty means the brief's (OpenAI-compatible): check that one, as on macOS.
            if effective.is_empty() && key != "model" {
                effective = settings.get("model").or_else(|| defaults.get("model")).cloned().unwrap_or_default();
            }
            if !allowed.contains(&effective) {
                settings.insert(key.to_string(), first.clone());
            }
        }
        settings
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::{item, now};
    use crate::{InboxBundle, SnoozeMode};

    fn source(id: &str, plugin_id: &str) -> InboxItem {
        InboxItem { plugin_id: plugin_id.into(), ..item(id, InboxBundle::reviews()) }
    }

    fn map(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn brief_stays_fresh_for_the_cache_duration() {
        let brief = Brief::new("s", vec![], now());
        assert!(brief.is_fresh(30, now() + Duration::minutes(29)));
        assert!(!brief.is_fresh(30, now() + Duration::minutes(30)));
    }

    #[test]
    fn zero_minutes_means_no_cache() {
        let brief = Brief::new("s", vec![], now());
        assert!(!brief.is_fresh(0, now()));
        assert!(!brief.is_fresh(-5, now()));
    }

    #[test]
    fn bundle_summary_needs_the_same_items() {
        let items = vec![item("1", InboxBundle::reviews()), item("2", InboxBundle::reviews())];
        let summary = BundleSummary::new("t", &items, now());
        let reversed: Vec<InboxItem> = items.iter().rev().cloned().collect();
        assert!(summary.is_fresh(&reversed, 30, now()));
        let more = [items.clone(), vec![item("3", InboxBundle::reviews())]].concat();
        assert!(!summary.is_fresh(&more, 30, now()));
        assert!(!summary.is_fresh(&items, 30, now() + Duration::minutes(31)));
    }

    #[test]
    fn stored_as_the_macos_app_names_them() {
        let summary =
            serde_json::to_value(BundleSummary::new("t", &[item("1", InboxBundle::reviews())], now())).unwrap();
        assert_eq!(summary["itemIDs"], serde_json::json!(["1"]));
        assert!(summary.get("createdAt").is_some());
        let suggestion =
            TriageSuggestion { id: "a".into(), action: TriageAction::Reschedule, until: None, reason: "r".into() };
        assert_eq!(serde_json::to_value(&suggestion).unwrap()["action"], "reschedule");
        let snoozed = SnoozedItem {
            item: item("a", InboxBundle::reviews()),
            snooze: Snooze {
                until: now(),
                mode: SnoozeMode::Hide,
                note: None,
                fingerprint: "f".into(),
                reason: None,
                until_news: None,
            },
            times: 2,
        };
        assert_eq!(serde_json::from_value::<SnoozedItem>(serde_json::to_value(&snoozed).unwrap()).unwrap(), snoozed);
    }

    // Suggestions come from a model reading titles anyone can write; only reversible ones are ticked for you.

    fn suggestions() -> Vec<TriageSuggestion> {
        let s = |id: &str, action, reason: &str| TriageSuggestion {
            id: id.into(),
            action,
            until: None,
            reason: reason.into(),
        };
        vec![
            TriageSuggestion {
                until: Some(now() + Duration::days(1)),
                ..s("a", TriageAction::Reschedule, "Waiting on Bob")
            },
            s("b", TriageAction::Done, "Stale"),
            s("c", TriageAction::Now, "Due today"),
            s("d", TriageAction::Keep, "Fine as is"),
        ]
    }

    fn selected(toggled: &[&str]) -> Vec<String> {
        let toggled = toggled.iter().map(|s| s.to_string()).collect();
        TriageSuggestion::selection(&suggestions(), &toggled).into_iter().map(|s| s.id).collect()
    }

    #[test]
    fn only_reschedules_are_ticked_by_default() {
        assert_eq!(selected(&[]), ["a"]);
    }

    #[test]
    fn the_user_ticks_done_and_now_and_can_untick_a_reschedule() {
        assert_eq!(selected(&["b", "c"]), ["a", "b", "c"]);
        assert!(selected(&["a"]).is_empty());
    }

    #[test]
    fn keep_is_never_applied() {
        assert_eq!(selected(&["d"]), ["a"]);
        assert_eq!(
            TriageAction::ALL.into_iter().filter(|a| a.preselected()).collect::<Vec<_>>(),
            [TriageAction::Reschedule]
        );
        assert_eq!(TriageAction::parse("now"), Some(TriageAction::Now));
        assert_eq!(TriageAction::parse("snooze"), None);
    }

    // Sources kept away from the assistant, and models limited by the organization.

    #[test]
    fn excluded_sources_never_reach_the_assistant() {
        let policy = AssistantPolicy::new(HashSet::from(["slack".to_string(), "reminders".to_string()]), None);
        let items = vec![source("a", "github"), source("b", "slack"), source("c", "reminders"), source("d", "linear")];
        assert_eq!(policy.items(&items).iter().map(|i| i.id.as_str()).collect::<Vec<_>>(), ["a", "d"]);
        assert_eq!(AssistantPolicy::default().items(&items).len(), 4);
    }

    #[test]
    fn models_stay_within_the_allowed_list() {
        let defaults = map(&[("model", "claude-opus-5-5"), ("digestModel", "claude-haiku-5-5")]);
        let policy =
            AssistantPolicy::new(HashSet::new(), Some(vec!["claude-sonnet-5-5".into(), "claude-haiku-5-5".into()]));
        // Opus by default isn't allowed: the first allowed model replaces it; Haiku is allowed and stays.
        let settings = policy.constrained(&map(&[("model", ""), ("token", "k")]), &defaults);
        assert_eq!(settings["model"], "claude-sonnet-5-5");
        assert_eq!(settings.get("digestModel"), None, "empty means Haiku, which is allowed: left as is");
        assert_eq!(
            policy.constrained(&map(&[("model", "claude-opus-5-5"), ("digestModel", "claude-haiku-5-5")]), &defaults),
            map(&[("model", "claude-sonnet-5-5"), ("digestModel", "claude-haiku-5-5")])
        );
        assert_eq!(settings["token"], "k");
        // Claude Code's empty model means its own default, which nobody can vouch for: it becomes the first allowed.
        assert_eq!(policy.constrained(&HashMap::new(), &HashMap::new())["model"], "claude-sonnet-5-5");
    }

    #[test]
    fn without_an_allow_list_nothing_changes() {
        let settings = map(&[("model", "claude-opus-5-5")]);
        assert_eq!(AssistantPolicy::default().constrained(&settings, &HashMap::new()), settings);
        let blanks = AssistantPolicy::new(HashSet::new(), Some(vec![" ".into(), String::new()]));
        assert_eq!(blanks.constrained(&settings, &HashMap::new()), settings);
    }
}

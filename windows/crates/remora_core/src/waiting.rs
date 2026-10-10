//! Helps with what you're waiting on: who could review, and a friendly nudge when nobody answers.
//! Drafts are written locally from templates; nothing is sent. Port of the macOS `WaitingAssistant.swift`.
use crate::review_prep::{diff_size, fill, plural};
use crate::snooze_advisor::context_key;
use crate::{InboxBundle, InboxItem, Person};
use chrono::{DateTime, Duration, Utc};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WaitingHelp {
    /// Nobody is reviewing yet: people who could.
    SuggestReviewers(Vec<Person>),
    /// Reviewers asked, no answer for a while.
    Nudge { people: Vec<Person>, days: i64 },
}

#[derive(Clone, Debug)]
pub struct WaitingAssistant {
    /// A reviewer counts as silent after this long without activity.
    pub patience: Duration,
}

impl Default for WaitingAssistant {
    fn default() -> Self {
        WaitingAssistant { patience: Duration::days(1) }
    }
}

impl WaitingAssistant {
    pub fn help(&self, item: &InboxItem, items: &[InboxItem], me: Option<&str>, now: DateTime<Utc>) -> Option<WaitingHelp> {
        if item.bundle != InboxBundle::awaiting() || item.has_badge("draft") {
            return None;
        }
        if item.participants.is_empty() {
            let people = self.reviewers(item, items, me, 3);
            return (!people.is_empty()).then_some(WaitingHelp::SuggestReviewers(people));
        }
        let waiting_on: Vec<Person> = item.participants.iter().filter(|p| p.tone.is_none()).cloned().collect();
        let silence = now - item.date;
        if waiting_on.is_empty() || silence <= self.patience {
            return None;
        }
        Some(WaitingHelp::Nudge { people: waiting_on, days: silence.num_seconds().div_euclid(86_400).max(1) })
    }

    /// The source's suggestions first, then who reviews most often in the same repo.
    pub fn reviewers(&self, item: &InboxItem, items: &[InboxItem], me: Option<&str>, limit: usize) -> Vec<Person> {
        let excluded: HashSet<String> =
            [me, item.author.as_ref().map(|a| a.name.as_str())].into_iter().flatten().map(str::to_lowercase).collect();
        let place = context_key(&item.context);
        let local: Vec<&Person> = items
            .iter()
            .filter(|i| i.id != item.id && context_key(&i.context) == place)
            .flat_map(|i| &i.participants)
            .collect();
        let mut frequency: HashMap<&str, usize> = HashMap::new();
        for person in &local {
            *frequency.entry(person.name.as_str()).or_default() += 1;
        }
        let mut names: Vec<(&str, usize)> = frequency.into_iter().collect();
        // Most frequent first, then by name.
        names.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
        let ranked = names.into_iter().filter_map(|(name, _)| local.iter().find(|p| p.name == name).copied());
        let mut seen = HashSet::new();
        item.suggested_people
            .iter()
            .flatten()
            .chain(ranked)
            .filter(|p| !excluded.contains(&p.name.to_lowercase()) && seen.insert(p.name.to_lowercase()))
            .map(|p| Person { name: p.name.clone(), avatar_url: p.avatar_url.clone(), tone: None })
            .take(limit)
            .collect()
    }

    /// A short, kind message you can paste anywhere. `tr` maps each English key (the Mac's) to the UI language's
    /// format string, e.g. `|k| translator.t(k)`; the placeholders are filled here.
    pub fn draft(&self, help: &WaitingHelp, item: &InboxItem, tr: &dyn Fn(&str) -> String) -> String {
        let facts = facts(item, tr);
        let link = item.url.clone().unwrap_or_else(|| item.context.clone());
        match help {
            WaitingHelp::SuggestReviewers(people) => {
                let names = people.iter().take(1).map(|p| format!("@{}", p.name)).collect::<String>();
                fill(&tr("Hi %@, could you review “%@”? %@\n%@\nThanks!"), &[names, item.title.clone(), facts, link])
            }
            WaitingHelp::Nudge { people, days } => {
                let names = people.iter().map(|p| format!("@{}", p.name)).collect::<Vec<_>>().join(" ");
                let waited = plural("%d day", "%d days", *days, tr);
                fill(
                    &tr("Hi %@, a gentle ping on “%@”: it has been waiting for a review for %@. %@\n%@\nThanks!"),
                    &[names, item.title.clone(), waited, facts, link],
                )
            }
        }
    }
}

/// "It's small (+12 −3) and checks are green." Only what's true and useful.
fn facts(item: &InboxItem, tr: &dyn Fn(&str) -> String) -> String {
    let small = diff_size(item).is_some_and(|size| size < 150);
    let green = item.has_badge("checks.passing");
    let diff = || item.badges.iter().find(|b| b.id == "diff").map(|b| b.label.clone()).unwrap_or_default();
    match (small, green) {
        (true, true) => fill(&tr("It’s small (%@) and checks are green."), &[diff()]),
        (true, false) => fill(&tr("It’s small (%@)."), &[diff()]),
        (false, true) => tr("Checks are green."),
        (false, false) => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::*;
    use crate::{Badge, Tone};

    fn english(key: &str) -> String {
        key.to_string()
    }

    fn person(name: &str) -> Person {
        Person::named(name)
    }

    fn mine(id: &str, participants: Vec<Person>, hours_ago: i64, suggested: Option<Vec<Person>>) -> InboxItem {
        let mut item = item(id, InboxBundle::awaiting());
        item.date = now() - Duration::hours(hours_ago);
        item.context = format!("acme/app #{id}");
        item.title = "Add retries".into();
        item.author = Some(person("alice"));
        item.participants = participants;
        item.suggested_people = suggested;
        item.badges = vec![Badge::new("diff", "+12 −3", Tone::Neutral), Badge::new("checks.passing", "Checks", Tone::Positive)];
        item.url = Some(format!("https://github.com/acme/app/pull/{id}"));
        item
    }

    #[test]
    fn suggests_source_then_local_reviewers_but_never_you() {
        let others = [
            mine("2", vec![person("frank"), person("erin")], 1, None),
            mine("3", vec![person("erin"), person("alice")], 1, None),
        ];
        let item = mine("1", vec![], 1, Some(vec![person("dave")]));
        assert_eq!(
            WaitingAssistant::default().help(&item, &others, Some("alice"), now()),
            Some(WaitingHelp::SuggestReviewers(vec![person("dave"), person("erin"), person("frank")]))
        );
    }

    #[test]
    fn nudges_silent_reviewers_after_a_day() {
        let assistant = WaitingAssistant::default();
        let frank = Person { tone: Some(Tone::Accent), ..person("frank") };
        let waiting = mine("1", vec![person("erin"), frank], 50, None);
        assert_eq!(
            assistant.help(&waiting, &[], Some("alice"), now()),
            Some(WaitingHelp::Nudge { people: vec![person("erin")], days: 2 })
        );
        let fresh = mine("1", vec![person("erin")], 3, None);
        assert_eq!(assistant.help(&fresh, &[], Some("alice"), now()), None);
    }

    #[test]
    fn drafts_state_only_useful_facts() {
        let item = mine("1", vec![person("erin")], 50, None);
        let help = WaitingHelp::Nudge { people: vec![person("erin")], days: 2 };
        let draft = WaitingAssistant::default().draft(&help, &item, &english);
        assert!(draft.contains("@erin") && draft.contains("“Add retries”") && draft.contains("2 day"));
        assert!(draft.contains("small (+12 −3) and checks are green"));
        assert!(draft.contains("https://github.com/acme/app/pull/1"));
    }

    /// The UI's translator is applied to every key, including the day count.
    #[test]
    fn drafts_go_through_the_translator() {
        let french = |key: &str| match key {
            "%d day" => "%d jour".to_string(),
            "%d days" => "%d jours".to_string(),
            "Checks are green." => "Les tests passent.".to_string(),
            k if k.starts_with("Hi %@, a gentle ping") => "Salut %@ : « %@ » attend depuis %@. %@\n%@".to_string(),
            k => k.to_string(),
        };
        let mut item = mine("1", vec![person("erin")], 50, None);
        item.badges.retain(|b| b.id != "diff");
        let help = WaitingHelp::Nudge { people: vec![person("erin")], days: 2 };
        assert_eq!(
            WaitingAssistant::default().draft(&help, &item, &french),
            "Salut @erin : « Add retries » attend depuis 2 jours. Les tests passent.\nhttps://github.com/acme/app/pull/1"
        );
    }

    #[test]
    fn drafts_and_review_items_get_no_help() {
        let assistant = WaitingAssistant::default();
        let mut draft = mine("1", vec![], 1, None);
        draft.badges.push(Badge::new("draft", "Draft", Tone::Neutral));
        assert_eq!(assistant.help(&draft, &[], None, now()), None);
        assert_eq!(assistant.help(&item("2", InboxBundle::reviews()), &[], None, now()), None);
    }

    #[test]
    fn context_key_drops_the_item_number() {
        assert_eq!(context_key("acme/app #12"), "acme/app");
        assert_eq!(context_key("g/p !3"), "g/p");
        assert_eq!(context_key("#releases"), "#releases");
        assert_eq!(context_key("#12"), "#12");
    }
}

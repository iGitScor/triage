//! Finds items from different tools that are about the same thing, even when nobody linked them: a shared ticket
//! key ("CE-1390"), or titles with most significant words in common. Port of the macOS `LinkFinder.swift` with its
//! keyword similarity only: the Mac also uses Apple's word embeddings and a rare-token model, which Windows lacks.
use crate::{InboxItem, TicketKeys};
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

static TAGS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\s*\[[^\]]*\])+\s*").expect("valid regex"));
static COMMIT_PREFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-zA-Z]+(\([^)]*\))?!?:\s*").expect("valid regex"));

/// Words too common to tell two titles apart, in English and French (Swift's `KeywordSimilarity.stopWords`).
pub(crate) const STOP_WORDS: &[&str] = &[
    "the", "and", "for", "with", "from", "into", "this", "that", "your", "about", "when", "what", "after", "new",
    "pour", "avec", "dans", "les", "des", "une", "sur", "est", "pas", "qui", "que", "vous", "nous", "vers", "la", "le",
    "de", "du", "au",
];

/// An item with a similar title: its id, how close the titles are, and its date.
type Similar = (String, f64, chrono::DateTime<chrono::Utc>);

#[derive(Clone, Debug)]
pub struct LinkFinder {
    /// At most this many links per item, most certain first.
    pub limit: usize,
}

impl Default for LinkFinder {
    fn default() -> Self {
        LinkFinder { limit: 3 }
    }
}

impl LinkFinder {
    /// Item ID → linked item IDs: shared ticket keys first, then similar titles.
    pub fn links(&self, items: &[InboxItem]) -> HashMap<String, Vec<String>> {
        let reminders = crate::InboxBundle::reminders();
        let candidates: Vec<&InboxItem> = items.iter().filter(|i| i.bundle != reminders).collect();
        let tickets = TicketKeys::known_in(candidates.iter().copied());
        let keys: Vec<HashSet<String>> = candidates.iter().map(|i| keys_of(i, &tickets)).collect();
        let titles: Vec<String> = candidates.iter().map(|i| normalized(&i.title)).collect();
        let mut strong: HashMap<&str, Vec<String>> = HashMap::new();
        // Similar titles, with how close they are, to keep the closest when there are more than `limit`.
        let mut weak: HashMap<&str, Vec<Similar>> = HashMap::new();

        for i in 0..candidates.len() {
            for j in i + 1..candidates.len() {
                if candidates[i].plugin_id == candidates[j].plugin_id {
                    continue;
                }
                let (a, b) = (candidates[i].id.as_str(), candidates[j].id.as_str());
                if !keys[i].is_disjoint(&keys[j]) {
                    strong.entry(a).or_default().push(b.to_string());
                    strong.entry(b).or_default().push(a.to_string());
                } else if keyword_similar(&titles[i], &titles[j]) {
                    let score = closeness(&titles[i], &titles[j]);
                    weak.entry(a).or_default().push((b.to_string(), score, candidates[j].date));
                    weak.entry(b).or_default().push((a.to_string(), score, candidates[i].date));
                }
            }
        }
        let ids: HashSet<&str> = strong.keys().chain(weak.keys()).copied().collect();
        ids.into_iter()
            .map(|id| {
                // Closest first, then the most recent: not the order the sources happened to list them in.
                let mut similar = weak.get(id).cloned().unwrap_or_default();
                similar.sort_by(|x, y| y.1.total_cmp(&x.1).then(y.2.cmp(&x.2)).then(x.0.cmp(&y.0)));
                let linked =
                    strong.get(id).cloned().unwrap_or_default().into_iter().chain(similar.into_iter().map(|s| s.0));
                (id.to_string(), linked.take(self.limit).collect())
            })
            .collect()
    }
}

/// How close two titles are: the share of their significant words they have in common (Swift's
/// `LinkFinder.closeness`).
fn closeness(a: &str, b: &str) -> f64 {
    let (words_a, words_b) = (keywords(a), keywords(b));
    let union = words_a.union(&words_b).count();
    if union == 0 {
        0.0
    } else {
        words_a.intersection(&words_b).count() as f64 / union as f64
    }
}

/// Ticket keys in the title, the context (a Linear identifier lives there) and the preview.
fn keys_of(item: &InboxItem, tickets: &TicketKeys) -> HashSet<String> {
    tickets.keys(&[item.title.as_str(), &item.context, item.preview.as_deref().unwrap_or("")].join(" "))
}

/// "[SOAK] chore(deps): Bump axios" → "Bump axios": tags and commit-style prefixes say nothing about the topic.
pub(crate) fn normalized(title: &str) -> String {
    let untagged = TAGS.replace(title, "");
    COMMIT_PREFIX.replace(&untagged, "").into_owned()
}

/// Titles alone say nothing of which prefixes are real, so only keys written in capitals count.
pub(crate) fn ticket_keys(text: &str) -> HashSet<String> {
    TicketKeys::default().keys(text)
}

/// Same ticket key ("ENG-123"), or most significant words in common (Swift's `KeywordSimilarity.similar`).
pub(crate) fn keyword_similar(a: &str, b: &str) -> bool {
    if !ticket_keys(a).is_disjoint(&ticket_keys(b)) {
        return true;
    }
    let (words_a, words_b) = (keywords(a), keywords(b));
    let shared = words_a.intersection(&words_b).count();
    let union = words_a.union(&words_b).count();
    shared >= 2 && shared as f64 / union.max(1) as f64 >= 0.4
}

fn keywords(text: &str) -> HashSet<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() >= 4 && !STOP_WORDS.contains(w))
        .map(String::from)
        .collect()
}

/// The words of a title in order, letters only (Swift's `EmbeddingSimilarity.words`), for the personal ranker.
pub(crate) fn title_words(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphabetic())
        .filter(|w| w.chars().count() >= 3 && !STOP_WORDS.contains(w))
        .map(String::from)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::*;
    use crate::InboxBundle;

    fn task(id: &str, plugin: &str, title: &str, context: &str) -> InboxItem {
        let mut item = item(id, InboxBundle::tasks());
        item.plugin_id = plugin.into();
        item.title = title.into();
        item.context = context.into();
        item
    }

    #[test]
    fn shared_ticket_key_links_across_tools() {
        let issue = task("1", "linear", "Close Dotfile cases", "CE-1380");
        let pr = task("2", "github", "CE-1380: close cases on account closure", "");
        let links = LinkFinder::default().links(&[issue, pr]);
        assert_eq!(links["1"], ["2"]);
        assert_eq!(links["2"], ["1"]);
    }

    #[test]
    fn same_tool_is_never_linked() {
        let a = task("1", "github", "CE-1380 part 1", "");
        let b = task("2", "github", "CE-1380 part 2", "");
        assert!(LinkFinder::default().links(&[a, b]).is_empty());
    }

    #[test]
    fn prefixes_and_tags_are_stripped() {
        assert_eq!(normalized("[SOAK] [WIP] chore(deps): Bump axios"), "Bump axios");
        assert_eq!(normalized("feat!: New search"), "New search");
    }

    /// The Mac test's second half (the first needs Apple's embeddings): an ordinary shared word is not a link.
    #[test]
    fn ordinary_words_do_not_link() {
        let faker = task("3", "linear", "Update @faker-js/faker to address security vulnerabilities", "");
        let address = task("4", "github", "[SOAK] feat(api): add merchant address to virtual account- #13924", "");
        assert!(!LinkFinder::default().links(&[faker, address]).contains_key("3"));
    }

    #[test]
    fn titles_with_most_words_in_common_link_after_keys() {
        let items = [
            task("1", "linear", "Retry failed payments webhook", ""),
            task("2", "github", "fix: retry failed payments webhook", ""),
            task("3", "slack", "About CE-7 rollout", "CE-7"),
            task("4", "github", "CE-7 rollout plan", ""),
            task("5", "reminders", "Retry failed payments webhook", ""),
        ];
        let mut items = items.to_vec();
        items[4].bundle = InboxBundle::reminders();
        let links = LinkFinder::default().links(&items);
        assert_eq!(links["1"], ["2"]);
        assert_eq!(links["4"], ["3"]);
        assert!(!links.contains_key("5"), "reminders are never linked");
    }

    /// Two tools mentioning UTF-8 are not about the same thing; a Linear key typed in lower case in Slack is.
    #[test]
    fn versions_do_not_link_but_a_lower_case_known_key_does() {
        let items = [
            task("1", "linear", "Close Dotfile cases", "CE-1380"),
            task("2", "slack", "can someone look at ce-1380?", ""),
            task("3", "github", "Read files as utf-8", ""),
            task("4", "teams", "Export breaks on UTF-8 names", ""),
        ];
        let links = LinkFinder::default().links(&items);
        assert_eq!(links["1"], ["2"]);
        assert_eq!(links["2"], ["1"]);
        assert!(!links.contains_key("3") && !links.contains_key("4"));
    }

    /// More similar items than links shown: the closest are kept, not the first ones listed. Same case as the Mac.
    #[test]
    fn the_closest_similar_items_are_kept() {
        let links = LinkFinder::default().links(&[
            task("a", "github", "Export invoices to CSV for accounting", ""),
            task("b1", "linear", "Export invoices weekly accounting team report", ""),
            task("b4", "notion", "Invoices accounting export broken today", ""),
            task("b3", "jira", "Export invoices accounting monthly", ""),
            task("b2", "slack", "Export invoices accounting", ""),
        ]);
        assert_eq!(links["a"], ["b2", "b3", "b4"], "all the same words, then three of four, then three of five");
    }
}

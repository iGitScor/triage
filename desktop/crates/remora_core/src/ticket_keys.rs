//! Ticket keys ("CE-1380") in free text (Swift's `TicketKeys`). A prefix seen as a real key (a Linear or Jira item's
//! context is its key) is matched in any case, "ce-1380" in a Slack message too. Any other prefix must be written in
//! capitals and not be a version or an encoding ("UTF-8", "SHA-256", "Q4-2025"). A key starts a word: "vue-router-4"
//! has none. A CVE counts whole ("CVE-2024-3094"), never by its year.
use crate::InboxItem;
use regex::Regex;
use std::collections::HashSet;
use std::sync::LazyLock;

static CANDIDATE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"([A-Za-z][A-Za-z0-9]+)-([0-9]+)").expect("valid regex"));
static CVE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)CVE-[0-9]{4}-[0-9]+").expect("valid regex"));
static WHOLE_KEY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^([A-Z][A-Z0-9]+)-[0-9]+$").expect("valid regex"));

const NOT_TICKETS: &[&str] = &[
    "UTF", "UCS", "SHA", "MD", "AES", "RSA", "TLS", "SSL", "HTTP", "ISO", "IEC", "RFC", "PEP", "ES", "ECMA", "COVID",
    "CWE", "GPT", "IPV", "TCP", "UDP", "Q1", "Q2", "Q3", "Q4", "H1", "H2", "FY", "WIN", "IOS", "ARM", "PY", "NODE",
];

#[derive(Clone, Debug, Default)]
pub struct TicketKeys {
    /// Upper-cased prefixes of real keys.
    pub known: HashSet<String>,
}

impl TicketKeys {
    /// The prefixes of the items whose whole context is a key: Linear identifiers and Jira keys.
    pub fn known_in<'a>(items: impl IntoIterator<Item = &'a InboxItem>) -> Self {
        let known =
            items.into_iter().filter_map(|i| WHOLE_KEY.captures(&i.context)).map(|c| c[1].to_string()).collect();
        TicketKeys { known }
    }

    pub fn keys(&self, text: &str) -> HashSet<String> {
        let mut keys: HashSet<String> = CVE
            .find_iter(text)
            .filter(|m| bounded(text, m.start(), m.end()))
            .map(|m| m.as_str().to_uppercase())
            .collect();
        for c in CANDIDATE.captures_iter(text) {
            let whole = c.get(0).expect("whole match");
            if !bounded(text, whole.start(), whole.end()) {
                continue;
            }
            let prefix = &c[1];
            let upper = prefix.to_uppercase();
            if upper == "CVE" {
                continue;
            }
            if self.known.contains(&upper) || (prefix == upper && !NOT_TICKETS.contains(&upper.as_str())) {
                keys.insert(format!("{upper}-{}", &c[2]));
            }
        }
        keys
    }
}

/// Starts a word, not inside one nor a hyphenated name ("xCE-1", "vue-router-4", "a_b-1"), and ends one
/// ("SHA-256abc" is not a key; "CE-1380-close-cases" and "CE-1380." are). The regex crate has no lookbehind.
fn bounded(text: &str, start: usize, end: usize) -> bool {
    let starts = text[..start].chars().next_back().is_none_or(|c| !(c.is_alphanumeric() || c == '-' || c == '_'));
    let ends = text[end..].chars().next().is_none_or(|c| !(c.is_alphanumeric() || c == '_'));
    starts && ends
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::*;
    use crate::InboxBundle;

    fn set(keys: &[&str]) -> HashSet<String> {
        keys.iter().map(|k| k.to_string()).collect()
    }

    #[test]
    fn versions_and_encodings_are_not_keys() {
        let text =
            "utf-8 UTF-8 sha-256 SHA-256 covid-19 node-20 NODE-20 Q4-2025 ubuntu-22 vue-router-4 xCE-12 SHA-256abc";
        assert!(TicketKeys::default().keys(text).is_empty());
    }

    #[test]
    fn capital_keys_starting_a_word_count() {
        let keys = TicketKeys::default().keys("[CE-1380] fix, see /browse/ENG-7. and PAY2-3");
        assert_eq!(keys, set(&["CE-1380", "ENG-7", "PAY2-3"]));
    }

    #[test]
    fn known_prefixes_match_in_any_case() {
        assert!(TicketKeys::default().keys("blocked on ce-1380").is_empty());
        let known = TicketKeys { known: set(&["CE"]) };
        assert_eq!(known.keys("blocked on ce-1380"), set(&["CE-1380"]));
    }

    #[test]
    fn a_cve_counts_whole_never_by_its_year() {
        let keys = TicketKeys::default().keys("Patch cve-2024-3094 and CVE-2024-6387");
        assert_eq!(keys, set(&["CVE-2024-3094", "CVE-2024-6387"]));
        assert!(TicketKeys::default().keys("CVE-2024 review").is_empty());
    }

    #[test]
    fn known_prefixes_come_from_contexts_that_are_keys() {
        let mut linear = item("1", InboxBundle::tasks());
        linear.context = "CE-1380".into();
        let mut github = item("2", InboxBundle::tasks());
        github.context = "acme/app #12".into();
        assert_eq!(TicketKeys::known_in([&linear, &github]).known, set(&["CE"]));
    }
}

use std::sync::LazyLock;

use regex::Regex;

use crate::{InboxBundle, InboxItem};

/// What a chat message asks of you.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextIntent {
    Request,
    Info,
}

/// Explicit wording in English and French. Requests win over FYI ("FYI, can you check?"), but a negated request
/// asks nothing: "no need to reply" is information, and "not urgent" cancels "urgent". A request said
/// elsewhere in the message still counts ("no need to reply, but can you check?").
/// Keywords match whole words, a question mark only ends a sentence, and links and code are left out:
/// "pleased", `a ?? b` and `search?q=x` ask nothing.
pub struct KeywordIntentClassifier;

/// Links and code, which aren't wording.
static NOT_PROSE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)```.*?```|`[^`]*`|(https?://|www\.)\S+").expect("valid pattern"));
/// A question mark that ends a sentence: before a space, a closing quote or bracket, or the end.
static QUESTION: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"\?+["'”’)\]]*(\s|$)"#).expect("valid pattern"));
static REQUEST_WORDS: LazyLock<Regex> = LazyLock::new(|| word_pattern(KeywordIntentClassifier::REQUESTS));
static INFO_WORDS: LazyLock<Regex> = LazyLock::new(|| word_pattern(KeywordIntentClassifier::INFOS));
static NO_REPLY_WORDS: LazyLock<Regex> =
    LazyLock::new(|| word_pattern(&with_apostrophes(KeywordIntentClassifier::NO_REPLY)));
static NOT_URGENT_WORDS: LazyLock<Regex> =
    LazyLock::new(|| word_pattern(&with_apostrophes(KeywordIntentClassifier::NOT_URGENT)));

/// Each phrase with a straight and a curly apostrophe, as people type both.
fn with_apostrophes(phrases: &[&str]) -> Vec<String> {
    let mut all: Vec<String> = phrases.iter().map(|p| p.to_string()).collect();
    all.extend(phrases.iter().filter(|p| p.contains('\'')).map(|p| p.replace('\'', "’")));
    all
}

/// Any of the keywords as whole words: not preceded or followed by a letter or a digit. One pattern, compiled once.
fn word_pattern<S: AsRef<str>>(keywords: &[S]) -> Regex {
    let mut keywords: Vec<&str> = keywords.iter().map(AsRef::as_ref).collect();
    keywords.sort_by_key(|k| std::cmp::Reverse(k.chars().count()));
    let alternatives = keywords.iter().map(|k| regex::escape(k)).collect::<Vec<_>>().join("|");
    Regex::new(&format!(r"(^|[^\p{{L}}\p{{N}}])({alternatives})($|[^\p{{L}}\p{{N}}])")).expect("valid pattern")
}

impl KeywordIntentClassifier {
    const REQUESTS: &'static [&'static str] = &[
        "can you",
        "could you",
        "would you",
        "will you",
        "please",
        "pls",
        "plz",
        "let me know",
        "lmk",
        "what do you think",
        "thoughts",
        "your opinion",
        "need you",
        "waiting for you",
        "asap",
        "urgent",
        "peux-tu",
        "pourrais-tu",
        "tu peux",
        "tu pourrais",
        "pouvez-vous",
        "pourriez-vous",
        "vous pouvez",
        "merci de",
        "stp",
        "svp",
        "s'il te plaît",
        "s’il te plaît",
        "s'il vous plaît",
        "est-ce que",
        "qu'en penses",
        "qu’en penses",
        "dis-moi",
        "dites-moi",
        "ton avis",
        "votre avis",
        "besoin de toi",
    ];
    /// Says that nothing is expected: information, and taken out before looking for requests.
    const NO_REPLY: &'static [&'static str] = &[
        "no need to reply",
        "no need to answer",
        "no need to respond",
        "no need to do anything",
        "no reply needed",
        "no response needed",
        "no reply necessary",
        "no action needed",
        "no action required",
        "nothing to do",
        "you don't need to reply",
        "you don't have to reply",
        "don't need to reply",
        "no need for a reply",
        "pas besoin de répondre",
        "pas besoin de me répondre",
        "pas besoin de réponse",
        "inutile de répondre",
        "pas la peine de répondre",
        "aucune action requise",
        "aucune action nécessaire",
        "rien à faire",
        "tu n'as pas besoin de répondre",
        "vous n'avez pas besoin de répondre",
    ];
    /// Takes the urgency out, nothing more: "not urgent, but can you look?" is still a request.
    const NOT_URGENT: &'static [&'static str] = &[
        "not urgent",
        "nothing urgent",
        "no rush",
        "no hurry",
        "pas urgent",
        "rien d'urgent",
        "pas d'urgence",
        "sans urgence",
        "pas pressé",
        "pas de rush",
    ];
    const INFOS: &'static [&'static str] = &[
        "fyi",
        "for your information",
        "heads up",
        "heads-up",
        "just so you know",
        "announcement",
        "@here",
        "@channel",
        "@everyone",
        "pour info",
        "pour information",
        "pour rappel",
        "à titre d'info",
        "a titre d'info",
        "je vous informe",
        "annonce",
    ];

    pub fn intent(text: &str) -> Option<TextIntent> {
        let text = text.to_lowercase();
        let text = NOT_PROSE.replace_all(&text, " ");
        let says_no_reply = NO_REPLY_WORDS.is_match(&text);
        let text = NO_REPLY_WORDS.replace_all(&text, "$1 $3");
        let text = NOT_URGENT_WORDS.replace_all(&text, "$1 $3");
        if QUESTION.is_match(&text) || REQUEST_WORDS.is_match(&text) {
            return Some(TextIntent::Request);
        }
        if says_no_reply || INFO_WORDS.is_match(&text) {
            return Some(TextIntent::Info);
        }
        None
    }
}

/// Moves items from the kind a plugin reported to the verb you have to perform. On Windows the chat
/// text goes through keywords only; when unsure, a message goes to *To reply*.
pub struct VerbClassifier;

impl VerbClassifier {
    pub fn classify(mut item: InboxItem) -> InboxItem {
        match item.bundle.id.as_str() {
            "code.authored" => {
                item.bundle = Self::verb_for_authored(&item);
                item.needs_action = item.bundle != InboxBundle::awaiting();
            }
            "chat.direct" | "chat.mentions" => {
                let text = [Some(item.title.as_str()), item.preview.as_deref()]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join("\n");
                let intent = KeywordIntentClassifier::intent(&text).unwrap_or(TextIntent::Request);
                item.bundle = if intent == TextIntent::Request { InboxBundle::reply() } else { InboxBundle::read() };
                item.needs_action = intent == TextIntent::Request;
            }
            _ => {}
        }
        item
    }

    fn verb_for_authored(item: &InboxItem) -> InboxBundle {
        if item.has_badge("draft") {
            InboxBundle::awaiting()
        } else if item.has_badge("changes") || item.has_badge("checks.failing") || item.has_badge("conflicts") {
            InboxBundle::fix()
        } else if item.has_badge("approved") {
            InboxBundle::merge()
        } else {
            InboxBundle::awaiting()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::*;
    use crate::{Badge, Tone};

    #[test]
    fn keywords_catch_explicit_wording() {
        let cases = [
            ("Can you check the rollout plan?", TextIntent::Request),
            ("Tu peux regarder la PR stp", TextIntent::Request),
            ("Merci de valider avant vendredi", TextIntent::Request),
            ("FYI the deploy is done", TextIntent::Info),
            ("@here the office is closed tomorrow", TextIntent::Info),
            ("Pour info, la release est partie", TextIntent::Info),
            ("FYI, can you double check?", TextIntent::Request),
        ];
        for (text, expected) in cases {
            assert_eq!(KeywordIntentClassifier::intent(text), Some(expected), "{text}");
        }
        assert_eq!(KeywordIntentClassifier::intent("Lunch tomorrow"), None);
    }

    /// Whole words, a question mark that ends a sentence, links and code left out. Same cases as macOS.
    /// A negated request asks nothing; a real request elsewhere in the message still does.
    #[test]
    fn negated_requests_ask_nothing() {
        let cases = [
            ("No need to reply, the build is green.", Some(TextIntent::Info)),
            ("Pas besoin de répondre, c’est réglé.", Some(TextIntent::Info)),
            ("FYI no action needed on your side", Some(TextIntent::Info)),
            ("Rien à faire de ton côté, merci !", Some(TextIntent::Info)),
            ("Not urgent, just sharing the doc", None),
            ("Pas urgent, juste pour te tenir au courant", None),
            ("No need to reply, but can you check the doc", Some(TextIntent::Request)),
            ("Not urgent, but could you look at it", Some(TextIntent::Request)),
            ("Urgent: the deploy is broken", Some(TextIntent::Request)),
        ];
        for (text, expected) in cases {
            assert_eq!(KeywordIntentClassifier::intent(text), expected, "{text}");
        }
    }

    #[test]
    fn keywords_match_words_not_fragments() {
        let cases = [
            ("Pleased to share the new office plan", None),
            ("See https://example.com/search?q=remora for the numbers", None),
            ("Use `a ?? b` when the value can be missing", None),
            ("The stpierre account is migrated", None),
            ("Thoughtful review, merged", None),
            ("Is the deploy done?", Some(TextIntent::Request)),
            ("Ready? Let's ship it", Some(TextIntent::Request)),
            ("Tu peux regarder ?", Some(TextIntent::Request)),
            ("Can you check (the second link)?", Some(TextIntent::Request)),
            ("please merge", Some(TextIntent::Request)),
            ("Heads-up: the API moves on Monday", Some(TextIntent::Info)),
            ("FYI: www.example.com/faq?x=1 is updated", Some(TextIntent::Info)),
        ];
        for (text, expected) in cases {
            assert_eq!(KeywordIntentClassifier::intent(text), expected, "{text}");
        }
    }

    #[test]
    fn authored_merge_requests_get_a_verb() {
        let authored = |badges: &[&str]| {
            let mut item = item("1", InboxBundle::authored());
            item.badges = badges.iter().map(|id| Badge::new(id, id, Tone::Neutral)).collect();
            VerbClassifier::classify(item).bundle
        };
        assert_eq!(authored(&["approved"]), InboxBundle::merge());
        assert_eq!(authored(&["approved", "checks.failing"]), InboxBundle::fix());
        assert_eq!(authored(&["changes"]), InboxBundle::fix());
        assert_eq!(authored(&["draft", "changes"]), InboxBundle::awaiting());
        assert_eq!(authored(&[]), InboxBundle::awaiting());
    }

    #[test]
    fn chat_becomes_reply_or_read_and_unsure_goes_to_reply() {
        let mut question = item("1", InboxBundle::mentions());
        question.title = "Can you review the migration?".into();
        let mut news = item("2", InboxBundle::mentions());
        news.title = "FYI the migration is done".into();
        let mut unsure = item("3", InboxBundle::direct_messages());
        unsure.title = "Lunch tomorrow".into();

        let reply = VerbClassifier::classify(question);
        assert!(reply.bundle == InboxBundle::reply() && reply.needs_action);
        let read = VerbClassifier::classify(news);
        assert!(read.bundle == InboxBundle::read() && !read.needs_action);
        assert_eq!(VerbClassifier::classify(unsure).bundle, InboxBundle::reply());
    }
}

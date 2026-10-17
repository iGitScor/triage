use crate::badge;
use std::sync::LazyLock;

use regex::Regex;

use crate::{InboxBundle, InboxItem};

/// What a chat message asks of you.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextIntent {
    Request,
    Info,
    /// A thank-you, an OK, a 👍: the conversation is over, nobody waits for anything.
    Closing,
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
static CLOSER_WORDS: LazyLock<Regex> =
    LazyLock::new(|| word_pattern(&with_apostrophes(KeywordIntentClassifier::CLOSERS)));
/// "merci de" or "merci d'" and the next words (Swift's `merciDe`).
static MERCI_DE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(^|[^\p{L}\p{N}])merci (de\s+|d['’])((\p{L}+['’]?\s*){1,3})").expect("valid pattern")
});
/// Emoji codes (`:+1:`), which say no words.
static EMOJI_CODES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[:][a-z0-9_+'-]+[:]").expect("valid pattern"));

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

    /// Ends a conversation when a short message says little else: "thanks!", "sounds good", "merci, parfait".
    /// After requests and explicit information, which win. Same list as macOS.
    const CLOSERS: &'static [&'static str] = &[
        "thanks",
        "thank you",
        "thx",
        "ty",
        "tysm",
        "cheers",
        "great",
        "perfect",
        "awesome",
        "sounds good",
        "sgtm",
        "got it",
        "ok",
        "okay",
        "kk",
        "noted",
        "will do",
        "all good",
        "no problem",
        "np",
        "cool",
        "nice",
        "lgtm",
        "ack",
        "roger",
        "appreciated",
        "merci",
        "merci beaucoup",
        "parfait",
        "super",
        "top",
        "ça marche",
        "ca marche",
        "d'accord",
        "dac",
        "entendu",
        "noté",
        "bien reçu",
        "c'est noté",
        "nickel",
        "génial",
        "impeccable",
        "pas de souci",
        "pas de soucis",
        "ça roule",
        "bien vu",
        "vu",
    ];
    /// The most words a closing message has: "Thanks a lot, that's perfect for me" closes, a paragraph doesn't.
    const CLOSING_WORDS: usize = 12;

    pub fn intent(text: &str) -> Option<TextIntent> {
        let lowercased = text.to_lowercase();
        let text = NOT_PROSE.replace_all(&lowercased, " ");
        // Nothing but code or links (a snippet, a URL pasted alone) asks nothing in words: to read.
        if text.trim().is_empty() {
            return (!lowercased.trim().is_empty()).then_some(TextIntent::Info);
        }
        let says_no_reply = NO_REPLY_WORDS.is_match(&text);
        let text = NO_REPLY_WORDS.replace_all(&text, "$1 $3");
        let text = NOT_URGENT_WORDS.replace_all(&text, "$1 $3");
        if QUESTION.is_match(&text) || REQUEST_WORDS.is_match(&text) || Self::asks_with_merci(&text) {
            return Some(TextIntent::Request);
        }
        // Explicit information wins over a closing word: "FYI, done", "nothing to do, thanks" are to read.
        if says_no_reply || INFO_WORDS.is_match(&text) {
            return Some(TextIntent::Info);
        }
        if Self::closes(&text) {
            return Some(TextIntent::Closing);
        }
        None
    }

    /// "Merci de" and "merci d'" before something to do ask for it ("merci de relire", "merci d'accepter", "merci de
    /// bien vouloir signer", "merci de ne pas merger"); before what was given or done they thank ("merci de ton
    /// retour", "merci d'avoir regardé", "merci d'avance"). Something to do is an infinitive, after its pronouns.
    fn asks_with_merci(text: &str) -> bool {
        const SKIPPED: &[&str] = &[
            "me", "m", "te", "t", "nous", "vous", "lui", "leur", "le", "la", "les", "l", "se", "s", "y", "en", "bien",
        ];
        const DETERMINERS: &[&str] = &["votre", "notre", "vos", "nos", "ton", "ta", "tes", "mon", "ma", "mes"];
        MERCI_DE.captures_iter(text).any(|c| {
            let Some(word) = c[3].split(|ch: char| !ch.is_alphabetic()).find(|w| !w.is_empty() && !SKIPPED.contains(w))
            else {
                return false;
            };
            word == "ne"
                || (["er", "ir", "re", "oir"].iter().any(|end| word.ends_with(end))
                    && !["avoir", "être", "etre"].contains(&word)
                    && !DETERMINERS.contains(&word))
        })
    }

    /// A short message with a closing word, or nothing but emoji and punctuation (a lone 👍).
    fn closes(text: &str) -> bool {
        let without_codes = EMOJI_CODES.replace_all(text, " ");
        let words = without_codes
            .split(|c: char| !c.is_alphanumeric() && !matches!(c, '\'' | '’' | '-'))
            .filter(|w| !w.is_empty())
            .count();
        if words == 0 {
            return !text.trim().is_empty();
        }
        words <= Self::CLOSING_WORDS && CLOSER_WORDS.is_match(text)
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
                item.bundle = match intent {
                    TextIntent::Request => InboxBundle::reply(),
                    TextIntent::Closing => InboxBundle::finished(),
                    TextIntent::Info => InboxBundle::read(),
                };
                item.needs_action = intent == TextIntent::Request;
            }
            "chat.asked" | "chat.answered" => {
                // Your own message, last in the conversation: a question or request waits on the others. Anything
                // else waits on nobody: an answer of yours finishes the conversation (Done), and a message nobody
                // asked for (an FYI) is cleared. Unsure means not a question, so Waiting doesn't fill with chatter.
                let answered = item.bundle == InboxBundle::answered();
                let text = [Some(item.title.as_str()), item.preview.as_deref()]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join("\n");
                let intent = KeywordIntentClassifier::intent(&text);
                item.needs_action = false;
                if intent == Some(TextIntent::Request) {
                    item.bundle = InboxBundle::awaiting();
                } else if answered {
                    item.bundle = InboxBundle::finished();
                } else {
                    item.bundle = InboxBundle::awaiting();
                    item.expires = Some(item.date);
                }
            }
            _ => {}
        }
        item
    }

    fn verb_for_authored(item: &InboxItem) -> InboxBundle {
        if item.has_badge(badge::DRAFT) {
            InboxBundle::awaiting()
        } else if item.has_badge(badge::CHANGES)
            || item.has_badge(badge::CHECKS_FAILING)
            || item.has_badge(badge::CONFLICTS)
        {
            InboxBundle::fix()
        } else if item.has_badge(badge::APPROVED) {
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

    /// "Merci de" before something to do asks; before what was given or done it thanks. Same cases as the Mac.
    #[test]
    fn merci_de_asks_only_before_something_to_do() {
        let cases = [
            ("Invitation envoyée pour la rétro, merci d'accepter", Some(TextIntent::Request)),
            ("Merci de relire avant midi", Some(TextIntent::Request)),
            ("Merci de me renvoyer le fichier", Some(TextIntent::Request)),
            ("Merci de bien vouloir signer le contrat", Some(TextIntent::Request)),
            ("Merci de ne pas merger avant demain", Some(TextIntent::Request)),
            ("Merci de la relire", Some(TextIntent::Request)),
            ("Merci de ton retour", Some(TextIntent::Closing)),
            ("Merci de votre aide !", Some(TextIntent::Closing)),
            ("Merci d'avoir regardé si vite", Some(TextIntent::Closing)),
            ("Merci d’avance", Some(TextIntent::Closing)),
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

    /// Nothing but code or a link is to read, not to reply. Same cases as the Mac.
    #[test]
    fn code_and_links_alone_are_to_read() {
        for (text, expected) in [
            ("```\nlet total = items.count\n```", Some(TextIntent::Info)),
            ("https://example.com/runbook", Some(TextIntent::Info)),
            ("`make check`", Some(TextIntent::Info)),
            ("   ", None),
            ("https://example.com/runbook can you check?", Some(TextIntent::Request)),
        ] {
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

/// When a chat conversation needs nothing more from you. Same cases as macOS (`ConversationTests`).
#[cfg(test)]
mod conversation_tests {
    use super::*;
    use crate::fixtures::{item, now};
    use crate::{InboxAssembler, ItemState, Placement};

    #[test]
    fn closing_messages() {
        let cases = [
            ("Thanks!", TextIntent::Closing),
            ("thank you so much", TextIntent::Closing),
            ("Merci, parfait", TextIntent::Closing),
            ("Sounds good 👍", TextIntent::Closing),
            ("👍", TextIntent::Closing),
            (":+1:", TextIntent::Closing),
            ("Ok merci !", TextIntent::Closing),
            ("ça marche, à demain", TextIntent::Closing),
            ("Thanks, can you also send the slides?", TextIntent::Request),
            ("FYI, done", TextIntent::Info),
            ("Rien à faire de ton côté, merci !", TextIntent::Info),
        ];
        for (text, expected) in cases {
            assert_eq!(KeywordIntentClassifier::intent(text), Some(expected), "{text}");
        }
        let long = "Thanks for the update, the numbers for the third quarter are in the shared folder with the notes";
        assert_ne!(KeywordIntentClassifier::intent(long), Some(TextIntent::Closing));
    }

    fn classified(title: &str, bundle: InboxBundle) -> InboxItem {
        let mut item = item("c", bundle);
        item.title = title.into();
        item.needs_action = true;
        VerbClassifier::classify(item)
    }

    #[test]
    fn a_closing_message_finishes_the_conversation() {
        let thanks = classified("Thanks, that worked!", InboxBundle::direct_messages());
        assert!(thanks.bundle == InboxBundle::finished() && !thanks.needs_action);
        assert_eq!(classified("Can you look at this?", InboxBundle::mentions()).bundle, InboxBundle::reply());
    }

    /// Your answer, last: a question waits on them; anything else finishes the conversation. A message nobody
    /// asked for (asked) is cleared instead.
    #[test]
    fn your_answer_waits_only_when_it_asks() {
        let question = classified("Which branch should I use?", InboxBundle::answered());
        assert!(question.bundle == InboxBundle::awaiting() && !question.needs_action);
        let done = classified("Here it is", InboxBundle::answered());
        assert!(done.bundle == InboxBundle::finished() && done.expires.is_none());
        assert!(classified("Here it is", InboxBundle::asked()).expires.is_some());
    }

    /// A finished conversation is in Done until someone writes again; a pin keeps it where it is.
    #[test]
    fn a_finished_conversation_is_done() {
        let assembler = InboxAssembler { wake_on_activity: true };
        let finished = item("f", InboxBundle::finished());
        assert_eq!(assembler.placement(&finished, None, now()), Placement::Done);
        let pinned = ItemState { pinned: true, ..Default::default() };
        assert_eq!(assembler.placement(&finished, Some(&pinned), now()), Placement::Inbox);
        let states = std::collections::HashMap::from([("f".to_string(), pinned)]);
        let layout = assembler.layout(&[finished], &states, now(), "");
        assert!(layout.pinned.len() == 1 && layout.action_count() == 0, "shown where you pinned it, not counted");
    }
}

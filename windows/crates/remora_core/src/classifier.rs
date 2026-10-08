use crate::{InboxBundle, InboxItem};

/// What a chat message asks of you.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextIntent {
    Request,
    Info,
}

/// Explicit wording in English and French. Requests win over FYI ("FYI, can you check?").
pub struct KeywordIntentClassifier;

impl KeywordIntentClassifier {
    const REQUESTS: &'static [&'static str] = &[
        "?", "can you", "could you", "would you", "will you", "please", "pls", "plz", "let me know", "lmk",
        "what do you think", "thoughts", "your opinion", "need you", "waiting for you", "asap", "urgent",
        "peux-tu", "pourrais-tu", "tu peux", "tu pourrais", "pouvez-vous", "pourriez-vous", "vous pouvez",
        "merci de", "stp", "svp", "s'il te plaît", "s’il te plaît", "s'il vous plaît", "est-ce que",
        "qu'en penses", "qu’en penses", "dis-moi", "dites-moi", "ton avis", "votre avis", "besoin de toi",
    ];
    const INFOS: &'static [&'static str] = &[
        "fyi", "for your information", "heads up", "heads-up", "just so you know", "announcement",
        "@here", "@channel", "@everyone", "pour info", "pour information", "pour rappel", "à titre d'info",
        "a titre d'info", "je vous informe", "annonce",
    ];

    pub fn intent(text: &str) -> Option<TextIntent> {
        let text = text.to_lowercase();
        if Self::REQUESTS.iter().any(|k| text.contains(k)) {
            return Some(TextIntent::Request);
        }
        if Self::INFOS.iter().any(|k| text.contains(k)) {
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
                let text = [Some(item.title.as_str()), item.preview.as_deref()].into_iter().flatten().collect::<Vec<_>>().join("\n");
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

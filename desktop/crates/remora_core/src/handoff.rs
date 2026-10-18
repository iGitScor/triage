//! A request handed to an agent the user picks (Claude Code, Codex, Gemini…): a prompt written on the computer, from
//! what Remora already has, copied for the user to paste. No AI and no request: nothing leaves until they paste it.
//! In English, the language every agent reads; the same text on macOS (`Handoff.swift`).
use crate::{badge, InboxBundle, InboxItem};

/// What to ask, by where the item is, then what Remora knows of it. `hidden`: the tool's content is hidden (Settings,
/// or the organization's policy), so the title and the message stay out, as for the `remora` command.
pub fn handoff_prompt(item: &InboxItem, source: &str, hidden: bool) -> String {
    let mut lines = vec![ask(&item.bundle, source), String::new()];
    if !hidden {
        lines.push(format!("Item: {}", item.title));
    }
    lines.push(format!("Where: {source} · {}", item.context));
    if let Some(author) = &item.author {
        lines.push(format!("From: {}", author.name));
    }
    if let Some(url) = &item.url {
        lines.push(format!("Link: {url}"));
    }
    let state = states(item);
    if !state.is_empty() {
        lines.push(format!("State: {}", state.join("; ")));
    }
    if hidden {
        lines.extend([String::new(), "Its content is hidden in Remora: open the link to read it.".into()]);
    } else if let Some(preview) = item.preview.as_deref().filter(|p| !p.is_empty()) {
        lines.extend([String::new(), "Message:".into(), preview.into()]);
    }
    lines.join("\n")
}

fn ask(bundle: &InboxBundle, source: &str) -> String {
    let text = if *bundle == InboxBundle::reply() {
        "Draft a reply to this message for me. Don't send it: I'll read it first."
    } else if *bundle == InboxBundle::reviews() {
        "Review this pull request and list the comments you would make. Don't post them: I'll read them first."
    } else if *bundle == InboxBundle::fix() {
        "Find out what is wrong here and propose a fix. Don't push or post anything without asking me."
    } else if *bundle == InboxBundle::merge() {
        "Check whether this pull request is ready to merge and tell me what is left. Don't merge it."
    } else if *bundle == InboxBundle::tasks() {
        return format!(
            "Help me get this done: say what it needs and prepare what you can. Don't change anything in {source} \
             without asking me."
        );
    } else if *bundle == InboxBundle::awaiting() {
        "Help me move this forward: draft a short, friendly follow-up. Don't send it."
    } else {
        "Read this and tell me what, if anything, I need to do."
    };
    text.into()
}

/// The badges an agent needs to know, in words.
fn states(item: &InboxItem) -> Vec<&'static str> {
    [
        (badge::CHECKS_FAILING, "checks are failing"),
        (badge::CHANGES, "changes were requested"),
        (badge::CONFLICTS, "it has merge conflicts"),
        (badge::DRAFT, "it is a draft"),
        (badge::APPROVED, "it is approved"),
        (badge::OVERDUE, "it is overdue"),
        (badge::DUE_TODAY, "it is due today"),
        (badge::PRIORITY_URGENT, "urgent"),
        (badge::PRIORITY_HIGH, "high priority"),
    ]
    .into_iter()
    .filter(|(id, _)| item.has_badge(id))
    .map(|(_, words)| words)
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Badge, Person, Tone};
    use chrono::{TimeZone, Utc};

    fn item(id: &str, plugin: &str, bundle: InboxBundle, title: &str, context: &str) -> InboxItem {
        InboxItem {
            id: id.into(),
            account_id: "a".into(),
            plugin_id: plugin.into(),
            bundle,
            title: title.into(),
            context: context.into(),
            preview: None,
            url: None,
            app_url: None,
            author: None,
            participants: vec![],
            badges: vec![],
            date: Utc.timestamp_opt(1_800_000_000, 0).unwrap(),
            needs_action: true,
            priority: None,
            due: None,
            expires: None,
            changes: None,
            suggested_people: None,
            recurrence: None,
            reactions: None,
            reason: None,
        }
    }

    /// A pull request to fix: what to ask, where, who, the link, its state in words, and the message. As macOS.
    #[test]
    fn a_prompt_says_what_to_do_and_what_remora_knows() {
        let mut pr = item("1", "github", InboxBundle::fix(), "Speed up search", "acme/app #12");
        pr.preview = Some("The index is rebuilt on every keystroke.".into());
        pr.url = Some("https://github.com/acme/app/pull/12".into());
        pr.author = Some(Person { name: "frank".into(), avatar_url: None, review: None, bot: false });
        pr.badges = vec![
            Badge::new(badge::COMMENTS, "3", Tone::Neutral),
            Badge::new(badge::CHECKS_FAILING, "Checks failed", Tone::Negative),
            Badge::new(badge::CONFLICTS, "Conflicts", Tone::Warning),
        ];
        assert_eq!(
            handoff_prompt(&pr, "GitHub", false),
            "Find out what is wrong here and propose a fix. Don't push or post anything without asking me.\n\n\
             Item: Speed up search\nWhere: GitHub · acme/app #12\nFrom: frank\n\
             Link: https://github.com/acme/app/pull/12\nState: checks are failing; it has merge conflicts\n\n\
             Message:\nThe index is rebuilt on every keystroke."
        );
    }

    /// A tool whose content is hidden: where and the link only, never the title or the message.
    #[test]
    fn hidden_content_stays_out() {
        let mut message = item("2", "slack", InboxBundle::reply(), "Can you send the quote?", "#sales");
        message.preview = Some("Can you send the quote?".into());
        message.url = Some("https://acme.slack.com/x".into());
        assert_eq!(
            handoff_prompt(&message, "Slack", true),
            "Draft a reply to this message for me. Don't send it: I'll read it first.\n\n\
             Where: Slack · #sales\nLink: https://acme.slack.com/x\n\n\
             Its content is hidden in Remora: open the link to read it."
        );
    }

    #[test]
    fn each_tab_asks_for_its_own_help() {
        assert!(ask(&InboxBundle::reviews(), "GitLab").starts_with("Review this pull request"));
        assert!(ask(&InboxBundle::tasks(), "Linear").contains("Don't change anything in Linear"));
        assert_eq!(ask(&InboxBundle::mentions(), "Slack"), "Read this and tell me what, if anything, I need to do.");
    }
}

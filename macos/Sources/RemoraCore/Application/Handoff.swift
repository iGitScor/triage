import Foundation

/// A request handed to an agent the user picks (Claude Code, Codex, Gemini…): a prompt written on the computer, from
/// what Remora already has, copied for the user to paste. No AI and no request: nothing leaves until they paste it.
/// In English, the language every agent reads; the same text on Windows (`handoff.rs`).
public enum Handoff {
    /// What to ask, by where the item is, then what Remora knows of it. `hidden`: the tool's content is hidden
    /// (Settings, or the organization's policy), so the title and the message stay out, as for the `remora` command.
    public static func prompt(for item: InboxItem, source: String, hidden: Bool) -> String {
        var lines = [ask(item.bundle, source: source), ""]
        if !hidden { lines.append("Item: \(item.title)") }
        lines.append("Where: \(source) · \(item.context)")
        if let author = item.author?.name { lines.append("From: \(author)") }
        if let url = item.url { lines.append("Link: \(url.absoluteString)") }
        let state = states(item.badges.map(\.id))
        if !state.isEmpty { lines.append("State: \(state.joined(separator: "; "))") }
        if hidden {
            lines += ["", "Its content is hidden in Remora: open the link to read it."]
        } else if let preview = item.preview, !preview.isEmpty {
            lines += ["", "Message:", preview]
        }
        return lines.joined(separator: "\n")
    }

    static func ask(_ bundle: InboxBundle, source: String) -> String {
        switch bundle {
        case .reply: "Draft a reply to this message for me. Don't send it: I'll read it first."
        case .reviews:
            "Review this pull request and list the comments you would make. Don't post them: I'll read them first."
        case .fix: "Find out what is wrong here and propose a fix. Don't push or post anything without asking me."
        case .merge: "Check whether this pull request is ready to merge and tell me what is left. Don't merge it."
        case .tasks:
            "Help me get this done: say what it needs and prepare what you can. Don't change anything in \(source) without asking me."
        case .awaiting: "Help me move this forward: draft a short, friendly follow-up. Don't send it."
        default: "Read this and tell me what, if anything, I need to do."
        }
    }

    /// The badges an agent needs to know, in words.
    static func states(_ badges: [BadgeID]) -> [String] {
        let words: [(BadgeID, String)] = [
            (.checksFailing, "checks are failing"), (.changes, "changes were requested"),
            (.conflicts, "it has merge conflicts"), (.draft, "it is a draft"), (.approved, "it is approved"),
            (.overdue, "it is overdue"), (.dueToday, "it is due today"), (.priorityUrgent, "urgent"),
            (.priorityHigh, "high priority"),
        ]
        return words.filter { badges.contains($0.0) }.map(\.1)
    }
}

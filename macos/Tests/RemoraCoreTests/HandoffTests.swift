import Foundation
import Testing

@testable import RemoraCore

/// The same cases as Windows and Linux (`handoff.rs`).
struct HandoffTests {
    let date = Date(timeIntervalSince1970: 1_800_000_000)

    /// A pull request to fix: what to ask, where, who, the link, its state in words, and the message.
    @Test func aPromptSaysWhatToDoAndWhatRemoraKnows() {
        let item = InboxItem(
            id: "1", accountID: UUID(), pluginID: "github", bundle: .fix, title: "Speed up search",
            context: "acme/app #12", preview: "The index is rebuilt on every keystroke.",
            url: URL(string: "https://github.com/acme/app/pull/12"), author: Person(name: "frank"),
            badges: [
                Badge(id: .comments, label: "3", tone: .neutral),
                Badge(id: .checksFailing, label: "Checks failed", tone: .negative),
                Badge(id: .conflicts, label: "Conflicts", tone: .warning),
            ], date: date)
        #expect(
            Handoff.prompt(for: item, source: "GitHub", hidden: false) == """
                Find out what is wrong here and propose a fix. Don't push or post anything without asking me.

                Item: Speed up search
                Where: GitHub · acme/app #12
                From: frank
                Link: https://github.com/acme/app/pull/12
                State: checks are failing; it has merge conflicts

                Message:
                The index is rebuilt on every keystroke.
                """)
    }

    /// A tool whose content is hidden: where and the link only, never the title or the message.
    @Test func hiddenContentStaysOut() {
        let item = InboxItem(
            id: "2", accountID: UUID(), pluginID: "slack", bundle: .reply, title: "Can you send the quote?",
            context: "#sales", preview: "Can you send the quote?", url: URL(string: "https://acme.slack.com/x"),
            date: date)
        #expect(
            Handoff.prompt(for: item, source: "Slack", hidden: true) == """
                Draft a reply to this message for me. Don't send it: I'll read it first.

                Where: Slack · #sales
                Link: https://acme.slack.com/x

                Its content is hidden in Remora: open the link to read it.
                """)
    }

    @Test func eachTabAsksForItsOwnHelp() {
        #expect(Handoff.ask(.reviews, source: "GitLab").hasPrefix("Review this pull request"))
        #expect(Handoff.ask(.tasks, source: "Linear").contains("Don't change anything in Linear"))
        #expect(Handoff.ask(.mentions, source: "Slack") == "Read this and tell me what, if anything, I need to do.")
    }
}

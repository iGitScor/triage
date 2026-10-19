import Foundation
import RemoraCore
import Testing

@testable import RemoraPlugins

/// Where a Linear or Jira ticket stands, and the badge that says why. As Windows.
struct TicketStageTests {
    let account = UUID()
    let decoder = JSONDecoder.api()
    let now = Date(timeIntervalSince1970: 1_800_000_000)

    func linear(_ extra: String, priority: Int = 3) throws -> LinearPlugin.Issue {
        let json = """
            {"id": "i1", "identifier": "ENG-42", "title": "Fix", "updatedAt": "2027-01-15T08:00:00Z",
             "url": "https://linear.app/acme/issue/ENG-42/fix", "priority": \(priority), \(extra)}
            """
        return try decoder.decode(LinearPlugin.Issue.self, from: Data(json.utf8))
    }

    @Test func linearStagesFromTheStatusTypeAndTheCycle() throws {
        let cycle = #""cycle": {"startsAt": "2027-01-10T00:00:00Z", "endsAt": "2027-01-24T00:00:00Z"}"#
        #expect(
            try linear(#""state": {"name": "In Progress", "type": "started"}"#).stage(now: now, discussed: false)
                == .started)
        let planned = try linear(#""state": {"name": "Todo", "type": "unstarted"}, \#(cycle)"#)
        #expect(planned.stage(now: now, discussed: false) == .planned)
        #expect(planned.item(accountID: account, now: now).hasBadge(.cycle))
        #expect(
            try linear(#""state": {"name": "Todo", "type": "unstarted"}"#).stage(now: now, discussed: false) == .todo)
        #expect(
            try linear(#""state": {"name": "Triage", "type": "triage"}"#).stage(now: now, discussed: false) == .triage)
        #expect(try linear(#""state": {"name": "Custom"}"#).stage(now: now, discussed: false) == nil, "no guess")
    }

    @Test func aLinearIssueBlockedByAnOpenOneWaitsOnIt() throws {
        let relations = { (type: String) in
            #""state": {"name": "Todo", "type": "unstarted"}, "inverseRelations": {"nodes": [{"type": "blocks", "issue": {"identifier": "ENG-40", "state": {"type": "\#(type)"}}}, {"type": "related", "issue": {"identifier": "ENG-41"}}]}"#
        }
        let blocked = try linear(relations("started"))
        let item = blocked.item(accountID: account, now: now)
        #expect(blocked.blockers == ["ENG-40"] && item.stage == .blocked && item.hasBadge(.blocked))
        #expect(item.bundle == .blocked && !item.needsAction, "it waits on ENG-40, in Waiting")
        #expect(try linear(relations("completed")).blockers.isEmpty)
    }

    /// A comment waiting for you lifts the issue first in its priority, and a low one is counted again.
    @Test func aLinearIssueSomeoneWroteAboutComesFirst() throws {
        let issues = LinearPlugin.Viewer(
            name: "Alice",
            assignedIssues: .init(nodes: [try linear(#""state": {"name": "Backlog", "type": "backlog"}"#, priority: 4)])
        )
        let comment = try decoder.decode(
            LinearPlugin.Notification.self,
            from: Data(
                #"{"id": "n1", "type": "issueNewComment", "url": "https://linear.app/acme/issue/ENG-42/old#comment-1", "createdAt": "2027-01-15T07:00:00Z"}"#
                    .utf8))
        let items = LinearPlugin.snapshot(viewer: issues, notifications: [comment], accountID: account, now: now).items
        let issue = try #require(items.first { $0.context == "ENG-42" })
        #expect(issue.stage == .discussed && issue.hasBadge(.discussed))
        #expect(issue.priority == .normal && issue.needsAction)
    }

    /// A comment from someone else, never answered nor resolved, lifts the issue even when Linear's notification
    /// was read months ago; your answer, a resolved thread or a bot's comment don't. A duplicate, otherwise last.
    @Test func aLinearCommentLeftUnansweredComesFirst() throws {
        func issue(_ comments: String) throws -> InboxItem {
            try linear(
                #""state": {"name": "Duplicate", "type": "duplicate"}, "comments": {"nodes": [\#(comments)]}"#
            ).item(accountID: account, now: now)
        }
        let theirs = #"{"resolvedAt": null, "user": {"isMe": false}, "botActor": null}"#
        let mine = #"{"resolvedAt": null, "user": {"isMe": true}, "botActor": null}"#
        let bot = #"{"resolvedAt": null, "user": null, "botActor": {"id": "github"}}"#
        let unanswered = try issue(theirs)
        #expect(unanswered.stage == .discussed && unanswered.hasBadge(.discussed) && unanswered.needsAction)
        #expect(try issue(theirs + ", " + bot).stage == .discussed, "a bot after them doesn't answer")
        #expect(try issue(theirs + ", " + mine).stage == .backlog)
        #expect(try issue(#"{"resolvedAt": "2027-01-01T00:00:00Z", "user": {"isMe": false}}"#).stage == .backlog)
        #expect(try issue("").priority == .low, "a duplicate counts as low")
    }

    /// Started, then paused: ranked as not started, and old enough to say so when it matters.
    @Test func aPausedIssueIsNotWorkInProgress() throws {
        let paused = try linear(
            #""state": {"name": "Paused", "type": "started"}, "createdAt": "2026-12-01T00:00:00Z""#, priority: 2)
        #expect(paused.stage(now: now, discussed: false) == .todo)
        #expect(paused.item(accountID: account, now: now).hasBadge(.age))
        #expect(TaskStage.paused("On hold") && TaskStage.paused("En pause") && !TaskStage.paused("In Progress"))
    }

    /// Urgent or high, not started, open for weeks: said on the row, without making Done come back.
    @Test func anUrgentIssueLeftForWeeksSaysSo() throws {
        let old = try linear(
            #""state": {"name": "Todo", "type": "unstarted"}, "createdAt": "2026-12-01T00:00:00Z""#, priority: 2)
        let item = old.item(accountID: account, now: now)
        #expect(item.hasBadge(.age))
        var aged = item
        aged.badges.removeAll { $0.id == .age }
        #expect(aged.fingerprint == item.fingerprint)
        let started = try linear(
            #""state": {"name": "In Progress", "type": "started"}, "createdAt": "2026-12-01T00:00:00Z""#, priority: 2)
        #expect(!started.item(accountID: account, now: now).hasBadge(.age))
    }

    func jira(_ category: String, links: String = "[]") throws -> JiraPlugin.Issue {
        let json = """
            {"id": "1", "key": "CE-7", "fields": {"summary": "Close", "issuelinks": \(links),
             "status": {"name": "Status", "statusCategory": {"key": "\(category)"}}}}
            """
        return try decoder.decode(JiraPlugin.Issue.self, from: Data(json.utf8))
    }

    @Test func jiraStagesFromTheCategoryTheSprintAndTheLinks() throws {
        #expect(try jira("indeterminate").stage(inSprint: false, discussed: false) == .started)
        #expect(try jira("new").stage(inSprint: true, discussed: false) == .planned)
        #expect(try jira("new").stage(inSprint: false, discussed: false) == .todo)
        let blockedBy = { (category: String) in
            #"[{"type": {"name": "Blocks"}, "inwardIssue": {"key": "CE-1", "fields": {"status": {"name": "x", "statusCategory": {"key": "\#(category)"}}}}}, {"type": {"name": "Blocks"}, "outwardIssue": {"key": "CE-9"}}]"#
        }
        #expect(try jira("new", links: blockedBy("indeterminate")).blockers == ["CE-1"])
        #expect(try jira("new", links: blockedBy("done")).blockers.isEmpty)
        let paused =
            #"{"id": "1", "key": "CE-8", "fields": {"status": {"name": "On Hold", "statusCategory": {"key": "indeterminate"}}}}"#
        #expect(
            try decoder.decode(JiraPlugin.Issue.self, from: Data(paused.utf8)).stage(inSprint: false, discussed: false)
                == .todo)
        let host = URL(string: "https://acme.atlassian.net")!
        let discussed = try jira("new").item(host: host, accountID: account, discussed: true)
        #expect(discussed.stage == .discussed && discussed.hasBadge(.discussed))
    }
}

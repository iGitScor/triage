import Foundation
import RemoraCore
import Testing

@testable import RemoraPlugins

/// A ticket whose work is up for review waits on others, tied to its pull requests, in every tracker. As Windows.
struct TicketReviewTests {
    let account = UUID()
    let decoder = JSONDecoder.api()

    func linear(state: String, attachments: String = "") throws -> InboxItem {
        let json = """
            {"id": "i1", "identifier": "ENG-42", "title": "Fix CSV export", "updatedAt": "2026-10-08T10:00:00Z",
             "url": "https://linear.app/acme/issue/ENG-42/fix-csv-export", "state": \(state),
             "attachments": {"nodes": [\(attachments)]}}
            """
        return try decoder.decode(LinearPlugin.Issue.self, from: Data(json.utf8)).item(accountID: account)
    }

    @Test func aLinearIssueWithAnOpenPullRequestWaits() throws {
        let pr = "https://github.com/acme/app/pull/9"
        let open = try linear(
            state: #"{"name": "In Progress", "type": "started"}"#,
            attachments: #"{"url": "\#(pr)", "metadata": {"status": "open"}}, {"url": "https://figma.com/file/x"}"#)
        #expect(open.bundle == .awaiting && !open.needsAction && open.related == [URL(string: pr)!])
        let merged = try linear(
            state: #"{"name": "In Progress", "type": "started"}"#,
            attachments: #"{"url": "\#(pr)", "metadata": {"status": "merged"}}"#)
        #expect(merged.bundle == .tasks && merged.needsAction)
    }

    @Test func aLinearReviewStatusWaitsUnlessNobodyStarted() throws {
        #expect(try linear(state: #"{"name": "In Review", "type": "started"}"#).bundle == .awaiting)
        #expect(try linear(state: #"{"name": "In Review", "type": "backlog"}"#).bundle == .tasks)
    }

    /// The integration's metadata is its own: a shape Remora doesn't expect never fails the fetch.
    @Test func oddLinearMetadataIsIgnored() throws {
        let item = try linear(
            state: #"{"name": "Todo", "type": "unstarted"}"#,
            attachments: #"{"url": "https://github.com/acme/app/pull/9", "metadata": {"status": {"x": 1}}}"#)
        #expect(item.bundle == .awaiting, "an attached pull request is open unless said otherwise")
    }

    func jira(status: String) throws -> JiraPlugin.Issue {
        let json = #"{"id": "1", "key": "CE-7", "fields": {"summary": "Close", "status": {"name": "\#(status)"}}}"#
        return try decoder.decode(JiraPlugin.Issue.self, from: Data(json.utf8))
    }

    @Test func aJiraIssueWithAnOpenPullRequestOrInReviewWaits() throws {
        let host = URL(string: "https://acme.atlassian.net")!
        #expect(try jira(status: "In Progress").item(host: host, accountID: account).bundle == .tasks)
        let open = try jira(status: "In Progress").item(host: host, accountID: account, openChange: true)
        #expect(open.bundle == .awaiting)
        #expect(try jira(status: "Code Review").item(host: host, accountID: account).bundle == .awaiting)
    }

    @Test func aGitHubIssueWithAPullRequestThatClosesItWaits() throws {
        let json = """
            {"id": "I_1", "number": 31, "title": "Crash", "url": "https://github.com/acme/app/issues/31",
             "updatedAt": "2026-10-08T10:00:00Z", "repository": {"nameWithOwner": "acme/app"},
             "closedByPullRequestsReferences": {"nodes": [{"url": "https://github.com/acme/app/pull/32"}]}}
            """
        let item = try decoder.decode(AssignedIssue.self, from: Data(json.utf8)).item(accountID: account)
        #expect(item.bundle == .awaiting && item.related == [URL(string: "https://github.com/acme/app/pull/32")!])
    }

    @Test func anAzureWorkItemWaitsOnItsActivePullRequests() throws {
        let json = """
            {"id": 42, "fields": {"System.Title": "Export", "System.State": "Active", "System.TeamProject": "Acme"},
             "relations": [{"rel": "ArtifactLink", "url": "vstfs:///Git/PullRequestId/P1%2FR1%2F12"},
                           {"rel": "System.LinkTypes.Hierarchy-Reverse", "url": "https://dev.azure.com/x/_apis/wit/workItems/1"}]}
            """
        let work = try decoder.decode(AzureDevOpsPlugin.WorkItem.self, from: Data(json.utf8))
        #expect(work.pullRequests == ["vstfs:///git/pullrequestid/p1%2fr1%2f12"])
        let host = URL(string: "https://dev.azure.com/acme")!
        let pr = URL(string: "https://dev.azure.com/acme/Acme/_git/app/pullrequest/12")!
        #expect(work.item(host: host, accountID: account, openChanges: [pr]).related == [pr])
        #expect(work.item(host: host, accountID: account).bundle == .tasks, "a completed pull request isn't listed")
    }

    @Test func aNotionTaskInReviewWaits() throws {
        let json = """
            {"id": "p", "last_edited_time": "2026-10-08T10:00:00.000Z", "properties": {
              "Name": {"type": "title", "title": [{"plain_text": "Ship it"}]},
              "Status": {"type": "status", "status": {"name": "In review"}}}}
            """
        let page = try JSONDecoder.api(snakeCase: true).decode(NotionPlugin.Page.self, from: Data(json.utf8))
        #expect(page.item(accountID: account, database: "Tasks").bundle == .awaiting)
    }
}

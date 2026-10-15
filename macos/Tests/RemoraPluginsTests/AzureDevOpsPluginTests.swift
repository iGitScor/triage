import Foundation
import RemoraCore
import Testing

@testable import RemoraPlugins

/// The same fixtures as Windows (`azuredevops.rs`).
enum AzureDevOpsFixtures {
    static let me = #"{"authenticatedUser": {"id": "a1", "providerDisplayName": "Alice Martin"}}"#
    /// Both lists answer the same: the plugin tells yours from those you review by who created them.
    static let pulls = """
        {"count": 3, "value": [
          {"pullRequestId": 12, "codeReviewId": 12, "title": "Speed up *CI*", "status": "active", "isDraft": false,
           "creationDate": "2026-10-08T10:00:00.1234567Z", "mergeStatus": "succeeded",
           "createdBy": {"id": "a1", "displayName": "Alice Martin"},
           "repository": {"id": "r1", "name": "app", "project": {"id": "p1", "name": "Acme"}},
           "reviewers": [{"id": "e1", "displayName": "Erin", "vote": 10}, {"id": "f1", "displayName": "Frank", "vote": 0}]},
          {"pullRequestId": 13, "codeReviewId": 13, "title": "Add avatars", "status": "active",
           "creationDate": "2026-10-09T08:00:00Z", "mergeStatus": "conflicts",
           "createdBy": {"id": "b1", "displayName": "Bob"},
           "repository": {"id": "r1", "name": "app", "project": {"id": "p1", "name": "Acme"}},
           "reviewers": [{"id": "a1", "displayName": "Alice Martin", "vote": 0, "isRequired": true},
                         {"id": "e1", "displayName": "Erin", "vote": -5}]},
          {"pullRequestId": 14, "codeReviewId": 14, "title": "Old cleanup", "status": "active",
           "creationDate": "2026-10-01T08:00:00Z", "createdBy": {"id": "c1", "displayName": "Carol"},
           "repository": {"id": "r2", "name": "Acme", "project": {"id": "p1", "name": "Acme"}},
           "reviewers": [{"id": "a1", "displayName": "Alice Martin", "vote": 10}]}
        ]}
        """
    static let evaluations = """
        {"value": [
          {"status": "approved", "configuration": {"isEnabled": true, "type": {"id": "fa4e907d-c16b-4a4c-9dfa-4916e5d171ab"}}},
          {"status": "rejected", "configuration": {"isEnabled": true, "type": {"id": "0609b952-1397-4640-95ec-e00a01b2c241"}}}
        ]}
        """
    static let iterations = """
        {"value": [{"id": 1, "createdDate": "2026-10-09T08:00:00Z"},
                   {"id": 2, "createdDate": "2026-10-10T07:00:00Z", "updatedDate": "2026-10-10T07:00:00Z"}]}
        """
    static let changes = """
        {"changeEntries": [{"item": {"path": "/src/db/add_avatars.sql"}, "changeType": "add"},
                           {"item": {"path": "/src", "isFolder": true}, "changeType": "edit"}]}
        """
    static let wiql = #"{"workItems": [{"id": 42}, {"id": 43}]}"#
    static let workItems = """
        {"count": 2, "value": [
          {"id": 43, "fields": {"System.Title": "Tidy the logs", "System.State": "New", "System.WorkItemType": "Task",
            "System.TeamProject": "Acme", "System.ChangedDate": "2026-10-11T09:00:00Z", "Microsoft.VSTS.Common.Priority": 4}},
          {"id": 42, "fields": {"System.Title": "Export *CSV*", "System.State": "Active", "System.WorkItemType": "Bug",
            "System.TeamProject": "Acme", "System.ChangedDate": "2026-10-12T09:00:00.12Z", "Microsoft.VSTS.Common.Priority": 1,
            "Microsoft.VSTS.Scheduling.DueDate": "2020-01-01T00:00:00Z", "System.Description": "<p>Not decoded</p>"}}
        ]}
        """

    static let routes = [
        "/_apis/connectionData": me,
        "/_apis/git/pullrequests": pulls,
        "/_apis/policy/evaluations": evaluations,
        "/pullRequests/13/iterations": iterations,
        "/pullRequests/13/iterations/2/changes": changes,
        "/_apis/wit/wiql": wiql,
        "/_apis/wit/workitems": workItems,
    ]
}

/// Checks the token goes as Basic's password, then answers like `StubHTTP`.
struct AzureDevOpsStub: HTTPClient {
    var routes: [String: String]
    var failing: String?

    func send(_ request: URLRequest) async throws -> (Data, HTTPURLResponse) {
        #expect(request.value(forHTTPHeaderField: "Authorization") == "Basic OnQ=")
        #expect(request.value(forHTTPHeaderField: "X-TFS-FedAuthRedirect") == "Suppress")
        if let failing, request.url!.path.hasSuffix(failing) {
            return (Data(), HTTPURLResponse(url: request.url!, statusCode: 429, httpVersion: nil, headerFields: nil)!)
        }
        return try await StubHTTP(routes: routes).send(request)
    }
}

struct AzureDevOpsPluginTests {
    func plugin(_ http: HTTPClient) throws -> AzureDevOpsPlugin {
        try AzureDevOpsPlugin(config: config(["host": "dev.azure.com/acme/", "token": "t"]), http: http)
    }

    @Test func pullRequestsWithVotesChecksAndFilesThenWorkItems() async throws {
        let snapshot = try await plugin(AzureDevOpsStub(routes: AzureDevOpsFixtures.routes)).fetch()

        #expect(snapshot.identity == "Alice Martin")
        #expect(snapshot.remarks.isEmpty)
        #expect(snapshot.items.map(\.bundle) == [.authored, .reviews, .tasks, .tasks], "already voted on: left out")

        let mine = snapshot.items[0]
        #expect(mine.title == "Speed up CI")
        #expect(mine.context == "Acme/app !12")
        #expect(mine.url?.absoluteString == "https://dev.azure.com/acme/Acme/_git/app/pullrequest/12")
        #expect(mine.hasBadge("approved") && mine.hasBadge("checks.failing") && mine.needsAction)
        #expect(mine.participants.first { $0.name == "Erin" }?.tone == .accent)
        #expect(mine.participants.first { $0.name == "Frank" }?.tone == nil)
        #expect(mine.changes == nil, "your own pull requests don't need a review prep")

        let review = snapshot.items[1]
        #expect(review.author?.name == "Bob")
        #expect(review.hasBadge("changes") && review.hasBadge("conflicts") && !review.hasBadge("approved"))
        #expect(review.participants.first { $0.name == "Erin" }?.tone == .negative)
        #expect(review.changes?.files == [ChangedFile(path: "src/db/add_avatars.sql")])
        #expect(review.date == Date(iso8601: "2026-10-10T07:00:00Z"), "the latest push")

        let bug = snapshot.items[2]
        #expect(bug.title == "Export CSV")
        #expect(bug.context == "Acme · Bug 42")
        #expect(bug.url?.absoluteString == "https://dev.azure.com/acme/Acme/_workitems/edit/42")
        #expect(bug.priority == .high && bug.hasBadge("priority.high") && bug.hasBadge("overdue"))
        #expect(bug.hasBadge("status") && bug.needsAction)
        let task = snapshot.items[3]
        #expect(task.priority == .low && !task.needsAction)
    }

    /// Checks or work items that can't be read are said; a rate limit fails the fetch, so the backoff waits it out.
    @Test func failedPartsAreSaidAndARateLimitIsWaitedOut() async throws {
        var routes = AzureDevOpsFixtures.routes
        routes["/_apis/policy/evaluations"] = nil
        routes["/_apis/wit/wiql"] = nil
        let snapshot = try await plugin(AzureDevOpsStub(routes: routes)).fetch()
        #expect(snapshot.items.count == 2, "the pull requests still come")
        #expect(snapshot.remarks.count == 2)
        #expect(snapshot.remarks.first?.hasPrefix("Policy checks or changed files couldn’t be read") == true)
        #expect(snapshot.remarks.last?.hasPrefix("Work items couldn’t be read") == true)

        let limited = try plugin(AzureDevOpsStub(routes: AzureDevOpsFixtures.routes, failing: "/iterations"))
        await #expect(throws: HTTPError.rateLimited(nil)) { try await limited.fetch() }
    }

    @Test func buildAndStatusPoliciesAreTheChecks() {
        func evaluation(_ status: String, _ type: String = "0609b952-1397-4640-95ec-e00a01b2c241")
            -> AzureDevOpsPlugin.Evaluation
        {
            .init(status: status, configuration: .init(isEnabled: true, type: .init(id: type)))
        }
        let reviewers = "fa4e907d-c16b-4a4c-9dfa-4916e5d171ab"
        #expect(AzureDevOpsPlugin.checks([]) == .none)
        #expect(AzureDevOpsPlugin.checks([evaluation("rejected", reviewers)]) == .none, "votes aren't checks")
        #expect(AzureDevOpsPlugin.checks([evaluation("approved"), evaluation("running")]) == .running)
        #expect(AzureDevOpsPlugin.checks([evaluation("approved"), evaluation("notApplicable")]) == .passing)
        #expect(AzureDevOpsPlugin.checks([evaluation("queued"), evaluation("broken")]) == .failing)
    }
}

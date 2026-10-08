import Foundation
import Testing
import RemoraCore
@testable import RemoraPlugins

/// Answers Linear's two queries by looking at the request body.
struct LinearStub: HTTPClient {
    var issues: String
    var notifications: String

    func send(_ request: URLRequest) async throws -> (Data, HTTPURLResponse) {
        let body = String(data: request.httpBody ?? Data(), encoding: .utf8) ?? ""
        let response = body.contains("notifications") ? notifications : issues
        #expect(request.value(forHTTPHeaderField: "Authorization") == "lin_api_key", "personal keys go without Bearer")
        return (Data(response.utf8), HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: nil)!)
    }
}

struct LinearPluginTests {
    let now = Date()
    var recent: String { now.addingTimeInterval(-3_600).ISO8601Format() }
    var old: String { now.addingTimeInterval(-10 * 86_400).ISO8601Format() }

    var issues: String {
        """
        {"data": {"viewer": {"name": "Alice", "assignedIssues": {"nodes": [
          {"id": "i1", "identifier": "ENG-42", "title": "Fix CSV export", "url": "https://linear.app/x/issue/ENG-42",
           "priority": 1, "dueDate": "2020-01-01", "updatedAt": "\(recent)", "state": {"name": "In Progress"}},
          {"id": "i2", "identifier": "ENG-43", "title": "Polish onboarding", "priority": 4, "updatedAt": "\(recent)"},
          {"id": "i3", "identifier": "ENG-44", "title": "Upgrade axios", "priority": 3, "updatedAt": "\(recent)", "state": {"name": "Backlog", "type": "backlog"}},
          {"id": "i4", "identifier": "ENG-45", "title": "Patch CVE", "priority": 2, "updatedAt": "\(recent)", "state": {"name": "Backlog", "type": "backlog"}}
        ]}}}}
        """
    }

    @Test func issuesAndActionableNotifications() async throws {
        let notifications = """
        {"data": {"notifications": {"nodes": [
          {"id": "n1", "type": "issueCommentMention", "title": "Erin mentioned you in ENG-40", "url": "https://linear.app/x", "readAt": null, "createdAt": "\(recent)", "actor": {"name": "Erin"}},
          {"id": "n2", "type": "issueAssignedToYou", "title": "Assigned", "readAt": null, "createdAt": "\(recent)"},
          {"id": "n3", "type": "issueNewComment", "title": "Frank commented", "readAt": "\(recent)", "createdAt": "\(recent)"},
          {"id": "n4", "type": "issueMention", "title": "Old mention", "readAt": null, "createdAt": "\(old)"}
        ]}}}
        """
        let plugin = try LinearPlugin(config: config(["token": "lin_api_key"]), http: LinearStub(issues: issues, notifications: notifications))
        let snapshot = try await plugin.fetch()

        #expect(snapshot.identity == "Alice")
        let tasks = snapshot.items.filter { $0.bundle == .tasks }
        #expect(tasks.map(\.context) == ["ENG-42", "ENG-43", "ENG-44", "ENG-45"])
        #expect(tasks.map(\.priority) == [.urgent, .low, .low, .high], "backlog is low unless urgent or high")
        #expect(tasks.map(\.needsAction) == [true, false, false, true])
        #expect(tasks[0].due != nil)
        #expect(tasks[0].appURL?.absoluteString == "linear://x/issue/ENG-42")
        #expect(tasks[0].hasBadge("priority.urgent") && tasks[0].hasBadge("overdue") && tasks[0].hasBadge("status"))
        let mentions = snapshot.items.filter { $0.bundle == .mentions }
        #expect(mentions.map(\.title) == ["Erin mentioned you in ENG-40"], "unread, recent, mentions or comments only")
        #expect(mentions.first?.author?.name == "Erin")
    }

    @Test func issuesSurviveANotificationsSchemaChange() async throws {
        let broken = #"{"errors": [{"message": "Cannot query field \"title\" on type \"Notification\"."}]}"#
        let plugin = try LinearPlugin(config: config(["token": "lin_api_key"]), http: LinearStub(issues: issues, notifications: broken))
        let snapshot = try await plugin.fetch()
        #expect(snapshot.items.count == 4)
    }

    @Test func rejectedKeyIsReported() async throws {
        let rejected = #"{"errors": [{"message": "Authentication required"}]}"#
        let plugin = try LinearPlugin(config: config(["token": "lin_api_key"]), http: LinearStub(issues: rejected, notifications: rejected))
        await #expect(throws: HTTPError.api("Linear: Authentication required")) { try await plugin.fetch() }
    }
}

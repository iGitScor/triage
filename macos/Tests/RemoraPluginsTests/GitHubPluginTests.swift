import Foundation
import Testing
import RemoraCore
@testable import RemoraPlugins

struct GitHubPluginTests {
    let response = """
    {"data": {
      "viewer": {"login": "alice"},
      "authored": {"nodes": [{
        "id": "PR_1", "number": 7, "title": "Add inbox", "url": "https://github.com/acme/app/pull/7",
        "isDraft": false, "updatedAt": "2026-10-08T10:00:00Z", "additions": 10, "deletions": 2,
        "mergeable": "MERGEABLE", "reviewDecision": "APPROVED",
        "repository": {"nameWithOwner": "acme/app"},
        "author": {"login": "alice", "avatarUrl": "https://avatars/alice"},
        "comments": {"totalCount": 3},
        "latestOpinionatedReviews": {"nodes": [{"state": "APPROVED", "author": {"login": "erin", "avatarUrl": null}}]},
        "reviewRequests": {"nodes": [{"requestedReviewer": {"name": "platform-team"}}]},
        "commits": {"nodes": [{"commit": {"statusCheckRollup": {"state": "FAILURE"}}}]}
      }]},
      "reviewing": {"nodes": [{
        "id": "PR_2", "number": 9, "title": "Fix login", "url": "https://github.com/acme/app/pull/9",
        "isDraft": false, "updatedAt": "2026-10-08T09:00:00Z", "mergeable": "CONFLICTING",
        "repository": {"nameWithOwner": "acme/app"}, "author": {"login": "frank"},
        "changedFiles": 3,
        "files": {"nodes": [{"path": "src/search/index.ts", "additions": 40, "deletions": 2}, {"path": "src/search/index.test.ts", "additions": 12, "deletions": 0}]}
      }]}
    }}
    """

    /// A list past its 50 says so; one that fits doesn't.
    @Test func saysWhenAListIsCut() async throws {
        let plugin = { (json: String) in try GitHubPlugin(config: config(["host": "github.com", "token": "t"]), http: StubHTTP(routes: ["/graphql": json])) }
        #expect(try await plugin(response).fetch().remarks.isEmpty, "no count: nothing to say")
        let fits = response.replacingOccurrences(of: #""reviewing": {"nodes""#, with: #""reviewing": {"issueCount": 1, "nodes""#)
        #expect(try await plugin(fits).fetch().remarks.isEmpty)
        let cut = response.replacingOccurrences(of: #""reviewing": {"nodes""#, with: #""reviewing": {"issueCount": 73, "nodes""#)
        #expect(try await plugin(cut).fetch().remarks == [SourceSnapshot.truncated("GitHub")])
    }

    @Test func mapsAuthoredAndReviewRequests() async throws {
        let plugin = try GitHubPlugin(
            config: config(["host": "github.com", "token": "t"]),
            http: StubHTTP(routes: ["/graphql": response])
        )
        let snapshot = try await plugin.fetch()
        #expect(snapshot.identity == "alice")

        let authored = try #require(snapshot.items.first { $0.bundle == .authored })
        #expect(authored.context == "acme/app #7")
        #expect(authored.hasBadge("approved") && authored.hasBadge("checks.failing"))
        #expect(authored.badges.first { $0.id == "approved" }?.notify != nil)
        #expect(authored.participants.map(\.name) == ["erin", "platform-team"])
        #expect(authored.needsAction, "approved with failing checks: your turn")

        let review = try #require(snapshot.items.first { $0.bundle == .reviews })
        #expect(review.hasBadge("conflicts"))
        #expect(review.changes?.files.map(\.path) == ["src/search/index.ts", "src/search/index.test.ts"])
        #expect(review.changes?.fileCount == 3)
        #expect(authored.changes == nil, "your own PRs don't need a review prep")
        #expect(review.needsAction)
    }

    @Test func surfacesGraphQLErrors() async throws {
        let plugin = try GitHubPlugin(
            config: config(["host": "https://github.com", "token": "t"]),
            http: StubHTTP(routes: ["/graphql": #"{"errors": [{"message": "Bad credentials"}]}"#])
        )
        // A rejected token: the account asks for reconnecting.
        await #expect(throws: HTTPError.unauthorized) { try await plugin.fetch() }
    }

    @Test func requiresToken() {
        #expect(throws: PluginError.missingField("token")) {
            try GitHubPlugin(config: config(["host": "https://github.com"]), http: StubHTTP(routes: [:]))
        }
    }
}

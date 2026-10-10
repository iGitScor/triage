import Foundation
import Testing
import RemoraCore
@testable import RemoraPlugins

struct GitLabPluginTests {
    let mr = """
    {"id": 101, "iid": 12, "project_id": 5, "title": "Speed up CI", "web_url": "https://gitlab.acme.io/g/p/-/merge_requests/12",
     "draft": false, "updated_at": "2026-10-08T10:00:00.000Z", "author": {"username": "alice", "avatar_url": "/uploads/a.png"},
     "reviewers": [{"username": "erin"}, {"username": "frank"}], "user_notes_count": 2, "has_conflicts": false,
     "references": {"full": "g/p!12"}, "detailed_merge_status": "mergeable"}
    """

    @Test func mapsMergeRequestsWithApprovalsAndPipeline() async throws {
        let http = StubHTTP(routes: [
            "/api/v4/user": #"{"username": "alice"}"#,
            "/api/v4/merge_requests": "[\(mr)]",
            "/merge_requests/12/approvals": #"{"approved": true, "approved_by": [{"user": {"username": "erin"}}]}"#,
            "/merge_requests/12": #"{"head_pipeline": {"status": "success"}}"#,
            "/merge_requests/12/diffs": #"[{"new_path": "db/migrate/add_avatars.rb", "old_path": "db/migrate/add_avatars.rb", "diff": "+ secret code"}]"#,
        ])
        let plugin = try GitLabPlugin(config: config(["host": "gitlab.acme.io", "token": "t"]), http: http)
        let snapshot = try await plugin.fetch()

        #expect(snapshot.identity == "alice")
        #expect(snapshot.items.map(\.bundle) == [.authored, .reviews])
        let item = snapshot.items[0]
        #expect(item.context == "g/p !12")
        #expect(item.hasBadge("approved") && item.hasBadge("checks.passing") && item.hasBadge("comments"))
        #expect(item.participants.first { $0.name == "erin" }?.tone == .accent)
        #expect(item.participants.first { $0.name == "frank" }?.tone == nil)
        #expect(item.author?.avatarURL?.absoluteString == "https://gitlab.acme.io/uploads/a.png")
        let review = snapshot.items[1]
        #expect(review.changes?.files == [ChangedFile(path: "db/migrate/add_avatars.rb")])
        let stored = String(data: try JSONEncoder().encode(review), encoding: .utf8) ?? ""
        #expect(!stored.contains("secret code"), "diff text is never kept")
    }

    /// One GraphQL request instead of two or three REST calls per merge request.
    @Test func detailsComeFromOneGraphQLRequest() async throws {
        let graphQL = """
        {"data": {"currentUser": {
          "authored": {"nodes": [{"id": "gid://gitlab/MergeRequest/101", "approved": true,
            "approvedBy": {"nodes": [{"username": "erin", "avatarUrl": null}]}, "headPipeline": {"status": "FAILED"}}]},
          "reviewing": {"nodes": [{"id": "gid://gitlab/MergeRequest/101", "approved": true,
            "approvedBy": {"nodes": [{"username": "erin"}]}, "headPipeline": {"status": "FAILED"},
            "diffStats": [{"path": "app/models/user.rb", "additions": 12, "deletions": 3}]}]}
        }}}
        """
        let http = RecordingHTTP(StubHTTP(routes: [
            "/api/v4/user": #"{"username": "alice"}"#,
            "/api/v4/merge_requests": "[\(mr)]",
            "/api/graphql": graphQL,
        ]))
        let snapshot = try await GitLabPlugin(config: config(["host": "gitlab.acme.io", "token": "t"]), http: http).fetch()

        let paths = await http.paths
        #expect(paths.filter { $0.contains("/projects/") }.isEmpty, "no per-merge-request call")
        #expect(paths.count == 4, "user, the two lists, and one GraphQL request")
        let authored = snapshot.items[0]
        #expect(authored.hasBadge("approved") && authored.hasBadge("checks.failing"))
        #expect(authored.changes == nil)
        let review = snapshot.items[1]
        #expect(review.changes?.files == [ChangedFile(path: "app/models/user.rb", additions: 12, deletions: 3)])
    }
}

/// Records the paths a plugin requests, then answers with `base`.
actor RecordingHTTP: HTTPClient {
    let base: StubHTTP
    private(set) var paths: [String] = []

    init(_ base: StubHTTP) { self.base = base }

    func send(_ request: URLRequest) async throws -> (Data, HTTPURLResponse) {
        paths.append(request.url?.path ?? "")
        return try await base.send(request)
    }
}

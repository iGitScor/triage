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
}

import Foundation
import RemoraCore
import Testing

@testable import RemoraPlugins

struct GitLabPluginTests {
    let mr = """
        {"id": 101, "iid": 12, "project_id": 5, "title": "Speed up CI", "web_url": "https://gitlab.acme.io/g/p/-/merge_requests/12",
         "draft": false, "updated_at": "2026-10-08T10:00:00.000Z", "author": {"username": "alice", "avatar_url": "/uploads/a.png"},
         "reviewers": [{"username": "erin"}, {"username": "frank"}], "user_notes_count": 2, "has_conflicts": false,
         "references": {"full": "g/p!12"}, "detailed_merge_status": "mergeable"}
        """
    /// The merge request to review: `mr` under another id, so its details come from the same routes.
    var review: String { mr.replacingOccurrences(of: #""id": 101"#, with: #""id": 102"#) }
    var lists: [String: String] {
        [
            "/api/v4/merge_requests?scope=created_by_me": "[\(mr)]",
            "/api/v4/merge_requests?reviewer_username=": "[\(review)]",
        ]
    }

    @Test func mapsMergeRequestsWithApprovalsAndPipeline() async throws {
        let http = StubHTTP(
            routes: lists.merging([
                "/api/v4/user": #"{"username": "alice"}"#,
                "/merge_requests/12/approvals": #"{"approved": true, "approved_by": [{"user": {"username": "erin"}}]}"#,
                "/merge_requests/12": #"{"head_pipeline": {"status": "success"}}"#,
                "/merge_requests/12/diffs":
                    #"[{"new_path": "db/migrate/add_avatars.rb", "old_path": "db/migrate/add_avatars.rb", "diff": "+ secret code"}]"#,
            ]) { a, _ in a })
        let plugin = try GitLabPlugin(config: config(["host": "gitlab.acme.io", "token": "t"]), http: http)
        let snapshot = try await plugin.fetch()

        #expect(snapshot.identity == "alice")
        #expect(snapshot.items.map(\.bundle) == [.authored, .reviews])
        let item = snapshot.items[0]
        #expect(item.context == "g/p !12")
        #expect(item.hasBadge(.approved) && item.hasBadge(.checksPassing) && item.hasBadge(.comments))
        #expect(item.participants.first { $0.name == "erin" }?.review == .approved)
        #expect(item.participants.first { $0.name == "frank" }?.review == nil)
        #expect(item.author?.avatarURL?.absoluteString == "https://gitlab.acme.io/uploads/a.png")
        let review = snapshot.items[1]
        #expect(review.changes?.files == [ChangedFile(path: "db/migrate/add_avatars.rb")])
        let stored = String(data: try JSONEncoder().encode(review), encoding: .utf8) ?? ""
        #expect(!stored.contains("secret code"), "diff text is never kept")
    }

    /// Approvals that can't be read are said, not passed over; a rate limit there fails the fetch, so the backoff
    /// waits it out and the last items stay (CODE-02).
    @Test func failedDetailsAreSaidAndARateLimitIsWaitedOut() async throws {
        struct Limited: HTTPClient {
            let base: StubHTTP
            func send(_ request: URLRequest) async throws -> (Data, HTTPURLResponse) {
                if request.url!.path.hasSuffix("/approvals") {
                    return (
                        Data(),
                        HTTPURLResponse(url: request.url!, statusCode: 429, httpVersion: nil, headerFields: nil)!
                    )
                }
                return try await base.send(request)
            }
        }
        let routes = lists.merging(["/api/v4/user": #"{"username": "alice"}"#, "/api/v4/todos": "[]"]) { a, _ in a }
        // No approvals, pipeline or diffs routes: each answers 404.
        let plugin = try GitLabPlugin(
            config: config(["host": "gitlab.acme.io", "token": "t"]), http: StubHTTP(routes: routes))
        let snapshot = try await plugin.fetch()
        #expect(snapshot.items.count == 2, "the merge requests still come")
        #expect(snapshot.remarks.count == 1)
        #expect(snapshot.remarks.first?.hasPrefix("Approvals, pipelines or changed files couldn’t be read") == true)

        let limited = try GitLabPlugin(
            config: config(["host": "gitlab.acme.io", "token": "t"]), http: Limited(base: StubHTTP(routes: routes)))
        await #expect(throws: HTTPError.rateLimited(nil)) { try await limited.fetch() }
    }

    /// CODE-06: a merge request of yours where you are also a reviewer comes once, as yours. Same on Windows.
    @Test func yourMergeRequestYouAlsoReviewComesOnce() async throws {
        let http = StubHTTP(routes: ["/api/v4/user": #"{"username": "alice"}"#, "/api/v4/merge_requests": "[\(mr)]"])
        let snapshot = try await GitLabPlugin(config: config(["host": "gitlab.acme.io", "token": "t"]), http: http)
            .fetch()
        #expect(snapshot.items.map(\.bundle) == [.authored])
    }

    /// One GraphQL request instead of two or three REST calls per merge request.
    @Test func detailsComeFromOneGraphQLRequest() async throws {
        let graphQL = """
            {"data": {"currentUser": {
              "authored": {"nodes": [{"id": "gid://gitlab/MergeRequest/101", "approved": true,
                "approvedBy": {"nodes": [{"username": "erin", "avatarUrl": null}]}, "headPipeline": {"status": "FAILED"}}]},
              "reviewing": {"nodes": [{"id": "gid://gitlab/MergeRequest/102", "approved": true,
                "approvedBy": {"nodes": [{"username": "erin"}]}, "headPipeline": {"status": "FAILED"},
                "diffStats": [{"path": "app/models/user.rb", "additions": 12, "deletions": 3}]}]}
            }}}
            """
        let http = RecordingHTTP(
            StubHTTP(
                routes: lists.merging([
                    "/api/v4/user": #"{"username": "alice"}"#, "/api/graphql": graphQL, "/api/v4/todos": "[]",
                ]) { a, _ in a }))
        let snapshot = try await GitLabPlugin(config: config(["host": "gitlab.acme.io", "token": "t"]), http: http)
            .fetch()

        let paths = await http.paths
        #expect(paths.filter { $0.contains("/projects/") }.isEmpty, "no per-merge-request call")
        #expect(paths.count == 5, "user, the two lists, the To-Dos, and one GraphQL request")
        let authored = snapshot.items[0]
        #expect(authored.hasBadge(.approved) && authored.hasBadge(.checksFailing))
        #expect(authored.changes == nil)
        let review = snapshot.items[1]
        #expect(review.changes?.files == [ChangedFile(path: "app/models/user.rb", additions: 12, deletions: 3)])
    }

    /// MISS-06: To-Dos other than merge requests, already listed: assigned issues to do, mentions to answer. Same
    /// case as Windows.
    @Test func toDosBringIssuesAndMentionsButNotMergeRequestsAgain() async throws {
        let todos = """
            [{"id": 1, "action_name": "assigned", "target_type": "Issue",
              "target": {"title": "Export *CSV*", "iid": 42}, "target_url": "https://gitlab.acme.io/acme/app/-/issues/42",
              "author": {"name": "Erin", "username": "erin", "avatar_url": null}, "project": {"path_with_namespace": "acme/app"},
              "updated_at": "2026-10-12T09:00:00Z"},
             {"id": 2, "action_name": "mentioned", "target_type": "Issue", "target": {"title": "Login loop", "iid": 7},
              "body": "@alice can you look?", "target_url": "https://gitlab.acme.io/acme/app/-/issues/7#note_1",
              "author": {"username": "frank"}, "project": {"path_with_namespace": "acme/app"}, "created_at": "2026-10-12T10:00:00Z"},
             {"id": 3, "action_name": "review_requested", "target_type": "MergeRequest", "target": {"title": "Fix", "iid": 3}}]
            """
        let snapshot = try await GitLabPlugin(
            config: config(["host": "gitlab.acme.io", "token": "t"]),
            http: StubHTTP(routes: [
                "/api/v4/user": #"{"username": "alice"}"#, "/api/v4/merge_requests": "[]", "/api/v4/todos": todos,
            ])
        ).fetch()
        #expect(snapshot.items.count == 2, "the merge request is already listed as such")
        let issue = try #require(snapshot.items.first)
        #expect(issue.bundle == .tasks && issue.title == "Export CSV" && issue.context == "acme/app #42")
        let mention = try #require(snapshot.items.last)
        #expect(mention.bundle == .mentions && mention.preview == "@alice can you look?")
        #expect(mention.author?.name == "frank")
        #expect(snapshot.remarks.isEmpty)
    }

    /// PERF-09: who the token belongs to is asked once, not at every refresh.
    @Test func theUserIsAskedOnceAcrossRefreshes() async throws {
        let http = RecordingHTTP(
            StubHTTP(routes: ["/api/v4/user": #"{"username": "alice"}"#, "/api/v4/merge_requests": "[]"]))
        let config = PluginConfig(
            accountID: UUID(), values: ["host": "gitlab.acme.io", "token": "t"], cache: MemoryAccountCache())
        for _ in 1...2 { _ = try await GitLabPlugin(config: config, http: http).fetch() }
        #expect(await http.paths.filter { $0.hasSuffix("/user") }.count == 1)
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

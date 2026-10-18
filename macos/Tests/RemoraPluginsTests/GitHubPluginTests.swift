import Foundation
import RemoraCore
import Testing

@testable import RemoraPlugins

struct GitHubPluginTests {
    /// An app account is a bot as GitHub says it: its `Bot` type, or a `[bot]` login; a person never is. As Windows.
    @Test func appAccountsAreBots() throws {
        func bot(_ json: String) throws -> Bool {
            try JSONDecoder().decode(PullRequest.Actor.self, from: Data(json.utf8)).person.bot
        }
        #expect(try bot(#"{"__typename": "Bot", "login": "dependabot"}"#))
        #expect(try bot(#"{"login": "renovate[bot]"}"#))
        #expect(try !bot(#"{"__typename": "User", "login": "alice"}"#))
        #expect(try !bot(#"{"login": "robotics-team"}"#), "a name alone says nothing")
    }

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
        let plugin = { (json: String) in
            try GitHubPlugin(
                config: config(["host": "github.com", "token": "t"]), http: StubHTTP(routes: ["/graphql": json]))
        }
        #expect(try await plugin(response).fetch().remarks.isEmpty, "no count: nothing to say")
        let fits = response.replacingOccurrences(
            of: #""reviewing": {"nodes""#, with: #""reviewing": {"issueCount": 1, "nodes""#)
        #expect(try await plugin(fits).fetch().remarks.isEmpty)
        let cut = response.replacingOccurrences(
            of: #""reviewing": {"nodes""#, with: #""reviewing": {"issueCount": 73, "nodes""#)
        #expect(try await plugin(cut).fetch().remarks == [SourceSnapshot.truncated("GitHub")])
    }

    /// Issues assigned to you are things to do; a list past 50 says so too. Same case as Windows.
    @Test func issuesAssignedToYouAreThingsToDo() async throws {
        let issue = """
            "assigned": {"issueCount": 60, "nodes": [{"id": "I_1", "number": 31, "title": "Crash on *launch*",
              "url": "https://github.com/acme/app/issues/31", "updatedAt": "2026-10-08T11:00:00Z",
              "repository": {"nameWithOwner": "acme/app"}, "author": {"login": "grace"}}]},
            """
        let json = response.replacingOccurrences(
            of: #""viewer": {"login": "alice"},"#, with: #""viewer": {"login": "alice"}, "# + issue)
        let snapshot = try await GitHubPlugin(
            config: config(["host": "github.com", "token": "t"]), http: StubHTTP(routes: ["/graphql": json])
        ).fetch()
        let task = try #require(snapshot.items.first { $0.bundle == .tasks })
        #expect(task.title == "Crash on launch" && task.context == "acme/app #31")
        #expect(task.author?.name == "grace" && task.needsAction)
        #expect(snapshot.remarks == [SourceSnapshot.truncated("GitHub")])
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
        #expect(authored.hasBadge(.approved) && authored.hasBadge(.checksFailing))
        #expect(authored.badges.first { $0.id == .approved }?.notify != nil)
        #expect(authored.participants.map(\.name) == ["erin", "platform-team"])
        #expect(authored.needsAction, "approved with failing checks: your turn")

        let review = try #require(snapshot.items.first { $0.bundle == .reviews })
        #expect(review.hasBadge(.conflicts))
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

    /// GraphQL points used up: GitHub answers 200 with a RATE_LIMITED error, and the reset is in the headers.
    @Test func graphQLRateLimitWaitsForTheReset() throws {
        let body = #"{"errors": [{"type": "RATE_LIMITED", "message": "API rate limit exceeded"}]}"#
        let response = try JSONDecoder.api().decode(GraphQLResponse.self, from: Data(body.utf8))
        let reset = Date(timeIntervalSince1970: 1_800_000_600)
        #expect(throws: HTTPError.rateLimited(reset)) {
            try GitHubPlugin.snapshot(from: response, accountID: UUID(), resume: reset)
        }
    }

    @Test func requiresToken() {
        #expect(throws: PluginError.missingField("token")) {
            try GitHubPlugin(config: config(["host": "https://github.com"]), http: StubHTTP(routes: [:]))
        }
    }

    @Test func readsTheRepositoriesYouMaintain() {
        #expect(
            Maintained.repositories(" acme/web, acme/api\nAcme/Web  bad /x  a/b/c  acme/my.repo_2 ", nested: false)
                == ["acme/web", "acme/api", "acme/my.repo_2"])
        #expect(Maintained.repositories("acme/tools/cli, acme/web", nested: true) == ["acme/tools/cli", "acme/web"])
        let many = (0..<15).map { "acme/r\($0)" }.joined(separator: ",")
        #expect(Maintained.repositories(many, nested: false).count == Maintained.limit)
        #expect(Maintained.repositories(nil, nested: true).isEmpty)
    }

    let redRun = """
        {"workflow_runs": [{"id": 42, "name": "CI", "display_title": "Bump *serde*", "conclusion": "failure",
          "html_url": "https://github.com/acme/web/actions/runs/42", "updated_at": "2026-10-08T12:00:00Z"}]}
        """

    func maintaining(_ routes: [String: String]) throws -> GitHubPlugin {
        try GitHubPlugin(
            config: config(["host": "github.com", "token": "t", "repositories": "acme/web"]),
            http: StubHTTP(routes: routes.merging(["/graphql": response]) { a, _ in a }))
    }

    /// A red default branch and open critical or high alerts are things to fix. Same cases as Windows.
    @Test func aFailingDefaultBranchAndSecurityAlertsAreToFix() async throws {
        let snapshot = try await maintaining([
            "/repos/acme/web": #"{"default_branch": "main"}"#,
            "/repos/acme/web/actions/runs?branch=main": redRun,
            "/repos/acme/web/dependabot/alerts?severity=critical,high": """
            [{"number": 3, "html_url": "https://github.com/acme/web/security/dependabot/3",
              "created_at": "2026-10-07T08:00:00Z", "dependency": {"package": {"name": "lodash"}},
              "security_advisory": {"summary": "Prototype pollution", "severity": "critical"}}]
            """,
            "/repos/acme/web/code-scanning/alerts": """
            [{"number": 5, "html_url": "https://github.com/acme/web/security/code-scanning/5",
              "created_at": "2026-10-06T08:00:00Z", "rule": {"description": "SQL injection", "security_severity_level": "high"}}]
            """,
        ]).fetch()
        #expect(snapshot.remarks.isEmpty)
        let fix = snapshot.items.filter { $0.bundle == .fix }
        #expect(fix.count == 3)
        let ci = try #require(fix.first { $0.hasBadge(.checksFailing) })
        #expect(ci.title == "Bump serde" && ci.context == "acme/web · main" && ci.preview == "CI")
        #expect(ci.url?.absoluteString == "https://github.com/acme/web/actions/runs/42")
        #expect(ci.needsAction && ci.badges[0].notify != nil)
        let dependabot = try #require(fix.first { $0.title.hasPrefix("lodash") })
        #expect(dependabot.title == "lodash: Prototype pollution")
        #expect(dependabot.context == "acme/web" && dependabot.priority == .urgent)
        let scanning = try #require(fix.first { $0.title == "SQL injection" })
        #expect(scanning.priority == .high && scanning.id != dependabot.id)
    }

    /// A green run is nothing; no Actions or no code scanning is silent; alerts that can't be read say so.
    @Test func aGreenBranchIsNothingAndUnreadableAlertsAreARemark() async throws {
        let snapshot = try await maintaining([
            "/repos/acme/web": #"{"default_branch": "main"}"#,
            "/repos/acme/web/actions/runs": redRun.replacingOccurrences(of: "failure", with: "success"),
            "/repos/acme/web/dependabot/alerts": #"{"message": "nope"}"#,
        ]).fetch()
        #expect(!snapshot.items.contains { $0.bundle == .fix })
        #expect(snapshot.remarks.count == 1)
        #expect(snapshot.remarks.first?.hasPrefix("Security alerts couldn’t be read") == true)

        let missing = try await maintaining([:]).fetch()
        #expect(missing.remarks.first?.hasPrefix("Repositories you maintain couldn’t be read") == true)
        #expect(missing.items.contains { $0.bundle == .authored }, "pull requests still listed")
    }
}

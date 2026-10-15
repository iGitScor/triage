import Foundation
import RemoraCore
import Testing

@testable import RemoraPlugins

/// Answers Bitbucket by the end of the path, and the Data Center dashboard by its role. Checks the Authorization
/// header. Same fixtures as Windows (`bitbucket.rs`).
struct BitbucketStub: HTTPClient {
    var routes: [String: String]
    var authorization: String

    func send(_ request: URLRequest) async throws -> (Data, HTTPURLResponse) {
        #expect(request.value(forHTTPHeaderField: "Authorization") == authorization)
        let url = request.url!
        let role = URLComponents(url: url, resolvingAgainstBaseURL: false)?.queryItems?.first { $0.name == "role" }?
            .value
        let key = url.path + (url.path.hasSuffix("/dashboard/pull-requests") ? "?" + (role ?? "") : "")
        // The longest matching suffix, so `…/pullrequests/7/statuses` doesn't answer for `…/pullrequests`.
        let body = routes.filter { key.hasSuffix($0.key) }.max { $0.key.count < $1.key.count }?.value
        return (
            Data((body ?? "").utf8),
            HTTPURLResponse(url: url, statusCode: body == nil ? 404 : 200, httpVersion: nil, headerFields: nil)!
        )
    }
}

enum BitbucketFixtures {
    static let user = #"{"uuid": "{a1}", "account_id": "alice-id", "display_name": "Alice", "nickname": "alice"}"#

    static let authored = """
        {"values": [{"id": 7, "title": "Retry failed webhooks", "state": "OPEN", "draft": false, "comment_count": 2,
          "updated_on": "2026-10-12T09:14:03.123456+00:00",
          "author": {"uuid": "{a1}", "display_name": "Alice"},
          "destination": {"repository": {"full_name": "acme/inbox"}},
          "links": {"html": {"href": "https://bitbucket.org/acme/inbox/pull-requests/7"}},
          "participants": [
            {"user": {"uuid": "{b2}", "display_name": "Bob", "links": {"avatar": {"href": "https://secure.gravatar.com/avatar/b"}}},
             "role": "REVIEWER", "approved": true, "state": "approved"}]}],
         "pagelen": 50}
        """

    static let repositories = #"{"values": [{"full_name": "acme/web"}, {"full_name": "acme/api"}]}"#

    static let reviewing = """
        {"values": [
          {"id": 12, "title": "Add dark mode", "state": "OPEN", "comment_count": 0,
           "updated_on": "2026-10-12T08:00:00.000001+00:00",
           "author": {"uuid": "{e5}", "display_name": "Erin"},
           "destination": {"repository": {"full_name": "acme/web"}},
           "links": {"html": {"href": "https://bitbucket.org/acme/web/pull-requests/12"}},
           "participants": [
             {"user": {"uuid": "{a1}", "display_name": "Alice"}, "role": "REVIEWER", "approved": false},
             {"user": {"uuid": "{b2}", "display_name": "Bob"}, "role": "REVIEWER", "approved": false, "state": "changes_requested"}]},
          {"id": 13, "title": "Bump dependencies", "state": "OPEN",
           "updated_on": "2026-10-11T08:00:00+00:00",
           "author": {"uuid": "{e5}", "display_name": "Erin"},
           "destination": {"repository": {"full_name": "acme/web"}},
           "participants": [{"user": {"uuid": "{a1}", "display_name": "Alice"}, "role": "REVIEWER", "approved": true, "state": "approved"}]}
        ]}
        """

    static func cloud(authored: String = authored) -> [String: String] {
        [
            "/2.0/user": user,
            "/2.0/workspaces/acme/pullrequests/alice-id": authored,
            "/2.0/repositories/acme": repositories,
            "/2.0/repositories/acme/web/pullrequests": reviewing,
            "/2.0/repositories/acme/api/pullrequests": #"{"values": []}"#,
            "/2.0/repositories/acme/inbox/pullrequests/7/statuses":
                #"{"values": [{"state": "SUCCESSFUL"}, {"state": "FAILED"}]}"#,
            "/2.0/repositories/acme/web/pullrequests/12/statuses": #"{"values": [{"state": "INPROGRESS"}]}"#,
        ]
    }

    static let dataCenterAuthored = """
        {"values": [{"id": 42, "title": "Draft: new login page", "draft": true, "updatedDate": 1760259243000,
          "author": {"user": {"name": "alice", "displayName": "Alice"}, "status": "UNAPPROVED"},
          "reviewers": [],
          "fromRef": {"latestCommit": "c0ffee"},
          "toRef": {"repository": {"slug": "portal", "project": {"key": "WEB"}}},
          "links": {"self": [{"href": "https://bitbucket.acme.io/projects/WEB/repos/portal/pull-requests/42"}]}}],
         "isLastPage": true}
        """

    static let dataCenterReviewing = """
        {"values": [{"id": 9, "title": "Rotate keys", "updatedDate": 1760255643000,
          "author": {"user": {"name": "erin", "displayName": "Erin"}},
          "reviewers": [
            {"user": {"name": "alice", "displayName": "Alice"}, "status": "UNAPPROVED"},
            {"user": {"name": "bob", "displayName": "Bob"}, "status": "NEEDS_WORK"}],
          "fromRef": {"latestCommit": "beef"},
          "toRef": {"repository": {"slug": "vault", "project": {"key": "OPS"}}},
          "links": {"self": [{"href": "https://bitbucket.acme.io/projects/OPS/repos/vault/pull-requests/9"}]},
          "properties": {"commentCount": 3, "mergeResult": {"outcome": "CONFLICTED"}}}],
         "isLastPage": false}
        """

    static let dataCenter: [String: String] = [
        "/plugins/servlet/applinks/whoami": "alice\n",
        "/rest/api/1.0/dashboard/pull-requests?AUTHOR": dataCenterAuthored,
        "/rest/api/1.0/dashboard/pull-requests?REVIEWER": dataCenterReviewing,
        "/rest/build-status/1.0/commits/stats/c0ffee": #"{"successful": 2, "failed": 0, "inProgress": 0}"#,
        "/rest/build-status/1.0/commits/stats/beef": #"{"successful": 0, "failed": 0, "inProgress": 1}"#,
    ]
}

struct BitbucketPluginTests {
    let cloudConfig = ["workspace": "acme", "email": "alice@acme.com", "token": "t0ken"]
    var cloudAuth: String { "Basic " + Data("alice@acme.com:t0ken".utf8).base64EncodedString() }

    func cloud(_ routes: [String: String] = BitbucketFixtures.cloud()) throws -> BitbucketPlugin {
        try BitbucketPlugin(
            config: config(cloudConfig.merging(["host": "https://bitbucket.org"]) { $1 }),
            http: BitbucketStub(routes: routes, authorization: cloudAuth))
    }

    @Test func cloudPullRequestsYouOpenedAndReview() async throws {
        let snapshot = try await cloud().fetch()
        #expect(snapshot.identity == "Alice")
        #expect(snapshot.remarks.isEmpty)
        #expect(snapshot.items.map(\.context) == ["acme/inbox #7", "acme/web #12"], "#13 is approved by you")

        let mine = snapshot.items[0]
        #expect(mine.bundle == .authored && mine.needsAction)
        #expect(mine.badges.map(\.id) == ["approved", "checks.failing", "comments"])
        #expect(mine.participants.map(\.name) == ["Bob"] && mine.participants.first?.tone == ReviewerTone.approved)
        #expect(mine.url?.absoluteString == "https://bitbucket.org/acme/inbox/pull-requests/7")
        #expect(mine.date == Date(iso8601: "2026-10-12T09:14:03.123Z"))

        let review = snapshot.items[1]
        #expect(review.bundle == .reviews && review.needsAction)
        #expect(review.badges.map(\.id) == ["changes", "checks.running"])
        #expect(review.author?.name == "Erin")
        #expect(review.participants.map(\.name) == ["Bob"], "you aren't listed among the reviewers")
    }

    @Test func cloudSaysWhenPullRequestsAreCut() async throws {
        let more = BitbucketFixtures.authored.replacingOccurrences(
            of: #""pagelen": 50"#, with: #""next": "https://api.bitbucket.org/2.0/next""#)
        var routes = BitbucketFixtures.cloud()
        routes["/2.0/workspaces/acme/pullrequests/alice-id"] = more
        #expect(try await cloud(routes).fetch().remarks == [SourceSnapshot.truncated("Bitbucket")])
    }

    /// A token that can't list repositories: your own pull requests still come, with a remark.
    @Test func reviewRequestsAreBestEffort() async throws {
        var routes = BitbucketFixtures.cloud()
        routes["/2.0/repositories/acme"] = nil
        let snapshot = try await cloud(routes).fetch()
        #expect(snapshot.items.map(\.context) == ["acme/inbox #7"])
        #expect(snapshot.remarks.count == 1)
    }

    @Test func cloudNeedsTheWorkspaceAndEmail() {
        #expect(throws: PluginError.missingField("workspace")) {
            try BitbucketPlugin(config: config(["host": "bitbucket.org", "token": "t"]), http: StubHTTP(routes: [:]))
        }
        #expect(throws: PluginError.missingField("email")) {
            try BitbucketPlugin(
                config: config(["host": "bitbucket.org", "workspace": "acme", "token": "t"]),
                http: StubHTTP(routes: [:]))
        }
    }

    /// Data Center: an HTTP access token as Bearer, the dashboard, "needs work", conflicts and build statistics.
    @Test func dataCenterUsesTheDashboard() async throws {
        let plugin = try BitbucketPlugin(
            config: config(["host": "bitbucket.acme.io", "token": "pat"]),
            http: BitbucketStub(routes: BitbucketFixtures.dataCenter, authorization: "Bearer pat"))
        let snapshot = try await plugin.fetch()
        #expect(snapshot.identity == "alice")
        #expect(snapshot.remarks == [SourceSnapshot.truncated("Bitbucket")])
        #expect(snapshot.items.map(\.context) == ["WEB/portal #42", "OPS/vault #9"])

        let draft = snapshot.items[0]
        #expect(draft.bundle == .authored && !draft.needsAction)
        #expect(draft.badges.map(\.id) == ["draft", "checks.passing"])
        #expect(draft.url?.absoluteString == "https://bitbucket.acme.io/projects/WEB/repos/portal/pull-requests/42")
        #expect(draft.date == Date(timeIntervalSince1970: 1_760_259_243))

        let review = snapshot.items[1]
        #expect(review.bundle == .reviews && review.needsAction)
        #expect(review.badges.map(\.id) == ["changes", "conflicts", "checks.running", "comments"])
        #expect(review.participants.map(\.tone) == [ReviewerTone.waiting, ReviewerTone.changes])
    }

    @Test func readsCloudDatesWithMicroseconds() {
        #expect(BitbucketPlugin.date("2026-10-12T09:14:03.123456+00:00") == Date(iso8601: "2026-10-12T09:14:03.123Z"))
        #expect(BitbucketPlugin.date("2026-10-12T09:14:03+00:00") != nil)
        #expect(BitbucketPlugin.checks(["SUCCESSFUL", "STOPPED"]) == .passing)
        #expect(BitbucketPlugin.checks([]) == .none)
    }

    @Test func usesOnlyItsDeclaredHosts() async throws {
        let guarded = GuardedHTTPClient(
            BitbucketStub(routes: BitbucketFixtures.cloud(), authorization: cloudAuth),
            allowing: BitbucketPlugin.manifest.egress.hosts)
        let plugin = try BitbucketPlugin(
            config: config(cloudConfig.merging(["host": "bitbucket.org"]) { $1 }), http: guarded)
        #expect(try await plugin.fetch().items.count == 2)
    }
}

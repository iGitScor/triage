import Foundation
import RemoraCore
import Testing

@testable import RemoraPlugins

struct ComplianceTests {
    @Test func guardedClientOnlyReachesDeclaredHosts() async throws {
        let client = GuardedHTTPClient(StubHTTP(routes: ["/x": "{}"]), allowing: ["api.linear.app", "gitlab.acme.io"])
        _ = try await client.send(.get(URL(string: "https://api.linear.app/x")!))
        _ = try await client.send(.get(URL(string: "https://eu.gitlab.acme.io/x")!))
        await #expect(throws: EgressError.blockedHost("evil.example.com")) {
            try await client.send(.get(URL(string: "https://evil.example.com/x")!))
        }
        await #expect(throws: EgressError.blockedHost("api.linear.app.evil.com")) {
            try await client.send(.get(URL(string: "https://api.linear.app.evil.com/x")!))
        }
    }

    @Test func externalAIIsOffByDefault() {
        let policy = CompliancePolicy()
        #expect(!policy.allows(ClaudeCodePlugin.manifest))
        #expect(!policy.allows(ClaudePlugin.manifest))
        #expect(policy.allows(GitHubPlugin.manifest))
        #expect(CompliancePolicy(allowExternalAI: true).allows(ClaudeCodePlugin.manifest))
    }

    @Test func allowListRestrictsSources() {
        let policy = CompliancePolicy(allowedPlugins: ["github"], allowExternalAI: true)
        #expect(policy.allows(GitHubPlugin.manifest))
        #expect(!policy.allows(SlackPlugin.manifest))
        #expect(policy.refusal(for: SlackPlugin.manifest) != nil)
    }

    @Test func everyPluginDeclaresWhereItsDataGoes() {
        for manifest in PluginRegistry.manifests + PluginRegistry.assistantManifests {
            #expect(!manifest.egress.description.isEmpty, "\(manifest.id)")
            #expect(
                !manifest.egress.hosts.isEmpty || manifest.id == "claude-code" || manifest.id == "gitlab"
                    || manifest.egress.serverField != nil,
                "\(manifest.id) must declare hosts, or the setting that holds its server")
        }
    }

    /// Every plugin in the registry fetches its fixture through the guard, with the hosts the app would allow it: the
    /// fetch succeeds, every host it asked is declared, and its item ids follow `InboxItem.makeID`. A plugin added
    /// without a fixture here fails.
    @Test(arguments: (PluginRegistry.manifests + PluginRegistry.assistantManifests).map(\.id))
    func pluginsUseOnlyTheirDeclaredHosts(id: String) async throws {
        let manifest = try #require(PluginRegistry.manifest(id))
        let scenario = try #require(Self.scenario(id), "\(id) has no fixture: add one so its hosts are checked")
        guard case .fetch(let settings, let stub) = scenario else {
            #expect(manifest.egress.hosts.isEmpty && manifest.egress.serverField == nil, "\(id) reaches no host")
            return
        }
        let allowed = manifest.allowedHosts(in: settings)
        let http = HostRecorder(GuardedHTTPClient(stub, allowing: allowed))
        if let source = PluginRegistry.source(id) {
            let account = config(settings)
            let snapshot = try await source.init(config: account, http: http).fetch()
            // A list cut at 50 may be said; a part that couldn't be read may not.
            let failures = snapshot.remarks.filter { $0 != SourceSnapshot.truncated(manifest.name) }
            #expect(!snapshot.items.isEmpty && failures.isEmpty, "\(id): \(failures)")
            // CODE-06: ids start with the account's, so two accounts never share an item, and are unique.
            for item in snapshot.items {
                #expect(item.accountID == account.accountID && item.localID?.isEmpty == false, "\(id): \(item.id)")
            }
            #expect(Set(snapshot.items.map(\.id)).count == snapshot.items.count, "\(id) repeats an item id")
        } else {
            let assistant = try #require(PluginRegistry.assistants.first { $0.manifest.id == id })
            let item = InboxItem(
                id: "a", accountID: UUID(), pluginID: "github", bundle: .reviews, title: "Fix login",
                context: "acme/app #9", date: .now)
            let brief = try await assistant.init(config: config(settings), http: http).brief([item], now: .now)
            #expect(!brief.summary.isEmpty)
        }
        let hosts = await http.hosts
        #expect(!hosts.isEmpty)
        for host in hosts {
            #expect(GuardedHTTPClient.matches(host, allowed), "\(id) asked \(host), which its manifest doesn't declare")
        }
    }

    enum Scenario {
        /// An account's settings, and a stub answering its fetch with the plugin's own test fixtures.
        case fetch([String: String], HTTPClient)
        /// Makes no request: Claude Code runs a program on this Mac.
        case noRequest
    }

    /// Cloud hosts where the tool has them, so the declared hosts are what the guard allows.
    static func scenario(_ id: String) -> Scenario? {
        switch id {
        case "github":
            return .fetch(
                ["host": "github.com", "token": "t"], StubHTTP(routes: ["/graphql": GitHubPluginTests().response]))
        case "gitlab":
            return .fetch(
                ["host": "gitlab.com", "token": "t"],
                StubHTTP(
                    routes: GitLabPluginTests().lists.merging([
                        "/api/v4/user": #"{"username": "alice"}"#,
                        "/api/v4/todos": "[]",
                        "/api/graphql": """
                        {"data": {"currentUser": {
                          "authored": {"nodes": [{"id": "gid://gitlab/MergeRequest/101", "approved": true, "headPipeline": {"status": "SUCCESS"}}]},
                          "reviewing": {"nodes": [{"id": "gid://gitlab/MergeRequest/102", "approved": true, "diffStats": []}]}
                        }}}
                        """,
                    ]) { a, _ in a }))
        case "bitbucket":
            let tests = BitbucketPluginTests()
            return .fetch(
                tests.cloudConfig.merging(["host": "bitbucket.org"]) { $1 },
                BitbucketStub(routes: BitbucketFixtures.cloud(), authorization: tests.cloudAuth))
        case "azuredevops":
            return .fetch(
                ["host": "dev.azure.com/acme/", "token": "t"], AzureDevOpsStub(routes: AzureDevOpsFixtures.routes))
        case "slack":
            return .fetch(
                ["token": "xoxp"],
                SlackStub(mine: SlackPluginTests.yourThread, replies: SlackPluginTests.replies(reactions: "[]")))
        case "linear":
            return .fetch(
                ["token": "lin_api_key"],
                LinearStub(
                    issues: LinearPluginTests().issues, notifications: #"{"data": {"notifications": {"nodes": []}}}"#))
        case "jira":
            let tests = JiraPluginTests()
            return .fetch(
                ["host": "acme.atlassian.net", "email": "alice@acme.com", "token": "t0ken", "confluence": "true"],
                JiraStub(
                    myself: tests.cloudMe, assigned: tests.assigned, watched: tests.watched(mentioning: "alice-id"),
                    expectedAuthorization: tests.cloudAuth))
        case "notion":
            let page = """
                {"id": "a", "url": "https://notion.so/a", "last_edited_time": "2026-10-08T10:00:00.000Z",
                 "properties": {"Name": {"type": "title", "title": [{"plain_text": "Task a"}]}}}
                """
            return .fetch(
                ["token": "secret", "databases": "0123456789abcdef0123456789abcdef"],
                StubHTTP(routes: [
                    "/users/me": #"{"id": "u1", "type": "person", "name": "Alice"}"#,
                    "/databases/0123456789abcdef0123456789abcdef":
                        #"{"title": [{"plain_text": "Team tasks"}], "data_sources": [{"id": "ds1"}]}"#,
                    "/data_sources/ds1": #"{"properties": {}}"#,
                    "/data_sources/ds1/query": #"{"results": [\#(page)]}"#,
                ]))
        case "microsoft":
            return .fetch(
                ["tenant": "acme.com", "clientId": "app-1", "refreshToken": "r1"],
                StubHTTP(routes: [
                    "/oauth2/v2.0/token": #"{"access_token": "a1", "refresh_token": "r2", "expires_in": 3600}"#,
                    "/v1.0/me": MicrosoftPluginTests.me, "/me/messages": MicrosoftPluginTests.flagged,
                    "/inbox/messages": MicrosoftPluginTests.unread, "/me/calendarView": MicrosoftPluginTests.events,
                    "/me/chats": MicrosoftPluginTests.chats,
                ]))
        case "claude":
            let text = #"{\"summary\": \"One review waits.\", \"focus\": []}"#
            return .fetch(
                ["token": "k"],
                StubHTTP(routes: [
                    "/v1/messages": #"{"content": [{"type": "text", "text": "\#(text)"}], "stop_reason": "end_turn"}"#
                ]))
        case "openai":
            return .fetch(["token": "sk", "model": "gpt-x"], OpenAIPluginTests.Server())
        case "claude-code":
            return .noRequest
        default:
            return nil
        }
    }
}

/// Keeps the host of every request before the guard sees it, so a blocked one the plugin passes over still counts.
actor HostRecorder: HTTPClient {
    let base: HTTPClient
    private(set) var hosts: Set<String> = []

    init(_ base: HTTPClient) { self.base = base }

    func send(_ request: URLRequest) async throws -> (Data, HTTPURLResponse) {
        hosts.insert(request.url?.host?.lowercased() ?? "?")
        return try await base.send(request)
    }
}

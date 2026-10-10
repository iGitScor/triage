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

    @Test func pluginsUseOnlyTheirDeclaredHosts() async throws {
        let issues = #"{"data": {"viewer": {"name": "Alice", "assignedIssues": {"nodes": []}}}}"#
        let linear = try LinearPlugin(
            config: config(["token": "lin_api_key"]),
            http: GuardedHTTPClient(
                LinearStub(issues: issues, notifications: issues), allowing: LinearPlugin.manifest.egress.hosts)
        )
        _ = try await linear.fetch()
    }
}

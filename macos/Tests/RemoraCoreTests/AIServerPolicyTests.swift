import Foundation
import Testing

@testable import RemoraCore

/// AI-23: the policy's rules for an assistant whose server the user chooses.
struct AIServerPolicyTests {
    let manifest = PluginManifest(
        id: "openai", name: "OpenAI-compatible", symbol: "sparkles", summary: "",
        fields: [ConfigField(key: "host", label: "Server", defaultValue: "https://api.openai.com/v1")],
        egress: Egress(hosts: [], description: "", externalAI: true, serverField: "host"))
    let claude = PluginManifest(
        id: "claude", name: "Claude API", symbol: "sparkles", summary: "", fields: [],
        egress: Egress(hosts: ["api.anthropic.com"], description: "", externalAI: true))

    @Test func prefixesMatchSchemeHostPortAndWholePathSegments() {
        #expect(
            ServerPrefix.matches("https://acme.openai.azure.com/openai/v1", "https://acme.openai.azure.com/openai/"))
        #expect(!ServerPrefix.matches("https://acme.openai.azure.com/openaix", "https://acme.openai.azure.com/openai"))
        #expect(ServerPrefix.matches("https://API.openai.com/v1/", "https://api.openai.com/v1"), "case, trailing slash")
        #expect(ServerPrefix.matches("https://api.openai.com:443/v1", "https://api.openai.com"), "default port")
        #expect(!ServerPrefix.matches("https://api.openai.com:8443/v1", "https://api.openai.com"))
        #expect(!ServerPrefix.matches("http://api.openai.com/v1", "https://api.openai.com"), "scheme")
        #expect(ServerPrefix.matches("http://localhost:11434/v1", "http://localhost:11434"))
        #expect(!ServerPrefix.matches("http://localhost:8080/v1", "http://localhost:11434"))
        #expect(!ServerPrefix.matches("https://evil.com/api.openai.com", "https://api.openai.com"))
        #expect(!ServerPrefix.matches("https://api.openai.com.evil.com/v1", "https://api.openai.com"))
    }

    @Test func loopbackIsThisComputerOnly() {
        for host in ["localhost", "LOCALHOST", "127.0.0.1", "127.1.2.3", "::1", "[::1]"] {
            #expect(PluginConfig.isLoopback(host), "\(host)")
        }
        for host in ["128.0.0.1", "127.0.0", "127.0.0.256", "localhost.acme.io", "10.0.0.1", "api.openai.com"] {
            #expect(!PluginConfig.isLoopback(host), "\(host)")
        }
    }

    @Test func allowedServersRestrictTheServer() {
        let policy = CompliancePolicy(
            allowExternalAI: true, allowedAIServers: ["https://acme.openai.azure.com/openai/"])
        #expect(policy.refusal(for: manifest, settings: ["host": "https://acme.openai.azure.com/openai/v1"]) == nil)
        #expect(
            policy.refusal(for: manifest, settings: [:]) == L("Your organization doesn’t allow this AI server."),
            "the default server too")
        #expect(policy.refusal(for: claude, settings: [:]) == nil, "Claude has no server to choose")
        let none = CompliancePolicy(allowExternalAI: true, allowedAIServers: [])
        #expect(!none.allows(manifest, settings: ["host": "https://api.openai.com/v1"]), "a list that allows none")
    }

    @Test func aServerOnThisComputerFollowsAllowLocalAI() {
        let local = ["host": "http://localhost:11434/v1"]
        let cloud = ["host": "https://api.openai.com/v1"]
        let refused = L("Your organization doesn’t allow an AI server on this computer.")
        // Nobody's policy: the user's switch is for servers outside the Mac only.
        var policy = CompliancePolicy(allowExternalAI: false)
        #expect(policy.refusal(for: manifest, settings: local) == nil)
        #expect(policy.refusal(for: manifest, settings: cloud) == L("External AI is turned off in Privacy settings."))
        #expect(policy.refusal(for: manifest) == nil, "can be connected, to a local server")
        #expect(policy.refusal(for: claude) != nil)
        // The organization turned external AI off: local stays off unless it says so.
        policy.externalAIManaged = true
        #expect(policy.refusal(for: manifest, settings: local) == refused)
        #expect(policy.refusal(for: manifest) != nil)
        policy.allowLocalAI = true
        #expect(policy.refusal(for: manifest, settings: local) == nil)
        #expect(policy.refusal(for: manifest, settings: cloud) != nil)
        // AllowLocalAI false wins, even with external AI on.
        policy = CompliancePolicy(allowExternalAI: true, allowLocalAI: false, externalAIManaged: true)
        #expect(policy.refusal(for: manifest, settings: local) == refused)
        #expect(policy.refusal(for: manifest, settings: cloud) == nil)
        // AllowedPlugins first.
        policy = CompliancePolicy(allowedPlugins: ["claude"], allowExternalAI: true, allowLocalAI: true)
        #expect(policy.refusal(for: manifest, settings: local) == L("Not allowed by your privacy policy."))
    }

    @Test func anEmptySummariesModelIsCheckedAsTheBriefs() {
        let policy = AssistantPolicy(allowedModels: ["gpt-a", "gpt-b"])
        #expect(policy.constrained(["model": "gpt-b"], defaults: [:]) == ["model": "gpt-b"])
        #expect(policy.constrained(["model": "gpt-z"], defaults: [:])["model"] == "gpt-a")
        #expect(
            policy.constrained(["model": "gpt-b", "digestModel": "gpt-z"], defaults: [:])["digestModel"] == "gpt-a")
    }
}

import Foundation
import Testing
@testable import RemoraCore

/// Sources kept away from the assistant, and models limited by the organization.
struct AssistantPolicyTests {
    func item(_ id: String, _ pluginID: String) -> InboxItem {
        InboxItem(id: id, accountID: UUID(), pluginID: pluginID, bundle: .reviews, title: id, context: "c", date: .now)
    }

    @Test func excludedSourcesNeverReachTheAssistant() {
        let policy = AssistantPolicy(excludedSources: ["slack", "reminders"])
        let items = [item("a", "github"), item("b", "slack"), item("c", "reminders"), item("d", "linear")]
        #expect(policy.items(items).map(\.id) == ["a", "d"])
        #expect(AssistantPolicy().items(items).count == 4)
    }

    @Test func modelsStayWithinTheAllowedList() {
        let defaults = ["model": "claude-opus-5-5", "digestModel": "claude-haiku-5-5"]
        let policy = AssistantPolicy(allowedModels: ["claude-sonnet-5-5", "claude-haiku-5-5"])
        // Opus by default isn't allowed: the first allowed model replaces it; Haiku is allowed and stays.
        let settings = policy.constrained(["model": "", "token": "k"], defaults: defaults)
        #expect(settings["model"] == "claude-sonnet-5-5")
        #expect(settings["digestModel"] == nil, "empty means Haiku, which is allowed: left as is")
        #expect(policy.constrained(["model": "claude-opus-5-5", "digestModel": "claude-haiku-5-5"], defaults: defaults)
                == ["model": "claude-sonnet-5-5", "digestModel": "claude-haiku-5-5"])
        #expect(settings["token"] == "k")
        // Claude Code's empty model means its own default, which nobody can vouch for: it becomes the first allowed.
        #expect(policy.constrained([:], defaults: [:])["model"] == "claude-sonnet-5-5")
    }

    @Test func withoutAnAllowListNothingChanges() {
        let settings = ["model": "claude-opus-5-5"]
        #expect(AssistantPolicy().constrained(settings, defaults: [:]) == settings)
        #expect(AssistantPolicy(allowedModels: [" ", ""]).constrained(settings, defaults: [:]) == settings)
    }
}

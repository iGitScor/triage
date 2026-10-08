import Foundation
import Testing
import RemoraCore
@testable import RemoraPlugins

struct TriageTests {
    let now = Date(timeIntervalSince1970: 1_800_000_000)
    var items: [SnoozedItem] {
        ["a", "b", "c"].map { id in
            let item = InboxItem(id: id, accountID: UUID(), pluginID: "github", bundle: .reviews, title: "Item \(id)", context: "acme/app", date: now)
            return SnoozedItem(item: item, snooze: Snooze(until: now + 3_600, mode: .hide, fingerprint: item.fingerprint, reason: .motivation), times: 3)
        }
    }

    @Test func parsesSuggestionsAndDropsInvalidOnes() async throws {
        let later = (now + 86_400).ISO8601Format()
        let output = """
        {"is_error": false, "structured_output": {"suggestions": [
          {"id": "a", "action": "reschedule", "until": "\(later)", "reason": "After Erin's review"},
          {"id": "b", "action": "done", "until": "", "reason": "Merged elsewhere"},
          {"id": "c", "action": "reschedule", "until": "yesterday", "reason": "bad date"},
          {"id": "ghost", "action": "now", "until": "", "reason": "unknown item"}
        ]}}
        """
        let runner = RecordingRunner(output: output)
        let plugin = try ClaudeCodePlugin(config: config(["path": "/bin/echo"]), runner: runner)
        let suggestions = try await plugin.triage(items, now: now)

        #expect(suggestions.map(\.id) == ["a", "b"])
        #expect(suggestions[0].action == .reschedule && suggestions[0].until == Date(iso8601: later))
        #expect(suggestions[1].action == .done && suggestions[1].until == nil)
        let arguments = await runner.arguments
        let message = arguments[arguments.firstIndex(of: "--print")! + 1]
        #expect(message.contains(#""timesSnoozed":3"#) && message.contains(#""reason":"motivation""#))
        #expect(arguments[arguments.firstIndex(of: "--json-schema")! + 1].contains(#""enum":["keep","reschedule","done","now"]"#))
    }

    /// Real Claude Code. Opt in with REMORA_LIVE_CLAUDE=1.
    @Test(.enabled(if: ProcessInfo.processInfo.environment["REMORA_LIVE_CLAUDE"] == "1"))
    func liveTriage() async throws {
        let plugin = try ClaudeCodePlugin(config: config([:]), http: URLSessionHTTPClient())
        let suggestions = try await plugin.triage(items, now: .now)
        print("[triage]", suggestions.map { "\($0.id): \($0.action.rawValue) — \($0.reason)" })
        #expect(!suggestions.isEmpty)
    }
}

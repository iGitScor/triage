import Foundation
import Testing
import RemoraCore
@testable import RemoraPlugins

struct ClaudeCodePluginTests {
    let item = InboxItem(id: "a", accountID: UUID(), pluginID: "github", bundle: .reviews,
                         title: "Fix login", context: "acme/app #9", date: .now)

    @Test func runsHeadlessWithoutToolsOrCustomizations() {
        let arguments = ClaudeCodePlugin.arguments(items: [item], model: "", now: .now)
        #expect(arguments.contains("--safe-mode"))
        #expect(arguments.contains("--no-session-persistence"))
        #expect(arguments[arguments.firstIndex(of: "--tools")! + 1] == "")
        #expect(arguments[arguments.firstIndex(of: "--json-schema")! + 1].contains(#""additionalProperties":false"#))
        #expect(!arguments.contains("--model"))
    }

    @Test func readsStructuredOutput() throws {
        let json = #"{"type": "result", "is_error": false, "structured_output": {"summary": "Review first.", "focus": [{"id": "a", "reason": "Blocks Erin"}, {"id": "x", "reason": "?"}]}}"#
        let brief = try ClaudeCodePlugin.parse(Data(json.utf8), knownIDs: ["a"])
        #expect(brief.summary == "Review first.")
        #expect(brief.focus.map(\.id) == ["a"])
    }

    @Test func surfacesCLIErrors() {
        let json = #"{"type": "result", "is_error": true, "result": "Not logged in"}"#
        #expect(throws: HTTPError.api("Claude Code: Not logged in")) {
            try ClaudeCodePlugin.parse(Data(json.utf8), knownIDs: [])
        }
    }

    /// Calls the real Claude Code on this Mac. Opt in with REMORA_LIVE_CLAUDE=1 (uses a little of your plan).
    @Test(.enabled(if: ProcessInfo.processInfo.environment["REMORA_LIVE_CLAUDE"] == "1"))
    func liveBrief() async throws {
        let plugin = try ClaudeCodePlugin(config: config([:]), http: URLSessionHTTPClient())
        let brief = try await plugin.brief([item], now: .now)
        #expect(!brief.summary.isEmpty)
        #expect(brief.focus.allSatisfy { $0.id == "a" })
    }

    @Test func digestUsesTheLighterModel() async throws {
        let runner = RecordingRunner(output: #"{"is_error": false, "structured_output": {"summary": "Two reviews wait; one is blocking."}}"#)
        let plugin = try ClaudeCodePlugin(config: config(["path": "/bin/echo"]), runner: runner)
        let summary = try await plugin.digest([item, item], topic: "To review", now: .now)
        #expect(summary == "Two reviews wait; one is blocking.")
        let arguments = await runner.arguments
        #expect(arguments[arguments.firstIndex(of: "--model")! + 1] == "claude-haiku-5-5")
        #expect(arguments[arguments.firstIndex(of: "--print")! + 1].hasPrefix("Group: To review."))
    }

    /// Calls the real Claude Code with Haiku. Opt in with REMORA_LIVE_CLAUDE=1.
    @Test(.enabled(if: ProcessInfo.processInfo.environment["REMORA_LIVE_CLAUDE"] == "1"))
    func liveDigest() async throws {
        let plugin = try ClaudeCodePlugin(config: config([:]), http: URLSessionHTTPClient())
        let summary = try await plugin.digest([item], topic: "To review", now: .now)
        #expect(!summary.isEmpty)
    }
}

actor RecordingRunner: CommandRunner {
    let output: String
    private(set) var arguments: [String] = []

    init(output: String) { self.output = output }

    func run(_ executable: URL, arguments: [String]) async throws -> Data {
        self.arguments = arguments
        return Data(output.utf8)
    }
}

import Foundation
import Testing
import RemoraCore
@testable import RemoraPlugins

struct ClaudePluginTests {
    let item = InboxItem(id: "a", accountID: UUID(), pluginID: "github", bundle: .reviews,
                         title: "Fix login", context: "acme/app #9", date: .now)

    @Test func requestUsesStructuredOutputAndFallbacks() throws {
        let body = ClaudePlugin.body(items: [item], model: "claude-opus-5-5", now: .now)
        let json = try #require(String(data: JSONEncoder().encode(body), encoding: .utf8))
        #expect(json.contains(#""fallbacks":"default""#))
        #expect(json.contains(#""type":"json_schema""#))
        #expect(json.contains(#""additionalProperties":false"#))
        #expect(json.contains("Fix login"))
    }

    @Test func parsesBriefAndDropsUnknownItems() async throws {
        let text = #"{\"summary\": \"One review waits.\", \"focus\": [{\"id\": \"a\", \"reason\": \"Blocks Frank\"}, {\"id\": \"ghost\", \"reason\": \"?\"}]}"#
        let http = StubHTTP(routes: ["/v1/messages": #"{"content": [{"type": "thinking"}, {"type": "text", "text": "\#(text)"}], "stop_reason": "end_turn"}"#])
        let brief = try await ClaudePlugin(config: config(["token": "k"]), http: http).brief([item], now: .now)
        #expect(brief.summary == "One review waits.")
        #expect(brief.focus == [Brief.Focus(id: "a", reason: "Blocks Frank")])
    }

    @Test func refusalBecomesAnError() async throws {
        let http = StubHTTP(routes: ["/v1/messages": #"{"content": [], "stop_reason": "refusal"}"#])
        await #expect(throws: HTTPError.self) {
            try await ClaudePlugin(config: config(["token": "k"]), http: http).brief([item], now: .now)
        }
    }
}

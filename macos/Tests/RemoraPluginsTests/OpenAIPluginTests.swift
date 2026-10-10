import Foundation
import RemoraCore
import Testing

@testable import RemoraPlugins

/// AI-23: the OpenAI-compatible assistant.
struct OpenAIPluginTests {
    let item = InboxItem(
        id: "a", accountID: UUID(), pluginID: "github", bundle: .reviews,
        title: "Fix login", context: "acme/app #9", date: .now)

    /// Answers with each status in turn (200 once they run out), and keeps the requests.
    final class Server: HTTPClient, @unchecked Sendable {
        var statuses: [Int]
        var body: String
        var requests: [URLRequest] = []
        var failure: URLError?

        init(
            _ statuses: [Int] = [],
            body: String = Server.answer(
                #"{\"summary\": \"One review waits.\", \"focus\": [{\"id\": \"a\", \"reason\": \"Blocks Frank\"}, {\"id\": \"ghost\", \"reason\": \"?\"}]}"#
            )
        ) {
            self.statuses = statuses
            self.body = body
        }

        static func answer(_ content: String, finish: String = "stop", refusal: String = "null") -> String {
            #"{"choices": [{"message": {"content": "\#(content)", "refusal": \#(refusal)}, "finish_reason": "\#(finish)"}]}"#
        }

        func send(_ request: URLRequest) async throws -> (Data, HTTPURLResponse) {
            requests.append(request)
            if let failure { throw failure }
            let status = statuses.isEmpty ? 200 : statuses.removeFirst()
            return (
                Data(body.utf8),
                HTTPURLResponse(url: request.url!, statusCode: status, httpVersion: nil, headerFields: nil)!
            )
        }

        var json: [String: Any] {
            (try? JSONSerialization.jsonObject(with: requests.last?.httpBody ?? Data())) as? [String: Any] ?? [:]
        }
    }

    func plugin(_ values: [String: String], _ server: Server) throws -> OpenAIPlugin {
        var plugin = try OpenAIPlugin(config: config(values), http: server)
        plugin.sleep = { _ in }
        return plugin
    }

    @Test func asksChatCompletionsWithAStrictSchema() async throws {
        let server = Server()
        let brief = try await plugin(["token": "sk", "model": "gpt-x"], server).brief([item], now: .now)
        #expect(brief.summary == "One review waits.")
        #expect(brief.focus == [Brief.Focus(id: "a", reason: "Blocks Frank")], "made-up items dropped")

        let request = try #require(server.requests.first)
        #expect(request.url?.absoluteString == "https://api.openai.com/v1/chat/completions")
        #expect(request.value(forHTTPHeaderField: "Authorization") == "Bearer sk")
        #expect(request.value(forHTTPHeaderField: "api-key") == nil)
        #expect(request.timeoutInterval == ClaudePlugin.timeout)
        let json = server.json
        #expect(json["model"] as? String == "gpt-x")
        #expect((json["messages"] as? [[String: Any]])?.map { $0["role"] as? String } == ["system", "user"])
        let format = try #require(json["response_format"] as? [String: Any])
        #expect(format["type"] as? String == "json_schema")
        let schema = try #require(format["json_schema"] as? [String: Any])
        #expect(schema["name"] as? String == "brief" && schema["strict"] as? Bool == true)
        #expect((schema["schema"] as? [String: Any])?["additionalProperties"] as? Bool == false)
        #expect(json["max_tokens"] == nil && json["max_completion_tokens"] == nil, "nothing some servers refuse")
    }

    @Test func azureTakesItsKeyInApiKey() async throws {
        let server = Server()
        _ = try await plugin(["host": "https://acme.openai.azure.com/openai/v1/", "token": "k", "model": "m"], server)
            .brief([item], now: .now)
        let request = try #require(server.requests.first)
        #expect(request.url?.absoluteString == "https://acme.openai.azure.com/openai/v1/chat/completions")
        #expect(request.value(forHTTPHeaderField: "api-key") == "k")
        #expect(request.value(forHTTPHeaderField: "Authorization") == nil)
    }

    @Test func aServerOnThisComputerNeedsNoKeyAndMayUseHTTP() async throws {
        let server = Server()
        _ = try await plugin(["host": "http://localhost:11434/v1", "model": "llama"], server).brief([item], now: .now)
        let request = try #require(server.requests.first)
        #expect(request.url?.absoluteString == "http://localhost:11434/v1/chat/completions")
        #expect(request.value(forHTTPHeaderField: "Authorization") == nil)
    }

    @Test func otherServersNeedHTTPSAndAKey() {
        #expect(throws: HTTPError.api(L("Only a server on this computer can use http: use https."))) {
            try OpenAIPlugin(
                config: config(["host": "http://ai.acme.io/v1", "token": "k", "model": "m"]), http: Server())
        }
        #expect(throws: HTTPError.api(L("An API key is needed for this server."))) {
            try OpenAIPlugin(config: config(["model": "m"]), http: Server())
        }
        #expect(throws: PluginError.missingField("model")) {
            try OpenAIPlugin(config: config(["token": "k"]), http: Server())
        }
    }

    @Test func summariesUseTheirModelOrTheBriefs() async throws {
        let server = Server(body: Server.answer(#"{\"summary\": \"Two reviews.\"}"#))
        #expect(
            try await plugin(["token": "k", "model": "big"], server).digest([item], topic: "t", now: .now)
                == "Two reviews.")
        #expect(server.json["model"] as? String == "big")
        #expect(
            ((server.json["response_format"] as? [String: Any])?["json_schema"] as? [String: Any])?["name"] as? String
                == "digest")
        _ = try await plugin(["token": "k", "model": "big", "digestModel": "small"], server).digest(
            [item], topic: "t", now: .now)
        #expect(server.json["model"] as? String == "small")
    }

    @Test func refusalsCutOffAndStrangeAnswersSaySo() async throws {
        let refused = Server(body: Server.answer("", refusal: #""No.""#))
        await #expect(throws: HTTPError.api(L("The assistant declined this request."))) {
            try await plugin(["token": "k", "model": "m"], refused).brief([item], now: .now)
        }
        let cut = Server(body: Server.answer(#"{\"summary\": \"One rev"#, finish: "length"))
        await #expect(throws: HTTPError.api(L("The assistant’s answer was cut off. Try again, or with fewer items."))) {
            try await plugin(["token": "k", "model": "m"], cut).brief([item], now: .now)
        }
        let strange = Server(body: Server.answer("Sure! Here is your brief."))
        await #expect(throws: HTTPError.api(L("The assistant returned an unexpected answer."))) {
            try await plugin(["token": "k", "model": "m"], strange).brief([item], now: .now)
        }
        let slow = Server()
        slow.failure = URLError(.timedOut)
        await #expect(throws: HTTPError.api(L("The assistant took too long to answer. Try again later."))) {
            try await plugin(["token": "k", "model": "m"], slow).brief([item], now: .now)
        }
        #expect(slow.requests.count == 1, "a timeout isn't retried")
    }

    @Test func overloadedAnswersAreRetriedLikeClaudes() async throws {
        let server = Server([529, 429])
        #expect(
            try await plugin(["token": "k", "model": "m"], server).brief([item], now: .now).summary
                == "One review waits.")
        #expect(server.requests.count == 3)
        let rejected = Server([401])
        await #expect(throws: HTTPError.unauthorized) {
            try await plugin(["token": "k", "model": "m"], rejected).brief([item], now: .now)
        }
        #expect(rejected.requests.count == 1)
    }

    @Test func itsServerIsCheckedByThePolicy() {
        #expect(OpenAIPlugin.manifest.egress.serverField == "host")
        #expect(OpenAIPlugin.manifest.egress.externalAI && OpenAIPlugin.manifest.egress.hosts.isEmpty)
        #expect(PluginRegistry.isAssistant("openai"))
    }
}

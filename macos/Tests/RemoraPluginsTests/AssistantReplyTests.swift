import Foundation
import RemoraCore
import Testing

@testable import RemoraPlugins

/// AI-15: every assistant names why an answer is missing through `AssistantFailure`. Same cases on Windows.
struct AssistantReplyTests {
    private func failure(_ reply: some AssistantReply) -> AssistantFailure? {
        do {
            _ = try reply.answer()
            return nil
        } catch {
            return error
        }
    }

    private func claudeAPI(_ json: String) throws -> ClaudePlugin.Response {
        try JSONDecoder().decode(ClaudePlugin.Response.self, from: Data(json.utf8))
    }

    private func openAI(_ json: String) throws -> OpenAIPlugin.Response {
        try JSONDecoder().decode(OpenAIPlugin.Response.self, from: Data(json.utf8))
    }

    private func claudeCode(_ json: String) throws -> ClaudeCodePlugin.Result {
        try #require(ClaudeCodePlugin.Result(Data(json.utf8)))
    }

    @Test func claudeAPI() throws {
        #expect(failure(try claudeAPI(#"{"content": [], "stop_reason": "max_tokens"}"#)) == .cutOff)
        #expect(failure(try claudeAPI(#"{"content": [], "stop_reason": "refusal"}"#)) == .declined)
        #expect(failure(try claudeAPI(#"{"content": [], "stop_reason": "end_turn"}"#)) == .unexpectedAnswer)
        let answer = try claudeAPI(#"{"content": [{"type": "text", "text": "{\"summary\": \"s\"}"}]}"#)
        #expect(try answer.decode(BriefPrompt.DigestOutput.self).summary == "s")
    }

    @Test func claudeCode() throws {
        func code(_ subtype: String) throws -> ClaudeCodePlugin.Result {
            try claudeCode(#"{"is_error": true, "subtype": "\#(subtype)", "result": ""}"#)
        }
        #expect(failure(try code("error_max_structured_output_retries")) == .noStructuredAnswer)
        #expect(failure(try code("error_max_turns")) == .tooManySteps)
        #expect(failure(try code("error_max_budget_usd")) == .spendingLimit)
        #expect(
            failure(try claudeCode(#"{"is_error": true, "subtype": "error_during_execution", "result": "Boom"}"#))
                == .failed("Claude Code: Boom"))
        #expect(
            failure(try claudeCode(#"{"is_error": true, "result": "Not logged in · Please run /login"}"#))
                == .failed(L("Claude Code isn’t signed in. Run `claude` in Terminal once to sign in.")))
        #expect(failure(try claudeCode(#"{"is_error": false, "subtype": "success"}"#)) == .unexpectedAnswer)
        let answer = try claudeCode(#"{"is_error": false, "structured_output": {"summary": "s"}}"#)
        #expect(try answer.decode(BriefPrompt.DigestOutput.self).summary == "s")
    }

    @Test func openAICompatible() throws {
        #expect(failure(try openAI(#"{"choices": [{"message": {"refusal": "No"}}]}"#)) == .declined)
        #expect(
            failure(try openAI(#"{"choices": [{"message": {"content": "{"}, "finish_reason": "length"}]}"#)) == .cutOff)
        #expect(failure(try openAI(#"{"choices": []}"#)) == .unexpectedAnswer)
        let answer = try openAI(#"{"choices": [{"message": {"content": "{\"summary\": \"s\"}"}}]}"#)
        #expect(try answer.decode(BriefPrompt.DigestOutput.self).summary == "s")
    }

    /// AI-03: each provider's `usage`, cached prompt tokens counted as input; reported even for an answer cut off.
    @Test func usageIsReadFromEveryProvider() throws {
        let claude = try claudeAPI(
            #"{"content": [], "stop_reason": "max_tokens", "usage": {"input_tokens": 100, "cache_creation_input_tokens": 20, "cache_read_input_tokens": 3, "output_tokens": 50}}"#
        )
        #expect(claude.usage == TokenUsage(input: 123, output: 50))
        #expect(try claudeAPI(#"{"content": []}"#).usage == nil)
        let code = try claudeCode(
            #"{"is_error": false, "structured_output": {"summary": "s"}, "usage": {"input_tokens": 7, "cache_read_input_tokens": 1000, "output_tokens": 30}}"#
        )
        #expect(code.usage == TokenUsage(input: 1_007, output: 30))
        #expect(try claudeCode(#"{"is_error": false}"#).usage == nil)
        let compatible = try openAI(#"{"choices": [], "usage": {"prompt_tokens": 40, "completion_tokens": 9}}"#)
        #expect(compatible.usage == TokenUsage(input: 40, output: 9))
        #expect(try openAI(#"{"choices": []}"#).usage == nil, "a local server may say nothing")

        let meter = AssistantMeter()
        AssistantMeter.$current.withValue(meter) {
            _ = try? claude.decode(BriefPrompt.DigestOutput.self)
            _ = try? code.decode(BriefPrompt.DigestOutput.self)
        }
        #expect(meter.tokens == TokenUsage(input: 1_130, output: 80), "the cut-off answer counts too")
    }

    @Test func anAnswerThatIsntTheExpectedJSONIsUnexpected() throws {
        let answer = try openAI(#"{"choices": [{"message": {"content": "not json"}}]}"#)
        #expect(throws: HTTPError.api(AssistantFailure.unexpectedAnswer.message)) {
            try answer.decode(BriefPrompt.DigestOutput.self)
        }
    }
}

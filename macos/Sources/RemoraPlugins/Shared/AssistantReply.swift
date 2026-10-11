import Foundation
import RemoraCore

/// Why an assistant gave no usable answer, whatever the provider (Claude API, Claude Code, an OpenAI-compatible
/// server). Each cause is worded once, here; a provider only says which one applies (`AssistantReply`). Same causes
/// and text on Windows (`remora_plugins::assistant_reply`).
public enum AssistantFailure: Error, Equatable, Sendable {
    case cutOff
    case declined
    case unexpectedAnswer
    case noStructuredAnswer
    case tooManySteps
    case spendingLimit
    /// The provider's own explanation, already worded for people (e.g. Claude Code's sign-in message).
    case failed(String)

    public var message: String {
        switch self {
        case .cutOff: L("The assistant’s answer was cut off. Try again, or with fewer items.")
        case .declined: L("The assistant declined this request.")
        case .unexpectedAnswer: L("The assistant returned an unexpected answer.")
        case .noStructuredAnswer:
            L("The assistant couldn’t answer in the expected form. Try again, or with fewer items.")
        case .tooManySteps: L("The assistant stopped before answering. Try again.")
        case .spendingLimit: L("The assistant reached its spending limit.")
        case .failed(let message): message
        }
    }
}

/// What a provider sent back: the answer as JSON, or why there is none. Each provider's wire type implements it;
/// `decode` turns the answer into a brief, a digest or triage suggestions the same way for all of them.
protocol AssistantReply {
    func answer() throws(AssistantFailure) -> Data
}

extension AssistantReply {
    /// The answer as `Output`; a failure is shown like any other API error, with its cause's message.
    func decode<Output: Decodable>(_: Output.Type = Output.self) throws -> Output {
        do throws(AssistantFailure) {
            let data = try answer()
            guard let output = try? JSONDecoder().decode(Output.self, from: data) else {
                throw AssistantFailure.unexpectedAnswer
            }
            return output
        } catch {
            throw HTTPError.api(error.message)
        }
    }
}

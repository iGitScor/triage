import Foundation
import RemoraCore

/// The assistant through any server that speaks OpenAI's Chat Completions: OpenAI, Azure OpenAI, Mistral, or one on
/// this computer such as Ollama (AI-23). The same prompts and schemas as the Claude API, as a strict JSON schema.
public struct OpenAIPlugin: AssistantPlugin {
    public static let manifest = PluginManifest(
        id: "openai",
        name: "OpenAI-compatible",
        symbol: "sparkles",
        summary:
            "The brief through OpenAI, Azure OpenAI, Mistral or a server on this computer. Item titles and context are sent to that server.",
        fields: [
            ConfigField(
                key: "host", label: "Server", placeholder: "https://api.openai.com/v1",
                defaultValue: "https://api.openai.com/v1",
                help:
                    "The API's address, up to /v1. A server on this computer can use http, e.g. http://localhost:11434/v1."
            ),
            ConfigField(
                key: "token", label: "API key", isSecret: true,
                help: "From your provider. Not needed for a server on this computer."),
            ConfigField(key: "model", label: "Model for the brief", placeholder: "Model id from your provider"),
            ConfigField(
                key: "digestModel", label: "Model for bundle summaries", placeholder: "Same as the brief",
                isOptional: true),
        ],
        setupSteps: [
            "Create an API key with your provider, or start the server on this computer.",
            "Paste the server address, the key and the model id below.",
        ],
        egress: Egress(
            hosts: [],
            description: "Sends the titles, contexts, authors and statuses of inbox items to the server you set.",
            externalAI: true,
            serverField: "host"
        )
    )

    private let server: URL
    private let apiKey: String
    private let model: String
    private let digestModel: String
    private let http: HTTPClient
    /// How a retry waits; tests replace it to run instantly.
    var sleep: @Sendable (TimeInterval) async throws -> Void = { try await Task.sleep(for: .seconds($0)) }

    public init(config: PluginConfig, http: HTTPClient) throws {
        var config = config
        if config["host"].isEmpty { config.values["host"] = Self.manifest.server(in: [:]) }
        do {
            server = try config.url("host")
        } catch PluginError.insecureField {
            throw HTTPError.api(L("Only a server on this computer can use http: use https."))
        }
        apiKey = config["token"]
        if apiKey.isEmpty, !PluginConfig.isLoopback(server.host ?? "") {
            throw HTTPError.api(L("An API key is needed for this server."))
        }
        model = try config.required("model")
        digestModel = config["digestModel"].nilIfEmpty ?? model
        self.http = http
    }

    public func brief(_ items: [InboxItem], now: Date) async throws -> Brief {
        let output: BriefPrompt.Output = try await ask(
            model: model, system: BriefPrompt.system, message: BriefPrompt.message(items: items, now: now),
            schema: ("brief", .brief))
        return output.brief(knownIDs: Set(items.map(\.id)))
    }

    public func digest(_ items: [InboxItem], topic: String, now: Date) async throws -> String {
        let output: BriefPrompt.DigestOutput = try await ask(
            model: digestModel, system: BriefPrompt.digestSystem,
            message: BriefPrompt.digestMessage(items: items, topic: topic, now: now), schema: ("digest", .digest))
        return output.summary
    }

    public func triage(_ items: [SnoozedItem], now: Date) async throws -> [TriageSuggestion] {
        let output: BriefPrompt.TriageOutput = try await ask(
            model: model, system: BriefPrompt.triageSystem, message: BriefPrompt.triageMessage(items: items, now: now),
            schema: ("triage", .triage))
        return output.suggestions(knownIDs: Set(items.map(\.item.id)), now: now)
    }

    /// Azure takes its key in `api-key`; everyone else in `Authorization`. A server on this computer may need none.
    var headers: [String: String] {
        guard !apiKey.isEmpty else { return [:] }
        let host = server.host?.lowercased() ?? ""
        let azure = [".openai.azure.com", ".services.ai.azure.com"].contains { host.hasSuffix($0) }
        return azure ? ["api-key": apiKey] : ["Authorization": "Bearer \(apiKey)"]
    }

    var endpoint: URL { server.appending(path: "chat/completions") }

    static func body(model: String, system: String, message: String, schema: (String, BriefPrompt.Schema)) -> Request {
        Request(
            model: model,
            messages: [.init(role: "system", content: system), .init(role: "user", content: message)],
            responseFormat: .init(
                type: "json_schema", jsonSchema: .init(name: schema.0, strict: true, schema: schema.1))
        )
    }

    private func ask<Output: Decodable>(
        model: String, system: String, message: String, schema: (String, BriefPrompt.Schema)
    ) async throws -> Output {
        let body = Self.body(model: model, system: system, message: message, schema: schema)
        let request = try URLRequest.post(endpoint, json: body, headers: headers, timeout: ClaudePlugin.timeout)
        let response = try await AssistantRetry.decode(
            Response.self, from: request, http: http,
            tooSlow: L("The assistant took too long to answer. Try again later."), sleep: sleep)
        return try Self.output(of: response)
    }

    static func output<Output: Decodable>(of response: Response) throws -> Output {
        let choice = response.choices.first
        if let refusal = choice?.message.refusal, !refusal.isEmpty {
            throw HTTPError.api(L("The assistant declined this request."))
        }
        if choice?.finishReason == "length" {
            throw HTTPError.api(L("The assistant’s answer was cut off. Try again, or with fewer items."))
        }
        guard let text = choice?.message.content,
            let output = try? JSONDecoder().decode(Output.self, from: Data(text.utf8))
        else {
            throw HTTPError.api(L("The assistant returned an unexpected answer."))
        }
        return output
    }
}

// MARK: - Wire format

extension OpenAIPlugin {
    struct Request: Encodable {
        struct Message: Encodable {
            var role: String
            var content: String
        }
        struct ResponseFormat: Encodable {
            struct JSONSchema: Encodable {
                var name: String
                var strict: Bool
                var schema: BriefPrompt.Schema
            }
            var type: String
            var jsonSchema: JSONSchema

            enum CodingKeys: String, CodingKey {
                case type
                case jsonSchema = "json_schema"
            }
        }

        var model: String
        var messages: [Message]
        var responseFormat: ResponseFormat

        enum CodingKeys: String, CodingKey {
            case model, messages
            case responseFormat = "response_format"
        }
    }

    struct Response: Decodable {
        struct Choice: Decodable {
            struct Message: Decodable {
                var content: String?
                var refusal: String?
            }
            var message: Message
            var finishReason: String?

            enum CodingKeys: String, CodingKey {
                case message
                case finishReason = "finish_reason"
            }
        }
        var choices: [Choice]
    }
}

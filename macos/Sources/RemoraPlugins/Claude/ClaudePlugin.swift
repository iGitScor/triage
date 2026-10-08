import Foundation
import RemoraCore

/// Asks Claude for a short brief: what matters now and why.
public struct ClaudePlugin: AssistantPlugin {
    public static let manifest = PluginManifest(
        id: "claude",
        name: "Claude API",
        symbol: "sparkles",
        summary: "The brief through an Anthropic API key. Item titles and context are sent to Anthropic.",
        fields: [
            .token("API key", help: "An Anthropic API key from the Claude Console."),
            ConfigField(key: "model", label: "Model for the brief", defaultValue: "claude-opus-5-5"),
            ConfigField(key: "digestModel", label: "Model for bundle summaries", defaultValue: "claude-haiku-5-5"),
        ],
        setupSteps: [
            "Click “Create an API key” and sign in to the Claude Console.",
            "Create a key and paste it below.",
        ],
        setupLabel: "Create an API key",
        setupURL: { _ in URL(string: "https://platform.claude.com/settings/keys") },
        egress: Egress(
            hosts: ["api.anthropic.com"],
            description: "Sends the titles, contexts, authors and statuses of inbox items to Anthropic.",
            externalAI: true
        )
    )

    private let apiKey: String
    private let model: String
    private let digestModel: String
    private let http: HTTPClient
    private let endpoint = URL(string: "https://api.anthropic.com/v1/messages")!

    public init(config: PluginConfig, http: HTTPClient) throws {
        apiKey = try config.required("token")
        model = config["model"].nilIfEmpty ?? "claude-opus-5-5"
        digestModel = config["digestModel"].nilIfEmpty ?? "claude-haiku-5-5"
        self.http = http
    }

    public func brief(_ items: [InboxItem], now: Date) async throws -> Brief {
        let response = try await send(Self.body(items: items, model: model, now: now))
        return try Self.parse(response, knownIDs: Set(items.map(\.id)))
    }

    public func digest(_ items: [InboxItem], topic: String, now: Date) async throws -> String {
        let body = Request(
            model: digestModel,
            maxTokens: 2_000,
            system: BriefPrompt.digestSystem,
            messages: [.init(role: "user", content: BriefPrompt.digestMessage(items: items, topic: topic, now: now))],
            outputConfig: .init(effort: "low", format: .init(type: "json_schema", schema: .digest)),
            fallbacks: nil
        )
        let output: BriefPrompt.DigestOutput = try Self.output(of: try await send(body))
        return output.summary
    }

    public func triage(_ items: [SnoozedItem], now: Date) async throws -> [TriageSuggestion] {
        let body = Request(
            model: model,
            maxTokens: 8_000,
            system: BriefPrompt.triageSystem,
            messages: [.init(role: "user", content: BriefPrompt.triageMessage(items: items, now: now))],
            outputConfig: .init(effort: "low", format: .init(type: "json_schema", schema: .triage)),
            fallbacks: "default"
        )
        let output: BriefPrompt.TriageOutput = try Self.output(of: try await send(body))
        return output.suggestions(knownIDs: Set(items.map(\.item.id)), now: now)
    }

    private func send(_ body: Request) async throws -> Response {
        var headers = ["x-api-key": apiKey, "anthropic-version": "2023-06-01"]
        if body.fallbacks != nil { headers["anthropic-beta"] = "server-side-fallback-2026-07-01" }
        return try await http.decode(Response.self, from: try URLRequest.post(endpoint, json: body, headers: headers))
    }

    static func body(items: [InboxItem], model: String, now: Date) -> Request {
        Request(
            model: model,
            maxTokens: 16_000,
            system: BriefPrompt.system,
            messages: [.init(role: "user", content: BriefPrompt.message(items: items, now: now))],
            outputConfig: .init(effort: "low", format: .init(type: "json_schema", schema: .brief)),
            fallbacks: "default"
        )
    }

    static func parse(_ response: Response, knownIDs: Set<String>) throws -> Brief {
        let output: BriefPrompt.Output = try output(of: response)
        return output.brief(knownIDs: knownIDs)
    }

    private static func output<Output: Decodable>(of response: Response) throws -> Output {
        if response.stopReason == "refusal" { throw HTTPError.api(L("Claude declined this request.")) }
        guard let text = response.content.first(where: { $0.type == "text" })?.text,
              let output = try? JSONDecoder().decode(Output.self, from: Data(text.utf8)) else {
            throw HTTPError.api(L("Claude returned an unexpected answer."))
        }
        return output
    }
}

// MARK: - Wire format

extension ClaudePlugin {
    struct Request: Encodable {
        struct Message: Encodable { var role: String; var content: String }
        struct OutputConfig: Encodable {
            struct Format: Encodable { var type: String; var schema: BriefPrompt.Schema }
            var effort: String
            var format: Format
        }

        var model: String
        var maxTokens: Int
        var system: String
        var messages: [Message]
        var outputConfig: OutputConfig
        /// Server-side fallback on refusal; not available for Haiku.
        var fallbacks: String?

        enum CodingKeys: String, CodingKey {
            case model, system, messages, fallbacks
            case maxTokens = "max_tokens"
            case outputConfig = "output_config"
        }
    }

    struct Response: Decodable {
        struct Block: Decodable { var type: String; var text: String? }
        var content: [Block]
        var stopReason: String?

        enum CodingKeys: String, CodingKey { case content, stopReason = "stop_reason" }
    }
}

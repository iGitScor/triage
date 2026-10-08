import Foundation
import RemoraCore

/// The brief through the Claude Code already signed in on this Mac: no API key needed.
public struct ClaudeCodePlugin: AssistantPlugin {
    public static let manifest = PluginManifest(
        id: "claude-code",
        name: "Claude Code",
        symbol: "terminal",
        summary: "The brief through the Claude Code signed in on this Mac, on your Claude plan. No API key.",
        fields: [
            ConfigField(key: "path", label: "Path to claude", placeholder: "Found automatically", isOptional: true,
                        help: "Only if Claude Code is installed somewhere unusual (run `which claude` in Terminal)."),
            ConfigField(key: "model", label: "Model for the brief", placeholder: "Claude Code’s default", isOptional: true),
            ConfigField(key: "digestModel", label: "Model for bundle summaries", defaultValue: "claude-haiku-5-5",
                        help: "A lighter model is enough for two sentences and uses less of your plan."),
        ],
        setupSteps: [
            "Install Claude Code and sign in once by running `claude` in Terminal.",
            "Click Connect: Remora writes a first brief to check that everything works.",
        ],
        setupLabel: "Install Claude Code",
        setupURL: { _ in URL(string: "https://claude.com/product/claude-code") },
        egress: Egress(
            hosts: [],
            description: "Runs Claude Code on this Mac, which sends the titles, contexts, authors and statuses of inbox items to Anthropic.",
            externalAI: true
        )
    )

    static let searchPaths = ["~/.local/bin/claude", "~/.claude/local/claude", "/opt/homebrew/bin/claude", "/usr/local/bin/claude"]

    private let path: String
    private let model: String
    private let digestModel: String
    private let runner: CommandRunner

    public init(config: PluginConfig, http: HTTPClient) throws {
        try self.init(config: config, runner: ProcessCommandRunner())
    }

    init(config: PluginConfig, runner: CommandRunner) throws {
        path = config["path"]
        model = config["model"]
        digestModel = config["digestModel"].nilIfEmpty ?? "claude-haiku-5-5"
        self.runner = runner
    }

    public func brief(_ items: [InboxItem], now: Date) async throws -> Brief {
        let data = try await runner.run(try executable(), arguments: Self.arguments(items: items, model: model, now: now))
        return try Self.parse(data, knownIDs: Set(items.map(\.id)))
    }

    public func digest(_ items: [InboxItem], topic: String, now: Date) async throws -> String {
        let arguments = Self.command(
            message: BriefPrompt.digestMessage(items: items, topic: topic, now: now),
            system: BriefPrompt.digestSystem,
            schema: .digest,
            model: digestModel
        )
        let output: BriefPrompt.DigestOutput = try Self.decode(try await runner.run(try executable(), arguments: arguments))
        return output.summary
    }

    public func triage(_ items: [SnoozedItem], now: Date) async throws -> [TriageSuggestion] {
        let arguments = Self.command(
            message: BriefPrompt.triageMessage(items: items, now: now),
            system: BriefPrompt.triageSystem,
            schema: .triage,
            model: model
        )
        let output: BriefPrompt.TriageOutput = try Self.decode(try await runner.run(try executable(), arguments: arguments))
        return output.suggestions(knownIDs: Set(items.map(\.item.id)), now: now)
    }

    static func arguments(items: [InboxItem], model: String, now: Date) -> [String] {
        command(message: BriefPrompt.message(items: items, now: now), system: BriefPrompt.system, schema: .brief, model: model)
    }

    /// One-shot, no tools, no session saved, and the user's plugins, hooks and MCP servers left out.
    static func command(message: String, system: String, schema: BriefPrompt.Schema, model: String) -> [String] {
        var arguments = [
            "--print", message,
            "--safe-mode",
            "--output-format", "json",
            "--no-session-persistence",
            "--tools", "",
            "--system-prompt", system,
            "--json-schema", BriefPrompt.schemaJSON(schema),
        ]
        if !model.isEmpty { arguments += ["--model", model] }
        return arguments
    }

    static func parse(_ data: Data, knownIDs: Set<String>) throws -> Brief {
        let output: BriefPrompt.Output = try decode(data)
        return output.brief(knownIDs: knownIDs)
    }

    private static func decode<Output: Decodable>(_ data: Data) throws -> Output {
        guard let result = try? JSONDecoder().decode(Result<Output>.self, from: data) else {
            throw HTTPError.api(L("Claude Code returned an unexpected answer. Run `claude` in Terminal to check it’s signed in."))
        }
        guard !result.isError, let output = result.structuredOutput else {
            throw HTTPError.api(L("Claude Code: %@", result.result ?? L("the request failed.")))
        }
        return output
    }

    private func executable() throws -> URL {
        let candidates = path.isEmpty ? Self.searchPaths : [path]
        let found = candidates
            .map { ($0 as NSString).expandingTildeInPath }
            .first { FileManager.default.isExecutableFile(atPath: $0) }
        guard let found else { throw CommandError.notFound("Claude Code (claude)") }
        return URL(fileURLWithPath: found)
    }

    struct Result<Output: Decodable>: Decodable {
        var isError: Bool
        var result: String?
        var structuredOutput: Output?

        enum CodingKeys: String, CodingKey {
            case isError = "is_error", result, structuredOutput = "structured_output"
        }
    }
}

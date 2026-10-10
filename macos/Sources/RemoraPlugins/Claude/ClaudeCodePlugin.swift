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
            ConfigField(
                key: "path", label: "Path to claude", placeholder: "Found automatically", isOptional: true,
                help: "Only if Claude Code is installed somewhere unusual (run `which claude` in Terminal)."),
            ConfigField(
                key: "model", label: "Model for the brief", placeholder: "Claude Code’s default", isOptional: true),
            ConfigField(
                key: "digestModel", label: "Model for bundle summaries", defaultValue: "claude-haiku-5-5",
                help: "A lighter model is enough for two sentences and uses less of your plan."),
        ],
        setupSteps: [
            "Install Claude Code with Anthropic’s installer (recommended: it doesn’t need Node.js), then sign in once by running `claude` in Terminal.",
            "Click Connect: Remora writes a first brief to check that everything works.",
        ],
        setupLabel: "Install Claude Code",
        setupURL: { _ in URL(string: "https://claude.com/product/claude-code") },
        egress: Egress(
            hosts: [],
            description:
                "Runs Claude Code on this Mac, which sends the titles, contexts, authors and statuses of inbox items to Anthropic.",
            externalAI: true
        )
    )

    /// Where installs put `claude`: Anthropic's installer (recommended) and its older local install, Homebrew or npm
    /// with Homebrew's Node, npm's user prefix, volta, bun, mise and asdf. nvm's folders are added newest first.
    static let searchPaths = [
        "~/.local/bin/claude", "~/.claude/local/claude",
        "/opt/homebrew/bin/claude", "/usr/local/bin/claude",
        "~/.npm-global/bin/claude", "~/.volta/bin/claude", "~/.bun/bin/claude",
        "~/.local/share/mise/shims/claude", "~/.asdf/shims/claude",
    ]

    /// nvm keeps one folder per Node version.
    static func nvmPaths(home: String = NSHomeDirectory()) -> [String] {
        let root = home + "/.nvm/versions/node"
        let versions = (try? FileManager.default.contentsOfDirectory(atPath: root)) ?? []
        return versions.sorted { $0.compare($1, options: .numeric) == .orderedDescending }.map {
            "\(root)/\($0)/bin/claude"
        }
    }

    /// The `claude` Remora runs: the one entered in the account, else the first found. Shown in Privacy.
    /// Only a program it would trust (`isTrustworthy`).
    public static func locate(_ configured: String) -> URL? {
        let candidates =
            configured.isEmpty
            ? searchPaths.map { ($0 as NSString).expandingTildeInPath } + nvmPaths()
            : [(configured as NSString).expandingTildeInPath]
        return candidates.first(where: isTrustworthy).map { URL(fileURLWithPath: $0) }
    }

    /// A program named `claude` that only you or the system can change: it, the folder it's in and, through a
    /// link, the real file and its folder belong to you or root and aren't writable by everyone. Then
    /// `isClaudeCode` asks it what it is before it gets any inbox content.
    static func isTrustworthy(_ path: String) -> Bool {
        let files = FileManager.default
        guard (path as NSString).lastPathComponent == "claude", files.isExecutableFile(atPath: path) else {
            return false
        }
        let link = URL(fileURLWithPath: path)
        let real = link.resolvingSymlinksInPath()
        return [real.path, real.deletingLastPathComponent().path, link.deletingLastPathComponent().path].allSatisfy {
            path in
            guard let attributes = try? files.attributesOfItem(atPath: path),
                let owner = (attributes[.ownerAccountID] as? NSNumber)?.uint32Value,
                let permissions = (attributes[.posixPermissions] as? NSNumber)?.intValue
            else { return false }
            return (owner == getuid() || owner == 0) && permissions & 0o002 == 0
        }
    }

    /// Programs that answered `--version` as Claude Code, with the file's date then: checked again after an update.
    private static let verified = Verified()

    /// `claude --version` prints "2.1.295 (Claude Code)".
    static func isClaudeCode(_ executable: URL, runner: CommandRunner) async -> Bool {
        let real = executable.resolvingSymlinksInPath()
        let modified = (try? FileManager.default.attributesOfItem(atPath: real.path))?[.modificationDate] as? Date
        if let modified, verified.contains(real.path, modified) { return true }
        let output = try? await runner.run(
            executable, arguments: ["--version"], environment: environment(for: executable))
        guard let output, String(decoding: output, as: UTF8.self).contains("(Claude Code)") else { return false }
        if let modified { verified.insert(real.path, modified) }
        return true
    }

    final class Verified: @unchecked Sendable {
        private let lock = NSLock()
        private var programs: [String: Date] = [:]

        func contains(_ path: String, _ modified: Date) -> Bool { lock.withLock { programs[path] == modified } }
        func insert(_ path: String, _ modified: Date) { lock.withLock { programs[path] = modified } }
    }

    /// An app opened from the Dock gets the system's bare PATH, and an npm install of `claude` is a script that starts
    /// with `env node`. Node sits next to `claude` with nvm, Homebrew and volta, so `claude`'s folders go first.
    static func environment(for executable: URL, base: [String: String] = ProcessInfo.processInfo.environment)
        -> [String: String]
    {
        let folders = [
            executable.deletingLastPathComponent().path,
            executable.resolvingSymlinksInPath().deletingLastPathComponent().path,
            "/opt/homebrew/bin", "/usr/local/bin",
        ]
        let current = (base["PATH"] ?? "/usr/bin:/bin:/usr/sbin:/sbin").split(separator: ":").map(String.init)
        var seen = Set<String>()
        var environment = base
        environment["PATH"] = (folders + current).filter { seen.insert($0).inserted }.joined(separator: ":")
        return environment
    }

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
        let data = try await run(
            Self.command(
                message: BriefPrompt.message(items: items, now: now), system: BriefPrompt.system, schema: .brief,
                model: model))
        return try Self.parse(data, knownIDs: Set(items.map(\.id)))
    }

    public func digest(_ items: [InboxItem], topic: String, now: Date) async throws -> String {
        let command = Self.command(
            message: BriefPrompt.digestMessage(items: items, topic: topic, now: now),
            system: BriefPrompt.digestSystem,
            schema: .digest,
            model: digestModel
        )
        let output: BriefPrompt.DigestOutput = try Self.decode(try await run(command))
        return output.summary
    }

    public func triage(_ items: [SnoozedItem], now: Date) async throws -> [TriageSuggestion] {
        let command = Self.command(
            message: BriefPrompt.triageMessage(items: items, now: now),
            system: BriefPrompt.triageSystem,
            schema: .triage,
            model: model
        )
        let output: BriefPrompt.TriageOutput = try Self.decode(try await run(command))
        return output.suggestions(knownIDs: Set(items.map(\.item.id)), now: now)
    }

    static func arguments(items: [InboxItem], model: String, now: Date) -> [String] {
        command(
            message: BriefPrompt.message(items: items, now: now), system: BriefPrompt.system, schema: .brief,
            model: model
        ).arguments
    }

    /// One-shot, no tools, no session saved, and the user's plugins, hooks and MCP servers left out. The message, with
    /// the inbox in it, goes on standard input; the command line only holds Remora's own fixed instructions.
    static func command(message: String, system: String, schema: BriefPrompt.Schema, model: String) -> (
        arguments: [String], input: Data
    ) {
        var arguments = [
            "--print",
            "--safe-mode",
            "--output-format", "json",
            "--no-session-persistence",
            "--tools", "",
            "--system-prompt", system,
            "--json-schema", BriefPrompt.schemaJSON(schema),
        ]
        if !model.isEmpty { arguments += ["--model", model] }
        return (arguments, Data(message.utf8))
    }

    static func parse(_ data: Data, knownIDs: Set<String>) throws -> Brief {
        let output: BriefPrompt.Output = try decode(data)
        return output.brief(knownIDs: knownIDs)
    }

    private static func decode<Output: Decodable>(_ data: Data) throws -> Output {
        guard let result = try? JSONDecoder().decode(Result<Output>.self, from: data) else {
            throw HTTPError.api(
                L("Claude Code returned an unexpected answer. Run `claude` in Terminal to check it’s signed in."))
        }
        guard !result.isError, let output = result.structuredOutput else {
            throw HTTPError.api(L("Claude Code: %@", result.result ?? L("the request failed.")))
        }
        return output
    }

    private func run(_ command: (arguments: [String], input: Data)) async throws -> Data {
        guard let executable = Self.locate(path) else {
            let expanded = (path as NSString).expandingTildeInPath
            if path.isEmpty {
                throw HTTPError.api(
                    L(
                        "Couldn’t find Claude Code. Install it with Anthropic’s installer, or enter the path to claude (run `which claude` in Terminal)."
                    ))
            } else if FileManager.default.isExecutableFile(atPath: expanded) {
                throw HTTPError.api(
                    L("Remora only runs a program named claude that only you can change, which %@ isn’t.", path))
            } else {
                throw HTTPError.api(
                    L("There is no program at %@. Run `which claude` in Terminal and paste the path it shows.", path))
            }
        }
        guard await Self.isClaudeCode(executable, runner: runner) else {
            throw HTTPError.api(
                L("%@ isn’t Claude Code. Run `which claude` in Terminal and paste the path it shows.", executable.path))
        }
        do {
            return try await runner.run(
                executable, arguments: command.arguments, environment: Self.environment(for: executable),
                input: command.input)
        } catch CommandError.failed(_, let message) {
            throw Self.explain(message)
        }
    }

    /// Turns what `claude` printed on failure into something the user can act on.
    static func explain(_ message: String) -> HTTPError {
        let lowered = message.lowercased()
        if lowered.contains("env: node") || lowered.contains("node: no such file")
            || lowered.contains("node: command not found")
        {
            return .api(
                L(
                    "Claude Code needs Node.js, which Remora can’t find. Reinstall Claude Code with Anthropic’s installer, which doesn’t need Node.js, or enter the path to claude."
                ))
        }
        if lowered.contains("not logged in") || lowered.contains("/login") || lowered.contains("please log in") {
            return .api(L("Claude Code isn’t signed in. Run `claude` in Terminal once to sign in."))
        }
        return .api(L("Claude Code: %@", message.isEmpty ? L("the request failed.") : message))
    }

    struct Result<Output: Decodable>: Decodable {
        var isError: Bool
        var result: String?
        var structuredOutput: Output?

        enum CodingKeys: String, CodingKey {
            case isError = "is_error"
            case result
            case structuredOutput = "structured_output"
        }
    }
}

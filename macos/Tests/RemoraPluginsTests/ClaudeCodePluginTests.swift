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
        let plugin = try ClaudeCodePlugin(config: config(["path": fakeClaude()]), runner: runner)
        let summary = try await plugin.digest([item, item], topic: "To review", now: .now)
        #expect(summary == "Two reviews wait; one is blocking.")
        let arguments = await runner.arguments
        #expect(arguments[arguments.firstIndex(of: "--model")! + 1] == "claude-haiku-5-5")
        // The inbox goes on standard input, never on the command line, which `ps` shows to every process.
        let input = String(decoding: await runner.input ?? Data(), as: UTF8.self)
        #expect(input.hasPrefix("Group: To review."))
        #expect(!arguments.contains { $0.contains(item.title) }, "no title on the command line")
    }

    // MARK: Finding and running claude

    @Test func runsClaudeWithItsOwnFoldersFirstOnPATH() async throws {
        let runner = RecordingRunner(output: #"{"is_error": false, "structured_output": {"summary": "ok"}}"#)
        _ = try await ClaudeCodePlugin(config: config(["path": fakeClaude()]), runner: runner).digest([item], topic: "x", now: .now)
        let path = try #require(await runner.environment?["PATH"])
        #expect(path.hasPrefix(URL(fileURLWithPath: fakeClaude()).deletingLastPathComponent().path + ":"))
    }

    /// An npm `claude` is a link into node_modules and starts with `env node`: Node sits next to the link.
    @Test func pathPutsTheLinkFolderAndItsTargetFirstWithoutDuplicates() throws {
        let root = FileManager.default.temporaryDirectory.appending(path: UUID().uuidString)
        let bin = root.appending(path: "v22/bin"), package = root.appending(path: "v22/lib/node_modules/claude-code")
        try FileManager.default.createDirectory(at: bin, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: package, withIntermediateDirectories: true)
        try Data().write(to: package.appending(path: "cli.js"))
        try FileManager.default.createSymbolicLink(at: bin.appending(path: "claude"), withDestinationURL: package.appending(path: "cli.js"))
        defer { try? FileManager.default.removeItem(at: root) }

        let environment = ClaudeCodePlugin.environment(for: bin.appending(path: "claude"), base: ["PATH": "/usr/bin:/bin:/opt/homebrew/bin", "HOME": "/Users/x"])
        let folders = try #require(environment["PATH"]).split(separator: ":").map(String.init)
        #expect(folders.first == bin.path)
        #expect(folders.contains(package.resolvingSymlinksInPath().path))
        #expect(folders.filter { $0 == "/opt/homebrew/bin" }.count == 1)
        #expect(folders.suffix(2) == ["/usr/bin", "/bin"])
        #expect(environment["HOME"] == "/Users/x")
    }

    @Test func nvmVersionsAreSearchedNewestFirst() throws {
        let home = FileManager.default.temporaryDirectory.appending(path: UUID().uuidString)
        for version in ["v9.11.2", "v22.12.0", "v18.20.4"] {
            try FileManager.default.createDirectory(at: home.appending(path: ".nvm/versions/node/\(version)/bin"), withIntermediateDirectories: true)
        }
        defer { try? FileManager.default.removeItem(at: home) }
        let versions = ClaudeCodePlugin.nvmPaths(home: home.path).map { $0.components(separatedBy: "/").dropLast(2).last ?? "" }
        #expect(versions == ["v22.12.0", "v18.20.4", "v9.11.2"])
    }

    @Test func aMissingClaudeSaysHowToFixIt() async throws {
        let runner = RecordingRunner(output: "")
        let plugin = try ClaudeCodePlugin(config: config(["path": "/nowhere/claude"]), runner: runner)
        await #expect(throws: HTTPError.api(L("There is no program at %@. Run `which claude` in Terminal and paste the path it shows.", "/nowhere/claude"))) {
            try await plugin.digest([item], topic: "x", now: .now)
        }
        #expect(ClaudeCodePlugin.locate(fakeClaude())?.path == fakeClaude())
    }

    // MARK: Only Claude Code

    @Test func onlyAProgramNamedClaudeThatOnlyYouCanChange() throws {
        #expect(!ClaudeCodePlugin.isTrustworthy("/bin/echo"), "not named claude")
        #expect(ClaudeCodePlugin.isTrustworthy(fakeClaude()))
        let open = try makeProgram(named: "claude", permissions: 0o777)
        #expect(!ClaudeCodePlugin.isTrustworthy(open), "anyone can change it")
        let shared = try makeProgram(named: "claude", permissions: 0o755, folderPermissions: 0o777)
        #expect(!ClaudeCodePlugin.isTrustworthy(shared), "anyone can replace it")
    }

    @Test func aProgramThatIsntClaudeCodeGetsNothing() async throws {
        let runner = RecordingRunner(output: "{}", version: "echo 1.0")
        let impostor = try makeProgram(named: "claude", permissions: 0o755)
        let plugin = try ClaudeCodePlugin(config: config(["path": impostor]), runner: runner)
        await #expect(throws: HTTPError.api(L("%@ isn’t Claude Code. Run `which claude` in Terminal and paste the path it shows.", impostor))) {
            try await plugin.digest([item], topic: "x", now: .now)
        }
        #expect(await runner.arguments.isEmpty, "no inbox content was passed")
        let echo = try ClaudeCodePlugin(config: config(["path": "/bin/echo"]), runner: runner)
        await #expect(throws: HTTPError.api(L("Remora only runs a program named claude that only you can change, which %@ isn’t.", "/bin/echo"))) {
            try await echo.digest([item], topic: "x", now: .now)
        }
    }

    @Test func failuresAreExplained() {
        #expect(ClaudeCodePlugin.explain("env: node: No such file or directory").errorDescription?.contains("Node.js") == true)
        #expect(ClaudeCodePlugin.explain("Invalid API key · Please run /login").errorDescription?.contains("signed in") == true)
        #expect(ClaudeCodePlugin.explain("Error: rate limited") == .api(L("Claude Code: %@", "Error: rate limited")))
    }

    /// Calls the real Claude Code with Haiku. Opt in with REMORA_LIVE_CLAUDE=1.
    @Test(.enabled(if: ProcessInfo.processInfo.environment["REMORA_LIVE_CLAUDE"] == "1"))
    func liveDigest() async throws {
        let plugin = try ClaudeCodePlugin(config: config([:]), http: URLSessionHTTPClient())
        let summary = try await plugin.digest([item], topic: "To review", now: .now)
        #expect(!summary.isEmpty)
    }
}

/// A `claude` in a folder of its own, for tests that don't run it (a `RecordingRunner` answers instead).
func fakeClaude() -> String {
    let path = FileManager.default.temporaryDirectory.appending(path: "remora-fake-claude/claude").path
    if !FileManager.default.fileExists(atPath: path) { _ = try? makeProgram(named: "claude", permissions: 0o755, at: path) }
    return path
}

@discardableResult
func makeProgram(named name: String, permissions: Int, folderPermissions: Int = 0o755, at path: String? = nil) throws -> String {
    let path = path ?? FileManager.default.temporaryDirectory.appending(path: "remora-\(UUID().uuidString)/\(name)").path
    let folder = (path as NSString).deletingLastPathComponent
    try FileManager.default.createDirectory(atPath: folder, withIntermediateDirectories: true)
    try Data("#!/bin/sh\n".utf8).write(to: URL(fileURLWithPath: path))
    try FileManager.default.setAttributes([.posixPermissions: permissions], ofItemAtPath: path)
    try FileManager.default.setAttributes([.posixPermissions: folderPermissions], ofItemAtPath: folder)
    return path
}

actor RecordingRunner: CommandRunner {
    let output: String
    let version: String
    private(set) var arguments: [String] = []
    private(set) var environment: [String: String]?
    private(set) var input: Data?

    init(output: String, version: String = "2.1.295 (Claude Code)") {
        self.output = output
        self.version = version
    }

    func run(_ executable: URL, arguments: [String], environment: [String: String]?, input: Data?) async throws -> Data {
        if arguments == ["--version"] { return Data(version.utf8) }
        self.arguments = arguments
        self.environment = environment
        self.input = input
        return Data(output.utf8)
    }
}

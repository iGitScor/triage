import Foundation
import Testing

@testable import RemoraCore

/// V15-01: the MCP server against the shared contract of `mcp/` (same conversations as `mcp.rs` on Windows and
/// Linux).
struct McpServerTests {
    static let folder = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        .deletingLastPathComponent().appendingPathComponent("mcp")

    static func json(_ text: String) -> Any? {
        try? JSONSerialization.jsonObject(with: Data(text.utf8), options: [.fragmentsAllowed])
    }

    /// A tool's text: JSON compared as a value; a notice line then JSON, the line exactly and the JSON as a value.
    static func sameText(_ got: String, _ want: String) -> Bool {
        if let a = json(got), let b = json(want) { return same(a, b) }
        let g = got.split(separator: "\n", maxSplits: 1).map(String.init)
        let w = want.split(separator: "\n", maxSplits: 1).map(String.init)
        guard g.count == 2, w.count == 2, g[0] == w[0], let a = json(g[1]), let b = json(w[1]) else {
            return got == want
        }
        return same(a, b)
    }

    static func same(_ got: Any, _ want: Any) -> Bool {
        switch (got, want) {
        case (let g as [String: Any], let w as [String: Any]):
            return g.count == w.count
                && w.allSatisfy { key, value in
                    guard let other = g[key] else { return false }
                    if key == "text", let a = other as? String, let b = value as? String { return sameText(a, b) }
                    return same(other, value)
                }
        case (let g as [Any], let w as [Any]):
            return g.count == w.count && zip(g, w).allSatisfy { same($0, $1) }
        case (is NSNull, is NSNull): return true
        case (let g as NSNumber, let w as NSNumber): return g == w
        case (let g as String, let w as String): return g == w
        default: return false
        }
    }

    static func fixtures() throws -> [(String, [String: Any])] {
        let files = try FileManager.default.contentsOfDirectory(atPath: folder.appendingPathComponent("fixtures").path)
        return try files.filter { $0.hasSuffix(".json") }.sorted().map { name in
            let data = try Data(contentsOf: folder.appendingPathComponent("fixtures/\(name)"))
            return (name, try #require(try JSONSerialization.jsonObject(with: data) as? [String: Any]))
        }
    }

    @Test func theRecordedConversationsReplay() throws {
        let fixtures = try Self.fixtures()
        #expect(fixtures.count >= 10)
        for (name, fixture) in fixtures {
            let server = McpServer(
                version: fixture["version"] as? String ?? "", allowed: fixture["allowed"] as? Bool == true)
            for (n, step) in (fixture["steps"] as? [[String: Any]] ?? []).enumerated() {
                let line: String
                if let raw = step["raw"] as? String {
                    line = raw
                } else {
                    let data = try JSONSerialization.data(withJSONObject: step["client"] ?? [:])
                    line = String(decoding: data, as: UTF8.self)
                }
                var expected = step["app"] as? [[String: Any]] ?? []
                let answer = server.handle(line) { request in
                    guard !expected.isEmpty else {
                        Issue.record("\(name) step \(n): unexpected \(request)")
                        throw McpServer.Unreachable()
                    }
                    let exchange = expected.removeFirst()
                    #expect(
                        Self.same(Self.json(request) ?? NSNull(), exchange["request"] ?? NSNull()),
                        "\(name) step \(n): request \(request)")
                    if exchange["response"] as? String == "unreachable" { throw McpServer.Unreachable() }
                    let data = try JSONSerialization.data(withJSONObject: exchange["response"] ?? [:])
                    return String(decoding: data, as: UTF8.self)
                }
                #expect(expected.isEmpty, "\(name) step \(n): the app was not asked \(expected)")
                switch (answer, step["server"]) {
                case (nil, is NSNull), (nil, nil): break
                case (let got?, let want?):
                    #expect(
                        Self.same(Self.json(got) ?? NSNull(), want), "\(name) step \(n):\n got  \(got)\n want \(want)")
                default: Issue.record("\(name) step \(n): got \(answer ?? "nothing")")
                }
            }
        }
    }

    /// The tools served are `mcp/tools.json`, the file both apps share.
    @Test func theToolsAreTheSharedOnes() throws {
        let shared = try Data(contentsOf: Self.folder.appendingPathComponent("tools.json"))
        let file = try JSONSerialization.jsonObject(with: shared)
        #expect(Self.same(McpServer(version: "", allowed: true).tools, file))
    }

    /// V15-10: unset is allowed; off, or a value that can't be read, is off.
    @Test func theOrganizationDecidesLocalAccess() {
        #expect(LocalAPIPolicy.allowed(forced: { _ in nil }))
        #expect(!LocalAPIPolicy.allowed(forced: { _ in false as NSNumber }))
        #expect(LocalAPIPolicy.allowed(forced: { _ in true as NSNumber }))
        #expect(!LocalAPIPolicy.allowed(forced: { _ in "maybe" }))
        #expect(!LocalAPIPolicy.allowed(forced: { _ in ["x"] }))
    }

    @Test func onlyRequestsFromAgentsAreRecorded() {
        let now = Date(timeIntervalSince1970: 0)
        let read = AgentRead.from(#"{"command":"list","tab":"waiting","agent":"Claude\nDesktop"}"#, now: now)
        #expect(read == AgentRead(agent: "Claude Desktop", command: "list", tab: "waiting", at: now))
        #expect(AgentRead.from(#"{"command":"count"}"#, now: now) == nil, "the remora command names no agent")
        let log = (0..<205).reduce([AgentRead]()) { log, _ in AgentRead.adding(read!, to: log) }
        #expect(log.count == AgentRead.kept)
    }

    @Test func agentNamesAreCleaned() {
        #expect(McpServer.agentName("  Claude\tDesktop ") == "Claude Desktop")
        #expect(McpServer.agentName(nil) == "Unknown agent")
        #expect(McpServer.agentName(String(repeating: "x", count: 100)).count == 64)
    }
}

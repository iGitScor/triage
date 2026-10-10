import Foundation
import Testing

@testable import RemoraCore

struct PluginConfigTests {
    private func host(_ value: String) throws -> URL {
        try PluginConfig(accountID: UUID(), values: ["host": value]).url("host")
    }

    @Test func hostsDefaultToHTTPS() throws {
        #expect(try host("gitlab.acme.io/").absoluteString == "https://gitlab.acme.io")
        #expect(try host("https://github.acme.io//").absoluteString == "https://github.acme.io")
    }

    /// The token goes with every request, so a plain-http host is refused.
    @Test func plainHTTPIsRefusedExceptOnThisComputer() throws {
        #expect(throws: PluginError.insecureField("host")) { try host("http://gitlab.lan") }
        #expect(throws: PluginError.insecureField("host")) { try host("ftp://gitlab.lan") }
        #expect(try host("http://localhost:8929").absoluteString == "http://localhost:8929")
        #expect(try host("http://127.0.0.1").absoluteString == "http://127.0.0.1")
        #expect(throws: PluginError.invalidField("host")) { try host("https://gitlab acme.io") }
    }
}

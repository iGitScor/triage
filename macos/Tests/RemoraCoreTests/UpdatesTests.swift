import CryptoKit
import Foundation
import Testing
@testable import RemoraCore

/// What the Mac accepts as an update.
struct UpdatesTests {
    @Test func versionsCompareByNumberNotText() throws {
        #expect(try #require(AppVersion("0.10.0")) > #require(AppVersion("0.9.9")))
        #expect(try #require(AppVersion("v1.2")) == #require(AppVersion("1.2.0")))
        #expect(AppVersion("1.2.beta") == nil)
        #expect(AppVersion("") == nil)
    }

    private func manifest(_ version: String, url: String = "https://github.com/iGitScor/triage/releases/download/v0.4.0/Remora.dmg") throws -> UpdateManifest {
        let json = """
        {"version": "\(version)", "notes": "https://github.com/iGitScor/triage/releases/tag/v\(version)",
         "platforms": {"macos-universal": {"url": "\(url)", "signature": "c2ln"},
                       "windows-x86_64": {"url": "https://github.com/x/Remora-Setup.exe", "signature": "minisign"}}}
        """
        return try JSONDecoder().decode(UpdateManifest.self, from: Data(json.utf8))
    }

    @Test func onlyANewerReleaseIsOffered() throws {
        let current = try #require(AppVersion("0.3.2"))
        #expect(UpdateOffer.from(try manifest("0.4.0"), current: current)?.version.description == "0.4.0")
        #expect(UpdateOffer.from(try manifest("0.3.2"), current: current) == nil, "same version")
        #expect(UpdateOffer.from(try manifest("0.3.1"), current: current) == nil, "never a downgrade")
    }

    @Test func onlyAGitHubHttpsDownloadIsOffered() throws {
        let current = try #require(AppVersion("0.3.2"))
        #expect(UpdateOffer.from(try manifest("0.4.0", url: "https://evil.example/Remora.dmg"), current: current) == nil)
        #expect(UpdateOffer.from(try manifest("0.4.0", url: "http://github.com/Remora.dmg"), current: current) == nil)
        #expect(UpdateHosts.allows(URL(string: "https://release-assets.githubusercontent.com/x")))
        #expect(!UpdateHosts.allows(URL(string: "https://github.com.evil.example/x")))
    }

    @Test func aDownloadMustCarryTheReleaseSignature() {
        let key = Curve25519.Signing.PrivateKey()
        let publicKey = key.publicKey.rawRepresentation.base64EncodedString()
        let dmg = Data("the release".utf8)
        let signature = (try? key.signature(for: dmg))?.base64EncodedString() ?? ""
        #expect(UpdateSignature.isValid(dmg, signature: signature, publicKey: publicKey))
        #expect(!UpdateSignature.isValid(Data("tampered".utf8), signature: signature, publicKey: publicKey))
        let other = Curve25519.Signing.PrivateKey().publicKey.rawRepresentation.base64EncodedString()
        #expect(!UpdateSignature.isValid(dmg, signature: signature, publicKey: other), "another key")
        #expect(!UpdateSignature.isValid(dmg, signature: "not base64!", publicKey: publicKey))
    }
}

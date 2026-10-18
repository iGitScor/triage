import Foundation
import RemoraCore
import Security
import Testing

@testable import Remora

/// The checks on the app copied out of an update, after its download's signature: on throwaway apps, each refused for
/// its own reason, and one that passes.
@MainActor
struct LiveUpdateSystemTests {
    let identifier = "fr.igitscor.remora.test"
    let version = AppVersion("1.2.3")!

    /// A minimal app: an Info.plist and a copy of `/usr/bin/true` as its program; ad hoc signed when `signed`.
    func app(identifier: String, version: String, signed: Bool) throws -> URL {
        let root = FileManager.default.temporaryDirectory.appending(path: "update-\(UUID().uuidString)/Remora.app")
        let macOS = root.appending(path: "Contents/MacOS")
        try FileManager.default.createDirectory(at: macOS, withIntermediateDirectories: true)
        let info: [String: Any] = [
            "CFBundleIdentifier": identifier, "CFBundleShortVersionString": version, "CFBundleExecutable": "Remora",
            "CFBundlePackageType": "APPL",
        ]
        try (info as NSDictionary).write(to: root.appending(path: "Contents/Info.plist"))
        try FileManager.default.copyItem(at: URL(fileURLWithPath: "/usr/bin/true"), to: macOS.appending(path: "Remora"))
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/codesign")
        // Unsigned: the copied program's own signature is removed, so nothing vouches for the bundle.
        process.arguments =
            signed
            ? ["--force", "--sign", "-", root.path] : ["--remove-signature", macOS.appending(path: "Remora").path]
        process.standardError = FileHandle.nullDevice
        try process.run()
        process.waitUntilExit()
        #expect(process.terminationStatus == 0)
        return root
    }

    func requirement(_ text: String) throws -> SecRequirement {
        var requirement: SecRequirement?
        #expect(SecRequirementCreateWithString(text as CFString, [], &requirement) == errSecSuccess)
        return try #require(requirement)
    }

    func refusal(_ app: URL, requirement: SecRequirement?) -> UpdateError? {
        do {
            try LiveUpdateSystem.verify(app, version: version, identifier: identifier, requirement: requirement)
            return nil
        } catch {
            return error as? UpdateError
        }
    }

    @Test func theSameAppSignedTheSameWayPasses() throws {
        let app = try app(identifier: identifier, version: "1.2.3", signed: true)
        #expect(refusal(app, requirement: try requirement(#"identifier "\#(identifier)""#)) == nil)
    }

    @Test func anotherAppIsRefused() throws {
        let app = try app(identifier: "com.example.other", version: "1.2.3", signed: true)
        #expect(refusal(app, requirement: try requirement(#"identifier "com.example.other""#)) == .wrongApp)
    }

    @Test func anotherVersionIsRefused() throws {
        let app = try app(identifier: identifier, version: "1.2.2", signed: true)
        #expect(refusal(app, requirement: try requirement(#"identifier "\#(identifier)""#)) == .wrongVersion)
    }

    @Test func anUnsignedCopyIsRefused() throws {
        let app = try app(identifier: identifier, version: "1.2.3", signed: false)
        #expect(refusal(app, requirement: try requirement(#"identifier "\#(identifier)""#)) == .notSameDeveloper)
    }

    /// Ad hoc signed, but not by the developer the running app's requirement names.
    @Test func anotherSignerIsRefused() throws {
        let app = try app(identifier: identifier, version: "1.2.3", signed: true)
        let developer = try requirement(
            #"anchor apple generic and certificate leaf[subject.OU] = "ABCDE12345" and identifier "\#(identifier)""#)
        #expect(refusal(app, requirement: developer) == .notSameDeveloper)
    }

    /// When the running app's own requirement can't be read, nothing installs.
    @Test func noRequirementIsRefused() throws {
        let app = try app(identifier: identifier, version: "1.2.3", signed: true)
        #expect(refusal(app, requirement: nil) == .notSameDeveloper)
    }
}

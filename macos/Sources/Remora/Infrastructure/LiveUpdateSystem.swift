import AppKit
import Foundation
import RemoraCore
import Security

/// Updating on this Mac: `latest.json` and the DMG from GitHub, then the app bundle replaced in place.
@MainActor
struct LiveUpdateSystem: UpdateSystem {
    private let session = URLSession(
        configuration: {
            let configuration = URLSessionConfiguration.ephemeral
            configuration.urlCache = nil
            configuration.requestCachePolicy = .reloadIgnoringLocalCacheData
            return configuration
        }(),
        delegate: UpdateRedirects(), delegateQueue: nil
    )

    var currentVersion: AppVersion? {
        (Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String).flatMap(AppVersion.init)
    }

    var publicKey: String? {
        (Bundle.main.infoDictionary?["RemoraUpdatePublicKey"] as? String).flatMap { $0.isEmpty ? nil : $0 }
    }

    func manifest() async throws -> UpdateManifest {
        let (data, response) = try await session.data(for: .get(UpdateHosts.manifest))
        guard let http = response as? HTTPURLResponse, http.statusCode == 200 else {
            throw HTTPError.status((response as? HTTPURLResponse)?.statusCode ?? 0)
        }
        return try JSONDecoder().decode(UpdateManifest.self, from: data)
    }

    func download(_ url: URL) async throws -> URL {
        guard UpdateHosts.allows(url) else { throw EgressError.blockedHost(url.host ?? "?") }
        let (file, response) = try await session.download(for: .get(url, timeout: 120))
        guard let http = response as? HTTPURLResponse, http.statusCode == 200 else {
            throw HTTPError.status((response as? HTTPURLResponse)?.statusCode ?? 0)
        }
        let target = FileManager.default.temporaryDirectory.appending(path: "Remora-update-\(UUID().uuidString).dmg")
        try FileManager.default.moveItem(at: file, to: target)
        return target
    }

    func install(_ dmg: URL, version: AppVersion) async throws {
        let current = Bundle.main.bundleURL
        let folder = current.deletingLastPathComponent()
        // A translocated copy (run from Downloads or the disk image) or a folder we can't write: never half-replace.
        guard current.pathExtension == "app", !current.path.contains("/AppTranslocation/"),
            FileManager.default.isWritableFile(atPath: folder.path)
        else {
            throw UpdateError.cannotReplace(folder.path)
        }

        let mount = FileManager.default.temporaryDirectory.appending(path: "Remora-update-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: mount, withIntermediateDirectories: true)
        let hdiutil = URL(fileURLWithPath: "/usr/bin/hdiutil")
        let runner = ProcessCommandRunner(timeout: 120)
        _ = try await runner.run(
            hdiutil,
            arguments: ["attach", "-nobrowse", "-readonly", "-noautoopen", "-mountpoint", mount.path, dmg.path])
        let staging: URL
        do {
            // On the app's own volume, so the swap below is a rename, not a copy.
            let replacement = try FileManager.default.url(
                for: .itemReplacementDirectory, in: .userDomainMask, appropriateFor: current, create: true)
            staging = replacement.appending(path: "Remora.app")
            try FileManager.default.copyItem(at: mount.appending(path: "Remora.app"), to: staging)
        } catch {
            _ = try? await runner.run(hdiutil, arguments: ["detach", "-force", mount.path])
            throw error
        }
        _ = try? await runner.run(hdiutil, arguments: ["detach", "-force", mount.path])
        try? FileManager.default.removeItem(at: mount)

        do {
            try Self.verify(staging, version: version)
            _ = try FileManager.default.replaceItemAt(current, withItemAt: staging)
        } catch {
            try? FileManager.default.removeItem(at: staging.deletingLastPathComponent())
            throw error
        }
    }

    /// Same bundle identifier, the promised version, and signed so that it meets this app's designated requirement:
    /// with the release certificate, only an app signed by that same certificate does.
    static func verify(_ app: URL, version: AppVersion) throws {
        let info = NSDictionary(contentsOf: app.appending(path: "Contents/Info.plist")) as? [String: Any] ?? [:]
        guard info["CFBundleIdentifier"] as? String == Bundle.main.bundleIdentifier else { throw UpdateError.wrongApp }
        guard (info["CFBundleShortVersionString"] as? String).flatMap(AppVersion.init) == version else {
            throw UpdateError.wrongVersion
        }

        var running: SecCode?
        var runningStatic: SecStaticCode?
        var requirement: SecRequirement?
        var staticCode: SecStaticCode?
        guard SecCodeCopySelf([], &running) == errSecSuccess, let running,
            SecCodeCopyStaticCode(running, [], &runningStatic) == errSecSuccess, let runningStatic,
            SecCodeCopyDesignatedRequirement(runningStatic, [], &requirement) == errSecSuccess, let requirement,
            SecStaticCodeCreateWithPath(app as CFURL, [], &staticCode) == errSecSuccess, let staticCode
        else {
            throw UpdateError.notSameDeveloper
        }
        let flags = SecCSFlags(rawValue: kSecCSCheckAllArchitectures | kSecCSStrictValidate | kSecCSCheckNestedCode)
        guard SecStaticCodeCheckValidity(staticCode, flags, requirement) == errSecSuccess else {
            throw UpdateError.notSameDeveloper
        }
    }

    /// Quits, and a small shell waits for this process to end before opening the new app.
    func relaunch() {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/bin/sh")
        process.arguments = [
            "-c", "while kill -0 \"$1\" 2>/dev/null; do sleep 0.2; done; /usr/bin/open \"$2\"",
            "sh", String(ProcessInfo.processInfo.processIdentifier), Bundle.main.bundleURL.path,
        ]
        try? process.run()
        NSApp.terminate(nil)
    }
}

/// Release downloads redirect from github.com to GitHub's download servers: allowed between those hosts only, over
/// https. Nothing secret is sent, unlike the tools' requests, whose redirects stay on one host.
final class UpdateRedirects: NSObject, URLSessionTaskDelegate, @unchecked Sendable {
    func urlSession(
        _ session: URLSession,
        task: URLSessionTask,
        willPerformHTTPRedirection response: HTTPURLResponse,
        newRequest request: URLRequest
    ) async -> URLRequest? {
        UpdateHosts.allows(request.url) ? request : nil
    }
}

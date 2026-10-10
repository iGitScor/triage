import CryptoKit
import Foundation
import RemoraCore
import Security

@testable import Remora

/// A fresh temporary folder per test, never Application Support.
func temporaryFolder() -> URL {
    let folder = FileManager.default.temporaryDirectory.appending(
        path: "RemoraTests-\(UUID().uuidString)", directoryHint: .isDirectory)
    try? FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
    return folder
}

/// Keychain items in a dictionary, keyed by service then account.
/// `refusing` makes every write and delete fail, as when the user clicks Deny.
final class MemoryKeychainItems: KeychainItems {
    var items: [String: [String: Data]] = [:]
    var refusing = false

    func read(service: String, account: String) throws -> Data? { items[service]?[account] }
    func write(_ data: Data, service: String, account: String) throws {
        if refusing { throw KeychainError.status(errSecAuthFailed) }
        items[service, default: [:]][account] = data
    }
    func delete(service: String, account: String) throws {
        if refusing { throw KeychainError.status(errSecAuthFailed) }
        items[service]?[account] = nil
    }
    func deleteAll(service: String) throws {
        if refusing { throw KeychainError.status(errSecAuthFailed) }
        items[service] = nil
    }
}

@MainActor
final class MemorySecrets: SecretStore {
    var stored: [UUID: [String: String]] = [:]
    var erased = false
    var refusing = false

    func secrets(for account: UUID) -> [String: String] { stored[account] ?? [:] }
    func save(_ secrets: [String: String], for account: UUID) throws {
        if refusing { throw KeychainError.status(errSecAuthFailed) }
        stored[account] = secrets
    }
    func delete(_ account: UUID) throws {
        if refusing { throw KeychainError.status(errSecAuthFailed) }
        stored[account] = nil
    }
    func deleteAll() throws {
        if refusing { throw KeychainError.status(errSecAuthFailed) }
        stored = [:]
        erased = true
    }
}

/// What the inbox asked of notifications.
final class RecordingNotifier: Notifying {
    var posted: [Notice] = []
    var scheduled: [(notice: Notice, at: Date)] = []
    var cancelled: [String] = []
    var removedPrefixes: [String] = []
    var removedAll = false

    func post(_ notice: Notice) { posted.append(notice) }
    func schedule(_ notice: Notice, at date: Date) { scheduled.append((notice, date)) }
    func cancel(_ itemID: String) { cancelled.append(itemID) }
    func removeAll() { removedAll = true }
    func remove(itemsWithPrefix prefix: String) { removedPrefixes.append(prefix) }
}

struct ManagedDictionary: ManagedValues {
    var values: [String: Any] = [:]
    func forced(_ key: String) -> Any? { values[key] }
}

/// Answers Linear's two queries, or fails every request once `failing` is set.
final class LinearServer: HTTPClient, @unchecked Sendable {
    var failing = false
    /// Answers every request with this status (401: a rejected token).
    var status: Int?
    /// Fails as the system does when there's no network, before any answer.
    var offline = false
    private(set) var requests = 0

    func send(_ request: URLRequest) async throws -> (Data, HTTPURLResponse) {
        requests += 1
        let url = request.url!
        if offline { throw URLError(.notConnectedToInternet) }
        if let status {
            return (
                Data("{}".utf8), HTTPURLResponse(url: url, statusCode: status, httpVersion: nil, headerFields: nil)!
            )
        }
        if failing {
            return (Data("{}".utf8), HTTPURLResponse(url: url, statusCode: 500, httpVersion: nil, headerFields: nil)!)
        }
        let body = String(data: request.httpBody ?? Data(), encoding: .utf8) ?? ""
        let answer = body.contains("notifications") ? Self.notifications : Self.issues
        return (Data(answer.utf8), HTTPURLResponse(url: url, statusCode: 200, httpVersion: nil, headerFields: nil)!)
    }

    static var issues: String {
        let recent = Date.now.addingTimeInterval(-3_600).ISO8601Format()
        return """
            {"data": {"viewer": {"name": "Alice", "assignedIssues": {"nodes": [
              {"id": "i1", "identifier": "ENG-42", "title": "Fix CSV export", "url": "https://linear.app/x/issue/ENG-42",
               "priority": 1, "updatedAt": "\(recent)", "state": {"name": "In Progress"}},
              {"id": "i2", "identifier": "ENG-43", "title": "Polish onboarding", "url": "https://linear.app/x/issue/ENG-43",
               "priority": 2, "updatedAt": "\(recent)"}
            ]}}}}
            """
    }

    static let notifications = #"{"data": {"notifications": {"nodes": []}}}"#
}

/// The rest of the Mac: installed apps, opened links, the HTTP cache.
@MainActor
final class Outside {
    var installedApps = true
    var opened: [URL] = []
    var cachesRemoved = 0
}

/// One inbox on a temporary folder, with stand-ins for everything outside it.
@MainActor
struct Harness {
    let folder: URL
    let secrets = MemorySecrets()
    let notifier = RecordingNotifier()
    let server = LinearServer()
    let outside = Outside()
    let updates = StandInUpdates()
    let network = StandInNetwork()
    var managed = ManagedDictionary()
    var demo = false

    init(folder: URL = temporaryFolder()) {
        self.folder = folder
    }

    func model() -> InboxModel {
        let outside = outside
        return InboxModel(
            environment: AppEnvironment(
                folder: folder,
                secrets: secrets,
                managed: managed,
                notifications: notifier,
                http: server,
                canOpen: { _ in outside.installedApps },
                open: { url in
                    outside.opened.append(url)
                    return true
                },
                demo: demo,
                removeURLCaches: { outside.cachesRemoved += 1 },
                updates: updates,
                network: network
            ))
    }

    /// A model with a Linear account connected through the stand-in server.
    func connected() async throws -> InboxModel {
        let model = model()
        try await model.connect(pluginID: "linear", name: nil, settings: [:], secrets: ["token": "lin_api_key"])
        return model
    }

    /// Every file the model wrote, as text.
    func writtenText() -> String {
        let files = (try? FileManager.default.contentsOfDirectory(at: folder, includingPropertiesForKeys: nil)) ?? []
        return files.compactMap { try? String(contentsOf: $0, encoding: .utf8) }.joined(separator: "\n")
    }
}

/// A release server and installer in memory: what was asked, and whether it installed.
@MainActor
final class StandInUpdates: UpdateSystem {
    var currentVersion: AppVersion? = AppVersion("0.3.2")
    var publicKey: String? = StandInUpdates.key.publicKey.rawRepresentation.base64EncodedString()
    var published = "0.4.0"
    var file = Data("Remora 0.4.0".utf8)
    var signedFile: Data?
    var manifestRequests = 0
    var installed: AppVersion?
    var relaunched = false

    static let key = Curve25519.Signing.PrivateKey()

    func manifest() async throws -> UpdateManifest {
        manifestRequests += 1
        let signature = try Self.key.signature(for: signedFile ?? file).base64EncodedString()
        let json = """
            {"version": "\(published)", "platforms": {"macos-universal": {"url": "https://github.com/iGitScor/triage/releases/download/v\(published)/Remora.dmg", "signature": "\(signature)"}}}
            """
        return try JSONDecoder().decode(UpdateManifest.self, from: Data(json.utf8))
    }

    func download(_ url: URL) async throws -> URL {
        let target = FileManager.default.temporaryDirectory.appending(path: "RemoraTests-\(UUID().uuidString).dmg")
        try file.write(to: target)
        return target
    }

    func install(_ dmg: URL, version: AppVersion) async throws { installed = version }
    func relaunch() { relaunched = true }
}

/// The network, switched by the test.
@MainActor
final class StandInNetwork: NetworkStatus {
    private var change: (@MainActor (Bool) -> Void)?
    func observe(_ change: @escaping @MainActor (Bool) -> Void) { self.change = change }
    func set(online: Bool) { change?(online) }
}

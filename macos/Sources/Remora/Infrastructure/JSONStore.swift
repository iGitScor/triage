import Foundation
import RemoraCore

/// Application Support/Remora. Data from the app's earlier name (Perch) is moved here once.
enum AppFolder {
    static let url: URL = {
        let folder = URL.applicationSupportDirectory.appending(path: "Remora", directoryHint: .isDirectory)
        let previous = URL.applicationSupportDirectory.appending(path: "Perch", directoryHint: .isDirectory)
        let files = FileManager.default
        if !files.fileExists(atPath: folder.path), files.fileExists(atPath: previous.path) {
            try? files.moveItem(at: previous, to: folder)
        }
        try? files.createDirectory(at: folder, withIntermediateDirectories: true)
        // Only you can list or open it, also when an earlier version created it 0755.
        try? files.setAttributes([.posixPermissions: 0o700], ofItemAtPath: folder.path)
        return folder
    }()
}

/// The HTTP cache and cookies that earlier versions left on disk through `URLSession.shared` (API answers,
/// message content included). Requests no longer write there; this removes what's left.
enum URLCaches {
    static func remove() {
        URLCache.shared.removeAllCachedResponses()
        guard let id = Bundle.main.bundleIdentifier else { return }
        let files = FileManager.default
        for folder in [URL.cachesDirectory, URL.libraryDirectory.appending(path: "HTTPStorages")] {
            try? files.removeItem(at: folder.appending(path: id, directoryHint: .isDirectory))
        }
    }
}

/// A Codable value persisted as a JSON file in Application Support, readable by you only (0600).
struct JSONStore<Value: Codable> {
    let url: URL
    /// False for what Remora can fetch again (the inbox cache, briefs): it stays out of Time Machine and iCloud
    /// backups, where message content would outlive Erase.
    let backedUp: Bool

    /// `name`.json in `folder`: Application Support/Remora in the app, a temporary folder in tests.
    init(_ name: String, in folder: URL = AppFolder.url, backedUp: Bool = true) {
        url = folder.appending(path: "\(name).json")
        self.backedUp = backedUp
    }

    /// Why a file couldn't be used, so it is said instead of being silently replaced.
    enum Failure: LocalizedError, Equatable {
        /// The file didn't decode: it was moved to `aside`, so the next save can't destroy it.
        case corrupt(name: String, aside: String)
        case unwritable(name: String, reason: String)

        var errorDescription: String? {
            switch self {
            case .corrupt(let name, let aside):
                L("%@ couldn’t be read. It was kept as %@, and Remora started without it.", name, aside)
            case .unwritable(let name, let reason):
                L("Remora couldn’t save %@: %@", name, reason)
            }
        }
    }

    /// The saved value; nil when there is no file. A file that doesn't decode (corrupt, or from a newer version) is
    /// moved aside before anything is written, then reported: read as "nothing saved", the next save would
    /// replace every account with none, and orphan their tokens.
    func read() throws(Failure) -> Value? {
        guard let data = try? Data(contentsOf: url) else { return nil }
        if let value = try? JSONDecoder().decode(Value.self, from: data) { return value }
        var aside = url.appendingPathExtension("corrupt")
        var number = 2
        while FileManager.default.fileExists(atPath: aside.path) {
            aside = url.appendingPathExtension("corrupt\(number)")
            number += 1
        }
        try? FileManager.default.moveItem(at: url, to: aside)
        throw .corrupt(name: url.lastPathComponent, aside: aside.lastPathComponent)
    }

    /// `read()`, for what can be started again without a word (tests, values Remora fetches again).
    func load() -> Value? {
        try? read()
    }

    func save(_ value: Value) throws(Failure) {
        let data: Data
        do {
            data = try JSONEncoder().encode(value)
            try data.write(to: url, options: .atomic)
        } catch {
            throw .unwritable(name: url.lastPathComponent, reason: error.localizedDescription)
        }
        // An atomic write replaces the file, so its permissions and backup flag are set again each time.
        try? FileManager.default.setAttributes([.posixPermissions: 0o600], ofItemAtPath: url.path)
        if !backedUp {
            var values = URLResourceValues()
            values.isExcludedFromBackup = true
            var url = url
            try? url.setResourceValues(values)
        }
    }
}

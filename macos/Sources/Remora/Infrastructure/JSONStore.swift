import Foundation

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
        return folder
    }()
}

/// A Codable value persisted as a JSON file in Application Support.
struct JSONStore<Value: Codable> {
    let url: URL

    init(_ name: String) {
        url = AppFolder.url.appending(path: "\(name).json")
    }

    func load() -> Value? {
        guard let data = try? Data(contentsOf: url) else { return nil }
        return try? JSONDecoder().decode(Value.self, from: data)
    }

    func save(_ value: Value) {
        guard let data = try? JSONEncoder().encode(value) else { return }
        try? data.write(to: url, options: .atomic)
    }
}

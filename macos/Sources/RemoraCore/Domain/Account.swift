import Foundation

/// One connected instance of a plugin (e.g. "GitLab at gitlab.acme.io"). Secrets live in the Keychain.
public struct Account: Identifiable, Hashable, Codable, Sendable {
    public var id: UUID
    public var pluginID: String
    /// What the user calls this account ("Work", "Side project"). Optional.
    public var name: String?
    public var settings: [String: String]
    public var identity: String?

    public init(
        id: UUID = UUID(), pluginID: String, name: String? = nil, settings: [String: String], identity: String? = nil
    ) {
        self.id = id
        self.pluginID = pluginID
        self.name = name
        self.settings = settings
        self.identity = identity
    }

    public var subtitle: String {
        let host = settings["host"].flatMap { URL(string: $0)?.host ?? $0 }
        return [identity, host].compactMap { $0 }.filter { !$0.isEmpty }.joined(separator: " · ")
    }
}

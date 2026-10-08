import Foundation

/// Describes a plugin and the settings it needs. The settings UI is generated from it.
public struct PluginManifest: Sendable, Identifiable {
    public var id: String
    public var name: String
    public var symbol: String
    public var summary: String
    public var fields: [ConfigField]
    public var setupSteps: [String]
    public var setupLabel: String
    public var setupURL: (@Sendable (PluginConfig) -> URL?)?
    /// Listed with a "Soon" badge but can't be connected yet.
    public var isComingSoon: Bool
    /// Where the plugin's data goes.
    public var egress: Egress
    /// The tool's logo in the app's Resources/Logos (SVG), shown in the menu bar.
    public var logo: String?

    public init(
        id: String,
        name: String,
        symbol: String,
        summary: String,
        fields: [ConfigField],
        setupSteps: [String] = [],
        setupLabel: String = "Create a token",
        setupURL: (@Sendable (PluginConfig) -> URL?)? = nil,
        isComingSoon: Bool = false,
        egress: Egress,
        logo: String? = nil
    ) {
        self.id = id
        self.name = name
        self.symbol = symbol
        self.summary = summary
        self.fields = fields
        self.setupSteps = setupSteps
        self.setupLabel = setupLabel
        self.setupURL = setupURL
        self.isComingSoon = isComingSoon
        self.egress = egress
        self.logo = logo
    }
}

public struct ConfigField: Sendable, Identifiable, Hashable {
    public var key: String
    public var label: String
    public var placeholder: String
    public var defaultValue: String
    public var isSecret: Bool
    public var isOptional: Bool
    public var help: String?

    public var id: String { key }

    public init(
        key: String,
        label: String,
        placeholder: String = "",
        defaultValue: String = "",
        isSecret: Bool = false,
        isOptional: Bool = false,
        help: String? = nil
    ) {
        self.key = key
        self.label = label
        self.placeholder = placeholder
        self.defaultValue = defaultValue
        self.isSecret = isSecret
        self.isOptional = isOptional
        self.help = help
    }

    public static func token(_ label: String = "Access token", help: String? = nil) -> ConfigField {
        ConfigField(key: "token", label: label, isSecret: true, help: help)
    }

    public static func host(_ defaultValue: String) -> ConfigField {
        ConfigField(key: "host", label: "Host", placeholder: defaultValue, defaultValue: defaultValue)
    }
}

/// The values a user entered for one account of a plugin.
public struct PluginConfig: Sendable {
    public var accountID: UUID
    public var values: [String: String]

    public init(accountID: UUID, values: [String: String]) {
        self.accountID = accountID
        self.values = values
    }

    public subscript(key: String) -> String {
        (values[key] ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
    }

    public func required(_ key: String) throws -> String {
        let value = self[key]
        guard !value.isEmpty else { throw PluginError.missingField(key) }
        return value
    }

    public func url(_ key: String) throws -> URL {
        var raw = try required(key)
        while raw.hasSuffix("/") { raw.removeLast() }
        if !raw.contains("://") { raw = "https://" + raw }
        guard let url = URL(string: raw), url.host != nil else { throw PluginError.invalidField(key) }
        return url
    }
}

/// What a source returns on each refresh.
public struct SourceSnapshot: Sendable {
    public var identity: String
    public var items: [InboxItem]

    public init(identity: String, items: [InboxItem]) {
        self.identity = identity
        self.items = items
    }
}

/// An integration that feeds the inbox.
public protocol SourcePlugin: Sendable {
    static var manifest: PluginManifest { get }
    init(config: PluginConfig, http: HTTPClient) throws
    func fetch() async throws -> SourceSnapshot
}

public enum PluginError: LocalizedError, Equatable {
    case missingField(String)
    case invalidField(String)
    case unknownPlugin(String)

    public var errorDescription: String? {
        switch self {
        case .missingField(let key): L("“%@” is required.", key)
        case .invalidField(let key): L("“%@” is not valid.", key)
        case .unknownPlugin(let id): L("Unknown plugin “%@”.", id)
        }
    }
}

import Foundation

/// Where a plugin's data goes. Declared by every plugin and enforced by `GuardedHTTPClient`.
public struct Egress: Sendable, Equatable {
    /// Hosts the plugin may contact (subdomains included). The account's own host is added for
    /// self-hosted tools.
    public var hosts: [String]
    /// What is sent, in one sentence, for the Privacy settings and the compliance inventory.
    public var description: String
    /// Inbox content is sent to an AI provider outside the Mac.
    public var externalAI: Bool
    /// The setting that holds the AI server the user chose (`host`): the policy's AI server rules
    /// (`AllowedAIServers`, `AllowLocalAI`) apply to it, and a server on this computer isn't external.
    public var serverField: String?

    public init(hosts: [String], description: String, externalAI: Bool = false, serverField: String? = nil) {
        self.hosts = hosts
        self.description = description
        self.externalAI = externalAI
        self.serverField = serverField
    }
}

extension PluginManifest {
    /// The AI server an account of this plugin talks to: its own setting, else the field's default.
    public func server(in settings: [String: String]) -> String? {
        guard let key = egress.serverField else { return nil }
        let value = settings[key]?.trimmingCharacters(in: .whitespaces) ?? ""
        return value.isEmpty ? fields.first { $0.key == key }?.defaultValue : value
    }
}

/// An `AllowedAIServers` entry: a URL prefix. Scheme, host (any case) and port (443 or 80 when left out) must be equal,
/// and the path must start with the prefix's on a `/` boundary: `/openai/` allows `/openai/v1`, not `/openaix`.
public enum ServerPrefix {
    public static func matches(_ server: String, _ prefix: String) -> Bool {
        guard let server = parts(server), let prefix = parts(prefix) else { return false }
        guard server.scheme == prefix.scheme, server.host == prefix.host, server.port == prefix.port else {
            return false
        }
        return prefix.path.isEmpty || server.path == prefix.path || server.path.hasPrefix(prefix.path + "/")
    }

    private static func parts(_ raw: String) -> (scheme: String, host: String, port: Int, path: String)? {
        var raw = raw.trimmingCharacters(in: .whitespaces)
        if !raw.contains("://") { raw = "https://" + raw }
        guard let url = URL(string: raw), let scheme = url.scheme?.lowercased(), let host = url.host?.lowercased(),
            !host.isEmpty
        else { return nil }
        var path = url.path
        while path.hasSuffix("/") { path.removeLast() }
        return (scheme, host, url.port ?? (scheme == "http" ? 80 : 443), path)
    }
}

/// What may leave the Mac. Set by the user, or locked by the organization through MDM.
public struct CompliancePolicy: Equatable, Sendable {
    /// Plugins that may be connected and refreshed. Nil means every plugin.
    public var allowedPlugins: Set<String>?
    /// Plugins that send inbox content to an external AI need this.
    public var allowExternalAI: Bool
    /// Avatars, only from the hosts of allowed tools.
    public var allowRemoteImages: Bool
    /// `AllowedAIServers`: the URL prefixes an AI server must match. Nil: any server; empty: none.
    public var allowedAIServers: [String]?
    /// `AllowLocalAI`: an AI server on this computer. Nil when the organization didn't say.
    public var allowLocalAI: Bool?
    /// The organization set `AllowExternalAI`: when it's off, a server on this computer needs `AllowLocalAI` too.
    public var externalAIManaged: Bool

    public init(
        allowedPlugins: Set<String>? = nil, allowExternalAI: Bool = false, allowRemoteImages: Bool = true,
        allowedAIServers: [String]? = nil, allowLocalAI: Bool? = nil, externalAIManaged: Bool = false
    ) {
        self.allowedPlugins = allowedPlugins
        self.allowExternalAI = allowExternalAI
        self.allowRemoteImages = allowRemoteImages
        self.allowedAIServers = allowedAIServers
        self.allowLocalAI = allowLocalAI
        self.externalAIManaged = externalAIManaged
    }

    public func allows(_ manifest: PluginManifest) -> Bool {
        refusal(for: manifest) == nil
    }

    /// An account: for an assistant whose server the user chose, the AI server rules apply to that server.
    public func allows(_ manifest: PluginManifest, settings: [String: String]) -> Bool {
        refusal(for: manifest, settings: settings) == nil
    }

    /// Why a plugin is blocked, for the UI, before any server is known: one whose server could be on this computer
    /// is only refused when neither external nor local AI is allowed.
    public func refusal(for manifest: PluginManifest) -> String? {
        if let allowedPlugins, !allowedPlugins.contains(manifest.id) { return L("Not allowed by your privacy policy.") }
        guard manifest.egress.externalAI, !allowExternalAI else { return nil }
        if manifest.egress.serverField != nil, allowsLocalAI { return nil }
        return L("External AI is turned off in Privacy settings.")
    }

    /// Why an account is blocked: its plugin, then its AI server (AI-23).
    public func refusal(for manifest: PluginManifest, settings: [String: String]) -> String? {
        guard let server = manifest.server(in: settings) else { return refusal(for: manifest) }
        if let allowedPlugins, !allowedPlugins.contains(manifest.id) { return L("Not allowed by your privacy policy.") }
        guard manifest.egress.externalAI else { return nil }
        if let allowedAIServers, !allowedAIServers.contains(where: { ServerPrefix.matches(server, $0) }) {
            return L("Your organization doesn’t allow this AI server.")
        }
        let host = URL(string: server.contains("://") ? server : "https://" + server)?.host ?? ""
        if PluginConfig.isLoopback(host) {
            return allowsLocalAI ? nil : L("Your organization doesn’t allow an AI server on this computer.")
        }
        return allowExternalAI ? nil : L("External AI is turned off in Privacy settings.")
    }

    /// A server on this computer: what `AllowLocalAI` says, else allowed unless the organization turned external AI
    /// off, since a local server could pass everything on to the cloud.
    private var allowsLocalAI: Bool {
        allowLocalAI ?? !(externalAIManaged && !allowExternalAI)
    }
}

public enum EgressError: LocalizedError, Equatable {
    case blockedHost(String)
    case blockedPlugin(String)
    case blockedRedirect(String)

    public var errorDescription: String? {
        switch self {
        case .blockedHost(let host): L("Blocked: %@ is not an allowed destination.", host)
        case .blockedRedirect(let host): L("Blocked: the server redirected to %@.", host)
        case .blockedPlugin(let reason): reason
        }
    }
}

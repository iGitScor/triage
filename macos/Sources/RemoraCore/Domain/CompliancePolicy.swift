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

    public init(hosts: [String], description: String, externalAI: Bool = false) {
        self.hosts = hosts
        self.description = description
        self.externalAI = externalAI
    }
}

/// What may leave the Mac. Set by the user, or locked by the organization through MDM.
public struct CompliancePolicy: Equatable, Sendable {
    /// Plugins that may be connected and refreshed. Nil means every plugin.
    public var allowedPlugins: Set<String>?
    /// Plugins that send inbox content to an external AI (Claude) need this.
    public var allowExternalAI: Bool
    /// Avatars, only from the hosts of allowed tools.
    public var allowRemoteImages: Bool

    public init(allowedPlugins: Set<String>? = nil, allowExternalAI: Bool = false, allowRemoteImages: Bool = true) {
        self.allowedPlugins = allowedPlugins
        self.allowExternalAI = allowExternalAI
        self.allowRemoteImages = allowRemoteImages
    }

    public func allows(_ manifest: PluginManifest) -> Bool {
        (allowedPlugins?.contains(manifest.id) ?? true) && (!manifest.egress.externalAI || allowExternalAI)
    }

    /// Why a plugin is blocked, for the UI.
    public func refusal(for manifest: PluginManifest) -> String? {
        if let allowedPlugins, !allowedPlugins.contains(manifest.id) { return L("Not allowed by your privacy policy.") }
        if manifest.egress.externalAI, !allowExternalAI { return L("External AI is turned off in Privacy settings.") }
        return nil
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

import Foundation
import RemoraCore
import RemoraPlugins

/// The privacy policy as the inbox applies it: your Privacy settings with the organization's in place. What may be
/// connected and reached, which settings the organization replaces, what the assistant may read. Made from the
/// current preferences each time it is asked. Same on Windows (`policy.rs`).
@MainActor
struct InboxPolicy {
    let preferences: Preferences
    let managed: ManagedValues

    var compliance: CompliancePolicy { ManagedPolicy.policy(user: preferences, managed: managed) }

    /// What Remora acts on: your preferences with the organization's settings in place.
    var effectivePreferences: Preferences { ManagedPolicy.effective(preferences, managed: managed) }

    /// What the assistant may read and which models it may use.
    var assistantPolicy: AssistantPolicy { ManagedPolicy.assistantPolicy(user: preferences, managed: managed) }

    /// The organization can turn updating off entirely, or on for everyone.
    var updatesAllowed: Bool { ManagedPolicy.updatesAllowed(managed) }

    /// The account as Remora uses it: the organization's `AIServer` replaces the server the user typed, and its
    /// value for a field with a `managedKey` (`MicrosoftTenant`) replaces that field.
    func effective(_ account: Account) -> Account {
        guard let manifest = PluginRegistry.manifest(account.pluginID) else { return account }
        var account = account
        if let field = manifest.egress.serverField, let server = ManagedPolicy.aiServer(managed) {
            account.settings[field] = server
        }
        for field in manifest.fields {
            if let key = field.managedKey, let value = ManagedPolicy.setting(key, managed) {
                account.settings[field.key] = value
            }
        }
        return account
    }

    /// Why the privacy policy blocks an account: its tool, or for an assistant its AI server. Nil when allowed.
    func refusal(for account: Account) -> String? {
        guard let manifest = PluginRegistry.manifest(account.pluginID) else { return nil }
        return compliance.refusal(for: manifest, settings: effective(account).settings)
    }

    /// An assistant account with its model fields kept within the organization's allowed models, if any, and the
    /// organization's Claude Code path.
    func constrained(assistant account: Account) -> Account {
        let account = effective(account)
        var constrained = account
        let defaults = Dictionary(
            (PluginRegistry.manifest(account.pluginID)?.fields ?? []).map { ($0.key, $0.defaultValue) },
            uniquingKeysWith: { first, _ in first }
        )
        constrained.settings = assistantPolicy.constrained(account.settings, defaults: defaults)
        if account.pluginID == ClaudeCodePlugin.manifest.id, let path = ManagedPolicy.claudeCodePath(managed) {
            constrained.settings["path"] = path
        }
        return constrained
    }

    /// The only client a plugin gets: refused when the policy blocks the account, and limited to its declared
    /// hosts. `base` is what the guard lets requests through to: the network, or a layer in front of it.
    func guardedClient(for account: Account, through base: HTTPClient) throws -> HTTPClient {
        guard let manifest = PluginRegistry.manifest(account.pluginID) else {
            throw PluginError.unknownPlugin(account.pluginID)
        }
        if let refusal = refusal(for: account) { throw EgressError.blockedPlugin(refusal) }
        return GuardedHTTPClient(base, allowing: allowedHosts(for: account, manifest: manifest))
    }

    func allowedHosts(for account: Account, manifest: PluginManifest) -> [String] {
        let settings = effective(account).settings
        let own = (settings["host"] ?? manifest.server(in: settings)).flatMap {
            URL(string: $0.contains("://") ? $0 : "https://" + $0)?.host
        }
        return manifest.egress.hosts + [own].compactMap { $0 }
    }

    /// Avatars load only from the hosts of allowed, connected tools (`accounts`, the sources).
    func allowsImage(_ url: URL?, from accounts: [Account]) -> Bool {
        let policy = compliance
        guard policy.allowRemoteImages, url?.scheme?.lowercased() == "https", let host = url?.host?.lowercased()
        else { return false }
        return accounts.contains { account in
            guard let manifest = PluginRegistry.manifest(account.pluginID), policy.allows(manifest) else {
                return false
            }
            let hosts = allowedHosts(for: account, manifest: manifest) + (Self.imageOnlyHosts[account.pluginID] ?? [])
            return GuardedHTTPClient.matches(host, hosts)
        }
    }

    /// Hosts a tool's pictures come from without the tool ever being asked anything there: GitLab serves most
    /// avatars from Gravatar (Jira Cloud too), Slack a workspace's own emoji from its edge. Images only, and only while
    /// the policy allows them; never a request with a token.
    /// Same list on Windows (`view.rs`).
    static let imageOnlyHosts = [
        "gitlab": ["secure.gravatar.com", "www.gravatar.com"],
        // A workspace's own emoji, in reactions.
        "slack": ["emoji.slack-edge.com"],
        // Jira Cloud's avatars: Gravatar, falling back to Atlassian's initials pictures.
        "jira": ["secure.gravatar.com", "www.gravatar.com", "atl-paas.net"],
        // Bitbucket Cloud's avatars: the same.
        "bitbucket": ["secure.gravatar.com", "www.gravatar.com", "atl-paas.net"],
    ]
}

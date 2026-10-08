import RemoraCore

/// Every source the app knows about. Adding an integration means adding it here.
public enum PluginRegistry {
    public static let sources: [any SourcePlugin.Type] = [
        GitHubPlugin.self,
        GitLabPlugin.self,
        SlackPlugin.self,
        LinearPlugin.self,
        NotionPlugin.self,
    ]

    public static let assistants: [any AssistantPlugin.Type] = [
        ClaudeCodePlugin.self,
        ClaudePlugin.self,
    ]

    public static var manifests: [PluginManifest] { sources.map { $0.manifest } }
    public static var assistantManifests: [PluginManifest] { assistants.map { $0.manifest } }

    public static func manifest(_ id: String) -> PluginManifest? {
        (manifests + assistantManifests).first { $0.id == id }
    }

    public static func isAssistant(_ id: String) -> Bool {
        assistants.contains { $0.manifest.id == id }
    }

    public static func makeAssistant(_ account: Account, secrets: [String: String], http: HTTPClient) throws -> any AssistantPlugin {
        guard let type = assistants.first(where: { $0.manifest.id == account.pluginID }) else {
            throw PluginError.unknownPlugin(account.pluginID)
        }
        let values = account.settings.merging(secrets) { _, secret in secret }
        return try type.init(config: PluginConfig(accountID: account.id, values: values), http: http)
    }

    public static func source(_ id: String) -> (any SourcePlugin.Type)? {
        sources.first { $0.manifest.id == id }
    }

    public static func make(_ account: Account, secrets: [String: String], http: HTTPClient) throws -> any SourcePlugin {
        guard let type = source(account.pluginID) else { throw PluginError.unknownPlugin(account.pluginID) }
        let values = account.settings.merging(secrets) { _, secret in secret }
        return try type.init(config: PluginConfig(accountID: account.id, values: values), http: http)
    }
}

import Foundation

/// What the assistant may see and use, from the user's choices and the organization's MDM keys
/// `AIExcludedSources` and `AllowedAIModels`. Which assistant (Claude Code or the Claude API) is `AllowedPlugins`.
public struct AssistantPolicy: Equatable, Sendable {
    /// Sources whose items never reach the assistant: plugin IDs, or "reminders" for your own reminders.
    public var excludedSources: Set<String>
    /// Models the assistant may use. Nil: any.
    public var allowedModels: [String]?

    /// The assistant settings that name a model, with the value used when the field is empty.
    public static let modelFields = ["model", "digestModel"]

    public init(excludedSources: Set<String> = [], allowedModels: [String]? = nil) {
        self.excludedSources = excludedSources
        self.allowedModels = allowedModels.map {
            $0.map { $0.trimmingCharacters(in: .whitespaces) }.filter { !$0.isEmpty }
        }
    }

    /// The items the assistant may read.
    public func items(_ items: [InboxItem]) -> [InboxItem] {
        items.filter { !excludedSources.contains($0.pluginID) }
    }

    public func allows(_ item: InboxItem) -> Bool {
        !excludedSources.contains(item.pluginID)
    }

    /// An assistant account's settings with every model field kept within `allowedModels`: a model that isn't
    /// allowed (or an empty field, whose default may not be) becomes the first allowed one.
    public func constrained(_ settings: [String: String], defaults: [String: String]) -> [String: String] {
        guard let allowed = allowedModels, let first = allowed.first else { return settings }
        var settings = settings
        for key in Self.modelFields {
            let value = settings[key]?.trimmingCharacters(in: .whitespaces) ?? ""
            let effective = value.isEmpty ? (defaults[key] ?? "") : value
            if !allowed.contains(effective) { settings[key] = first }
        }
        return settings
    }
}

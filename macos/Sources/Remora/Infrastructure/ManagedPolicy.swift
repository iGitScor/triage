import Foundation
import RemoraCore

/// The privacy policy: the organization's MDM settings win over the user's own choices.
///
/// Managed keys, in the `fr.igitscor.remora` preference domain:
/// `AllowedPlugins` (array of plugin IDs), `AllowExternalAI` (Bool), `AllowRemoteImages` (Bool),
/// `AIExcludedSources` (array of plugin IDs, or "reminders"), `AllowedAIModels` (array of model IDs).
/// Settings: `HiddenContentSources` (array of plugin IDs, or "reminders", added to the user's),
/// `RefreshMinutes` (Int, 1 to 60), `OpenInApps` (Bool), `ClaudeCodePath` (String).
/// Updates: `AutomaticUpdates` (Bool). True checks every day, locked on; false turns updating off entirely,
/// Check now included, for fleets that deploy each version themselves.
enum ManagedPolicy {
    static let allowedPluginsKey = "AllowedPlugins"
    static let externalAIKey = "AllowExternalAI"
    static let remoteImagesKey = "AllowRemoteImages"
    static let aiExcludedSourcesKey = "AIExcludedSources"
    static let allowedAIModelsKey = "AllowedAIModels"
    static let hiddenContentKey = "HiddenContentSources"
    static let refreshMinutesKey = "RefreshMinutes"
    static let openInAppsKey = "OpenInApps"
    static let claudeCodePathKey = "ClaudeCodePath"
    static let automaticUpdatesKey = "AutomaticUpdates"

    /// What Remora acts on: your preferences with the organization's settings in place. Hiding message content can only
    /// be added to; a refresh interval outside 1 to 60 minutes, or not a number, is ignored.
    static func effective(_ preferences: Preferences, managed: ManagedValues = ManagedDefaults()) -> Preferences {
        var effective = preferences
        if let value = managed.forced(hiddenContentKey) {
            effective.hiddenContentPlugins.formUnion(strings(value))
        }
        if let value = managed.forced(refreshMinutesKey), let minutes = int(value), (1...60).contains(minutes) {
            effective.refreshMinutes = minutes
        }
        if let value = managed.forced(openInAppsKey) {
            effective.openInApps = bool(value)
        }
        if let value = managed.forced(automaticUpdatesKey) {
            effective.checkForUpdates = bool(value)
        }
        return effective
    }

    /// False when the organization turned updates off: no check at all, not even Check now.
    static func updatesAllowed(_ managed: ManagedValues = ManagedDefaults()) -> Bool {
        managed.forced(automaticUpdatesKey).map(bool) ?? true
    }

    /// The `claude` the organization chose, which wins over the one in the account.
    static func claudeCodePath(_ managed: ManagedValues = ManagedDefaults()) -> String? {
        (managed.forced(claudeCodePathKey) as? String).flatMap {
            $0.trimmingCharacters(in: .whitespaces).isEmpty ? nil : $0
        }
    }

    /// Whether a setting is the organization's: its control is then locked in Settings.
    static func isForced(_ key: String, _ managed: ManagedValues = ManagedDefaults()) -> Bool {
        managed.forced(key) != nil
    }

    /// Sources whose content the organization hides: their toggles are locked on.
    static func managedHiddenContent(_ managed: ManagedValues = ManagedDefaults()) -> Set<String> {
        managed.forced(hiddenContentKey).map { Set(strings($0)) } ?? []
    }

    /// True when the organization set at least one key: the Privacy settings are then read-only.
    static func isManaged(_ managed: ManagedValues = ManagedDefaults()) -> Bool {
        [allowedPluginsKey, externalAIKey, remoteImagesKey, aiExcludedSourcesKey, allowedAIModelsKey]
            .contains { managed.forced($0) != nil }
    }

    /// Sources the organization keeps away from the assistant: locked off in Settings.
    static func managedAIExclusions(_ managed: ManagedValues = ManagedDefaults()) -> Set<String> {
        guard let value = managed.forced(aiExcludedSourcesKey) else { return [] }
        return Set(strings(value))
    }

    /// The user's exclusions plus the organization's; the models only the organization restricts.
    static func assistantPolicy(user preferences: Preferences, managed: ManagedValues = ManagedDefaults())
        -> AssistantPolicy
    {
        let models = managed.forced(allowedAIModelsKey).map(strings)
        return AssistantPolicy(
            excludedSources: preferences.assistantExcludedSources.union(managedAIExclusions(managed)),
            allowedModels: models
        )
    }

    static func policy(user preferences: Preferences, managed: ManagedValues = ManagedDefaults()) -> CompliancePolicy {
        var policy = CompliancePolicy(
            allowedPlugins: preferences.allowedPlugins.map(Set.init),
            allowExternalAI: preferences.allowExternalAI,
            allowRemoteImages: preferences.allowRemoteImages
        )
        if let value = managed.forced(allowedPluginsKey) {
            policy.allowedPlugins = Set(strings(value))
        }
        if let value = managed.forced(externalAIKey) {
            policy.allowExternalAI = bool(value)
        }
        if let value = managed.forced(remoteImagesKey) {
            policy.allowRemoteImages = bool(value)
        }
        return policy
    }

    /// A list the organization set; a value of another type allows nothing, as `stringArray(forKey:)` did.
    private static func strings(_ value: Any) -> [String] {
        value as? [String] ?? []
    }

    private static func int(_ value: Any) -> Int? {
        if let number = value as? NSNumber { return number.intValue }
        return (value as? String).flatMap { Int($0.trimmingCharacters(in: .whitespaces)) }
    }

    /// Read as `UserDefaults.bool(forKey:)` does: a number, or "YES" / "true" / a number as text.
    private static func bool(_ value: Any) -> Bool {
        if let number = value as? NSNumber { return number.boolValue }
        if let text = value as? String { return ["yes", "true"].contains(text.lowercased()) || (Int(text) ?? 0) != 0 }
        return false
    }
}

/// The values an organization forced through MDM: the managed preferences in the app, a dictionary in tests.
protocol ManagedValues {
    /// The value of a key the organization set, nil when it didn't.
    func forced(_ key: String) -> Any?
}

/// `fr.igitscor.remora`'s managed preferences.
struct ManagedDefaults: ManagedValues {
    func forced(_ key: String) -> Any? {
        UserDefaults.standard.objectIsForced(forKey: key)
            ? UserDefaults.standard.object(forKey: key) ?? [String]() : nil
    }
}

import Foundation
import RemoraCore

/// The privacy policy: the organization's MDM settings win over the user's own choices.
///
/// Managed keys, in the `fr.igitscor.remora` preference domain:
/// `AllowedPlugins` (array of plugin IDs), `AllowExternalAI` (Bool), `AllowRemoteImages` (Bool).
enum ManagedPolicy {
    static let allowedPluginsKey = "AllowedPlugins"
    static let externalAIKey = "AllowExternalAI"
    static let remoteImagesKey = "AllowRemoteImages"

    /// True when the organization set at least one key: the Privacy settings are then read-only.
    static var isManaged: Bool {
        [allowedPluginsKey, externalAIKey, remoteImagesKey].contains { UserDefaults.standard.objectIsForced(forKey: $0) }
    }

    static func policy(user preferences: Preferences) -> CompliancePolicy {
        let defaults = UserDefaults.standard
        var policy = CompliancePolicy(
            allowedPlugins: preferences.allowedPlugins.map(Set.init),
            allowExternalAI: preferences.allowExternalAI,
            allowRemoteImages: preferences.allowRemoteImages
        )
        if defaults.objectIsForced(forKey: allowedPluginsKey) {
            policy.allowedPlugins = Set(defaults.stringArray(forKey: allowedPluginsKey) ?? [])
        }
        if defaults.objectIsForced(forKey: externalAIKey) {
            policy.allowExternalAI = defaults.bool(forKey: externalAIKey)
        }
        if defaults.objectIsForced(forKey: remoteImagesKey) {
            policy.allowRemoteImages = defaults.bool(forKey: remoteImagesKey)
        }
        return policy
    }
}

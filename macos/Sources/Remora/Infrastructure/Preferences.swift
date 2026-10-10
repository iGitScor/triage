import Foundation
import RemoraCore

struct Preferences: Codable, Equatable {
    enum Appearance: String, Codable, CaseIterable, Identifiable {
        case system, light, dark
        var id: String { rawValue }
        var label: String { L(rawValue.capitalized) }
    }

    /// MacOS has no text size apps can follow, so Remora has its own.
    enum TextSize: String, Codable, CaseIterable, Identifiable {
        case standard, large, larger
        var id: String { rawValue }
        var scale: CGFloat {
            switch self {
            case .standard: 1
            case .large: 1.15
            case .larger: 1.3
            }
        }
        var label: String {
            switch self {
            case .standard: L("Default")
            case .large: L("Large")
            case .larger: L("Larger")
            }
        }
    }

    enum MenuBarCount: String, Codable, CaseIterable, Identifiable {
        case waitingOnMe, everything, hidden
        var id: String { rawValue }
        var label: String {
            switch self {
            case .waitingOnMe: L("My turn")
            case .everything: L("Everything in my inbox")
            case .hidden: L("Nothing")
            }
        }
    }

    var refreshMinutes = 5
    var menuBarCount = MenuBarCount.waitingOnMe
    var countPerSource = false
    var brandedMenuBar = true
    /// While a task is in progress, the menu bar shows only it.
    var focusWhileInProgress = true
    var notifyArrivals = true
    var notifyStatusChanges = true
    /// Integrations whose notifications only say where something happened, not what was written.
    var hiddenContentPlugins: Set<String> = []
    /// Sources whose items are never sent to the assistant (plugin IDs, or "reminders").
    var assistantExcludedSources: Set<String> = []
    var wakeOnActivity = true
    var appearance = Appearance.system
    var textSize = TextSize.standard
    var briefCacheMinutes = 30
    var wholeInboxBrief = true
    /// Privacy: plugins allowed to connect (nil = all), external AI (off by default), avatars.
    var allowedPlugins: [String]?
    var allowExternalAI = false
    var allowRemoteImages = true
    var openInApps = true
    /// Off until the user turns it on; nothing calls home before.
    var checkForUpdates = false

    init() {}

    /// Missing keys keep their defaults, so adding a preference never resets the others.
    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let defaults = Preferences()
        refreshMinutes = try container.decodeIfPresent(Int.self, forKey: .refreshMinutes) ?? defaults.refreshMinutes
        menuBarCount = try container.decodeIfPresent(MenuBarCount.self, forKey: .menuBarCount) ?? defaults.menuBarCount
        countPerSource = try container.decodeIfPresent(Bool.self, forKey: .countPerSource) ?? defaults.countPerSource
        brandedMenuBar = try container.decodeIfPresent(Bool.self, forKey: .brandedMenuBar) ?? defaults.brandedMenuBar
        focusWhileInProgress = try container.decodeIfPresent(Bool.self, forKey: .focusWhileInProgress) ?? defaults.focusWhileInProgress
        notifyArrivals = try container.decodeIfPresent(Bool.self, forKey: .notifyArrivals) ?? defaults.notifyArrivals
        notifyStatusChanges = try container.decodeIfPresent(Bool.self, forKey: .notifyStatusChanges) ?? defaults.notifyStatusChanges
        hiddenContentPlugins = try container.decodeIfPresent(Set<String>.self, forKey: .hiddenContentPlugins) ?? defaults.hiddenContentPlugins
        assistantExcludedSources = try container.decodeIfPresent(Set<String>.self, forKey: .assistantExcludedSources) ?? defaults.assistantExcludedSources
        wakeOnActivity = try container.decodeIfPresent(Bool.self, forKey: .wakeOnActivity) ?? defaults.wakeOnActivity
        appearance = try container.decodeIfPresent(Appearance.self, forKey: .appearance) ?? defaults.appearance
        textSize = try container.decodeIfPresent(TextSize.self, forKey: .textSize) ?? defaults.textSize
        briefCacheMinutes = try container.decodeIfPresent(Int.self, forKey: .briefCacheMinutes) ?? defaults.briefCacheMinutes
        wholeInboxBrief = try container.decodeIfPresent(Bool.self, forKey: .wholeInboxBrief) ?? defaults.wholeInboxBrief
        allowedPlugins = try container.decodeIfPresent([String].self, forKey: .allowedPlugins)
        allowExternalAI = try container.decodeIfPresent(Bool.self, forKey: .allowExternalAI) ?? defaults.allowExternalAI
        allowRemoteImages = try container.decodeIfPresent(Bool.self, forKey: .allowRemoteImages) ?? defaults.allowRemoteImages
        openInApps = try container.decodeIfPresent(Bool.self, forKey: .openInApps) ?? defaults.openInApps
        checkForUpdates = try container.decodeIfPresent(Bool.self, forKey: .checkForUpdates) ?? defaults.checkForUpdates
    }
}

import Foundation
import RemoraCore

struct Preferences: Codable, Equatable {
    enum Appearance: String, Codable, CaseIterable, Identifiable {
        case system, light, dark
        var id: String { rawValue }
        var label: String { L(rawValue.capitalized) }
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
    var notifyArrivals = true
    var notifyStatusChanges = true
    var wakeOnActivity = true
    var appearance = Appearance.system
    var briefCacheMinutes = 30
    var wholeInboxBrief = true
    /// Privacy: plugins allowed to connect (nil = all), external AI (off by default), avatars.
    var allowedPlugins: [String]?
    var allowExternalAI = false
    var allowRemoteImages = true
    var openInApps = true

    init() {}

    /// Missing keys keep their defaults, so adding a preference never resets the others.
    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let defaults = Preferences()
        refreshMinutes = try container.decodeIfPresent(Int.self, forKey: .refreshMinutes) ?? defaults.refreshMinutes
        menuBarCount = try container.decodeIfPresent(MenuBarCount.self, forKey: .menuBarCount) ?? defaults.menuBarCount
        countPerSource = try container.decodeIfPresent(Bool.self, forKey: .countPerSource) ?? defaults.countPerSource
        brandedMenuBar = try container.decodeIfPresent(Bool.self, forKey: .brandedMenuBar) ?? defaults.brandedMenuBar
        notifyArrivals = try container.decodeIfPresent(Bool.self, forKey: .notifyArrivals) ?? defaults.notifyArrivals
        notifyStatusChanges = try container.decodeIfPresent(Bool.self, forKey: .notifyStatusChanges) ?? defaults.notifyStatusChanges
        wakeOnActivity = try container.decodeIfPresent(Bool.self, forKey: .wakeOnActivity) ?? defaults.wakeOnActivity
        appearance = try container.decodeIfPresent(Appearance.self, forKey: .appearance) ?? defaults.appearance
        briefCacheMinutes = try container.decodeIfPresent(Int.self, forKey: .briefCacheMinutes) ?? defaults.briefCacheMinutes
        wholeInboxBrief = try container.decodeIfPresent(Bool.self, forKey: .wholeInboxBrief) ?? defaults.wholeInboxBrief
        allowedPlugins = try container.decodeIfPresent([String].self, forKey: .allowedPlugins)
        allowExternalAI = try container.decodeIfPresent(Bool.self, forKey: .allowExternalAI) ?? defaults.allowExternalAI
        allowRemoteImages = try container.decodeIfPresent(Bool.self, forKey: .allowRemoteImages) ?? defaults.allowRemoteImages
        openInApps = try container.decodeIfPresent(Bool.self, forKey: .openInApps) ?? defaults.openInApps
    }
}

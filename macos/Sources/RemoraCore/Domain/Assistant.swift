import Foundation

/// A daily-brief style triage of the inbox.
public struct Brief: Codable, Equatable, Sendable {
    public struct Focus: Codable, Equatable, Sendable {
        public var id: String
        public var reason: String

        public init(id: String, reason: String) {
            self.id = id
            self.reason = reason
        }
    }

    public var summary: String
    public var focus: [Focus]
    public var createdAt: Date

    public init(summary: String, focus: [Focus], createdAt: Date = .now) {
        self.summary = summary
        self.focus = focus
        self.createdAt = createdAt
    }

    /// When a new brief may be written; until then the current one is reused.
    public func freshUntil(cacheMinutes: Int) -> Date {
        createdAt.addingTimeInterval(Double(max(0, cacheMinutes)) * 60)
    }

    public func isFresh(cacheMinutes: Int, now: Date = .now) -> Bool {
        now < freshUntil(cacheMinutes: cacheMinutes)
    }
}

/// A one- or two-sentence take on a single bundle ("To review", "Mentions"…).
public struct BundleSummary: Codable, Equatable, Sendable {
    public var text: String
    public var itemIDs: [String]
    public var createdAt: Date

    public init(text: String, items: [InboxItem], createdAt: Date = .now) {
        self.text = text
        self.itemIDs = items.map(\.id).sorted()
        self.createdAt = createdAt
    }

    /// Reused only while recent and while the bundle still holds the same items.
    public func isFresh(for items: [InboxItem], cacheMinutes: Int, now: Date = .now) -> Bool {
        now < createdAt.addingTimeInterval(Double(max(0, cacheMinutes)) * 60) && itemIDs == items.map(\.id).sorted()
    }
}

/// A snoozed item as the assistant sees it.
public struct SnoozedItem: Sendable {
    public var item: InboxItem
    public var snooze: Snooze
    /// How many times it was snoozed lately.
    public var times: Int

    public init(item: InboxItem, snooze: Snooze, times: Int) {
        self.item = item
        self.snooze = snooze
        self.times = times
    }
}

/// What the assistant proposes for one snoozed item.
public struct TriageSuggestion: Codable, Equatable, Sendable, Identifiable {
    public enum Action: String, Codable, CaseIterable, Sendable {
        case keep, reschedule, done, now

        /// Ticked before the user looks. Only rescheduling, which keeps the item snoozed and is easy to undo:
        /// "done" hides an item and "now" opens its link, and the suggestions come from a model reading titles that
        /// anyone can write, so those two are the user's own choice.
        public var preselected: Bool { self == .reschedule }
    }

    /// What "Apply" would do: the preselected suggestions, plus or minus the ones the user ticked or unticked.
    /// "Keep" is never applied.
    public static func selection(_ suggestions: [TriageSuggestion], toggled: Set<String>) -> [TriageSuggestion] {
        suggestions.filter { $0.action != .keep && $0.action.preselected != toggled.contains($0.id) }
    }

    public var id: String
    public var action: Action
    public var until: Date?
    public var reason: String

    public init(id: String, action: Action, until: Date? = nil, reason: String) {
        self.id = id
        self.action = action
        self.until = until
        self.reason = reason
    }
}

/// An AI integration that helps triage the inbox.
public protocol AssistantPlugin: Sendable {
    static var manifest: PluginManifest { get }
    init(config: PluginConfig, http: HTTPClient) throws
    /// The whole inbox: what to handle first.
    func brief(_ items: [InboxItem], now: Date) async throws -> Brief
    /// One bundle in two short sentences, with a lighter model.
    func digest(_ items: [InboxItem], topic: String, now: Date) async throws -> String
    /// Proposes what to do with each snoozed item.
    func triage(_ items: [SnoozedItem], now: Date) async throws -> [TriageSuggestion]
}

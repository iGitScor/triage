import Foundation

/// One inbox item as this Mac's Spotlight index may hold it (IDEA-38): its title and where it comes from, never the
/// message preview, the people or the link.
public struct SpotlightEntry: Equatable, Hashable, Sendable {
    /// The item's id, which a Spotlight hit gives back to open it.
    public var id: String
    /// The account's id, so disconnecting removes that account's items at once; "reminders" for reminders.
    public var domain: String
    public var title: String
    /// "GitHub · acme/api".
    public var context: String

    public init(id: String, domain: String, title: String, context: String) {
        self.id = id
        self.domain = domain
        self.title = title
        self.context = context
    }
}

/// What goes to Spotlight and what leaves it. Off by default; a tool whose content is hidden never goes there.
public enum SpotlightIndex {
    public static let remindersDomain = "reminders"

    /// What the index should hold now. `sourceNames`: plugin id → the name shown ("GitHub"), "reminders" included.
    public static func entries(
        items: [InboxItem], enabled: Bool, hiddenContent: Set<String>, sourceNames: [String: String]
    ) -> [SpotlightEntry] {
        guard enabled else { return [] }
        var seen = Set<String>()
        return items.compactMap { item in
            guard !hiddenContent.contains(item.pluginID), seen.insert(item.id).inserted else { return nil }
            let title = item.title.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !title.isEmpty else { return nil }
            let isReminder = item.pluginID == remindersDomain
            let source = sourceNames[item.pluginID] ?? item.pluginID
            let context = item.context.trimmingCharacters(in: .whitespacesAndNewlines)
            return SpotlightEntry(
                id: item.id,
                domain: isReminder ? remindersDomain : item.accountID.uuidString,
                title: title,
                context: isReminder || context.isEmpty ? source : "\(source) · \(context)"
            )
        }
    }

    public struct Changes: Equatable, Sendable {
        /// Everything goes first: the index may hold what an earlier launch put there.
        public var deleteAll = false
        public var delete: [String] = []
        public var index: [SpotlightEntry] = []

        public init(deleteAll: Bool = false, delete: [String] = [], index: [SpotlightEntry] = []) {
            self.deleteAll = deleteAll
            self.delete = delete
            self.index = index
        }

        public var isEmpty: Bool { !deleteAll && delete.isEmpty && index.isEmpty }
    }

    /// From what was indexed (nil: unknown, at launch) to what should be. Off, everything goes.
    public static func changes(indexed: [String: SpotlightEntry]?, wanted: [SpotlightEntry]) -> Changes {
        guard let indexed else { return Changes(deleteAll: true, index: wanted) }
        if wanted.isEmpty { return indexed.isEmpty ? Changes() : Changes(deleteAll: true) }
        let ids = Set(wanted.map(\.id))
        return Changes(
            delete: indexed.keys.filter { !ids.contains($0) }.sorted(),
            index: wanted.filter { indexed[$0.id] != $0 }
        )
    }
}

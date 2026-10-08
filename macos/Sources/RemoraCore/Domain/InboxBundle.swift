/// Items are grouped by what they ask of you, not by where they come from (Google Inbox "bundles").
///
/// Plugins file items under a *kind* (mentions, direct messages, your merge requests…);
/// `VerbClassifier` then moves them to a *verb* (to reply, to fix, to read…).
public struct InboxBundle: Hashable, Codable, Sendable, Identifiable {
    public var id: String
    public var title: String
    public var symbol: String
    public var rank: Int

    public init(id: String, title: String, symbol: String, rank: Int) {
        self.id = id
        self.title = title
        self.symbol = symbol
        self.rank = rank
    }

    /// Whose turn the bundle is, when the bundle alone tells. Kinds leave it to `needsAction`.
    public var isMine: Bool? {
        switch id {
        case Self.awaiting.id: false
        case Self.authored.id, Self.mentions.id, Self.directMessages.id: nil
        default: true
        }
    }

    /// Shown, but neither counted in the menu bar nor announced.
    public var isQuiet: Bool { id == Self.read.id }
}

// MARK: Verbs

extension InboxBundle {
    public static let reminders = InboxBundle(id: "reminders", title: "Reminders", symbol: "alarm", rank: 0)
    public static let reply = InboxBundle(id: "verb.reply", title: "To reply", symbol: "arrowshape.turn.up.left", rank: 1)
    public static let reviews = InboxBundle(id: "code.review", title: "To review", symbol: "eye", rank: 2)
    public static let fix = InboxBundle(id: "verb.fix", title: "To fix", symbol: "wrench.and.screwdriver", rank: 3)
    public static let merge = InboxBundle(id: "verb.merge", title: "Ready to merge", symbol: "arrow.triangle.merge", rank: 4)
    public static let tasks = InboxBundle(id: "docs.tasks", title: "To do", symbol: "checklist", rank: 5)
    public static let read = InboxBundle(id: "verb.read", title: "To read", symbol: "text.alignleft", rank: 8)
    public static let awaiting = InboxBundle(id: "verb.awaiting", title: "Waiting on others", symbol: "hourglass", rank: 9)
}

// MARK: Kinds, as reported by plugins

extension InboxBundle {
    public static let mentions = InboxBundle(id: "chat.mentions", title: "Mentions", symbol: "at", rank: 6)
    public static let directMessages = InboxBundle(id: "chat.direct", title: "Direct messages", symbol: "bubble.left.and.bubble.right", rank: 7)
    public static let authored = InboxBundle(id: "code.authored", title: "Your merge requests", symbol: "arrow.triangle.pull", rank: 9)
}

import Foundation

/// Anything that deserves attention: a review request, a Slack mention, a Notion task, a reminder.
public struct InboxItem: Identifiable, Hashable, Codable, Sendable {
    public var id: String
    public var accountID: UUID
    public var pluginID: String
    public var bundle: InboxBundle
    public var title: String
    public var context: String
    public var preview: String?
    public var url: URL?
    /// Opens the item in the tool's desktop app (slack://, linear://) when one is installed.
    public var appURL: URL?
    public var author: Person?
    public var participants: [Person]
    public var badges: [Badge]
    public var date: Date
    public var needsAction: Bool
    /// From the source, when it has one (Linear priority, backlog…). Nil means normal.
    public var priority: Priority?
    /// When it's due, when the source says so.
    public var due: Date?
    /// People the source suggests (e.g. GitHub's suggested reviewers).
    public var suggestedPeople: [Person]?
    /// Files a pull/merge request touches: paths and line counts only, never code.
    public var changes: ChangeSet?
    /// When it stops being worth showing (an app's reminder of an event that has started): it then leaves the
    /// inbox as if cleared, unless you pinned or started it.
    public var expires: Date?

    public init(
        id: String,
        accountID: UUID,
        pluginID: String,
        bundle: InboxBundle,
        title: String,
        context: String,
        preview: String? = nil,
        url: URL? = nil,
        author: Person? = nil,
        participants: [Person] = [],
        badges: [Badge] = [],
        date: Date,
        needsAction: Bool = false,
        priority: Priority? = nil,
        due: Date? = nil,
        suggestedPeople: [Person]? = nil,
        appURL: URL? = nil,
        changes: ChangeSet? = nil
    ) {
        self.id = id
        self.accountID = accountID
        self.pluginID = pluginID
        self.bundle = bundle
        self.title = title
        self.context = context
        self.preview = preview
        self.url = url
        self.author = author
        self.participants = participants
        self.badges = badges
        self.date = date
        self.needsAction = needsAction
        self.priority = priority
        self.due = due
        self.suggestedPeople = suggestedPeople
        self.appURL = appURL
        self.changes = changes
    }

    /// Changes whenever something noteworthy happens, so Done or snoozed items can resurface.
    public var fingerprint: String {
        let badgeIDs = badges.map(\.id).sorted().joined(separator: ",")
        return "\(date.timeIntervalSince1970)|\(badgeIDs)"
    }

    public func hasBadge(_ id: String) -> Bool { badges.contains { $0.id == id } }
}

public struct ChangeSet: Hashable, Codable, Sendable {
    public var files: [ChangedFile]
    /// All files changed, which can exceed the listed ones.
    public var fileCount: Int

    public init(files: [ChangedFile], fileCount: Int? = nil) {
        self.files = files
        self.fileCount = max(fileCount ?? files.count, files.count)
    }
}

public struct ChangedFile: Hashable, Codable, Sendable {
    public var path: String
    public var additions: Int?
    public var deletions: Int?

    public init(path: String, additions: Int? = nil, deletions: Int? = nil) {
        self.path = path
        self.additions = additions
        self.deletions = deletions
    }

    public var lines: Int { (additions ?? 0) + (deletions ?? 0) }
}

public struct Person: Hashable, Codable, Sendable, Identifiable {
    public var name: String
    public var avatarURL: URL?
    public var tone: Tone?

    public var id: String { name }

    public init(name: String, avatarURL: URL? = nil, tone: Tone? = nil) {
        self.name = name
        self.avatarURL = avatarURL
        self.tone = tone
    }

    public var initials: String {
        let parts = name.split(whereSeparator: { $0 == " " || $0 == "." || $0 == "-" || $0 == "_" })
        return parts.prefix(2).compactMap(\.first).map(String.init).joined().uppercased()
    }
}

/// A status chip. When `notify` is set, its first appearance on an item triggers a notification.
public struct Badge: Hashable, Codable, Sendable, Identifiable {
    public var id: String
    public var label: String
    public var symbol: String?
    public var tone: Tone
    public var notify: Notify?

    public struct Notify: Hashable, Codable, Sendable {
        public var title: String
        public init(title: String) { self.title = title }
    }

    public init(id: String, label: String, symbol: String? = nil, tone: Tone, notify: Notify? = nil) {
        self.id = id
        self.label = label
        self.symbol = symbol
        self.tone = tone
        self.notify = notify
    }
}

public enum Priority: Int, Hashable, Codable, Sendable, Comparable {
    case low, normal, high, urgent

    public static func < (a: Priority, b: Priority) -> Bool { a.rawValue < b.rawValue }
}

public enum Tone: String, Hashable, Codable, Sendable {
    case accent, positive, negative, warning, neutral
}

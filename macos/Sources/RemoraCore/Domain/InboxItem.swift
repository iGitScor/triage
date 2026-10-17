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
    /// How a reminder repeats. Set on reminders only; not part of the fingerprint.
    public var recurrence: Recurrence?
    /// Emoji reactions on the message (Slack); nil for tools without them. Not in the fingerprint.
    public var reactions: [Reaction]?

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
        let badgeIDs = badges.map(\.id.rawValue).sorted().joined(separator: ",")
        return "\(date.timeIntervalSince1970)|\(badgeIDs)"
    }

    public func hasBadge(_ id: BadgeID) -> Bool { badges.contains { $0.id == id } }
}

/// Item ids are the account's id, a slash, then the plugin's own id. Two accounts on the same tool never share an
/// item (Done, snoozes and pins are kept by id), and an account connected again finds its items (`moved`).
extension InboxItem {
    public static func makeID(account: UUID, local: String) -> String { accountPrefix(account) + local }

    /// What starts the id of every item of this account, to find them all (notifications at a disconnect).
    public static func accountPrefix(_ account: UUID) -> String { "\(account.uuidString)/" }

    /// The plugin's own part of the id; nil when the id doesn't start with this item's account.
    public var localID: String? {
        let prefix = Self.accountPrefix(accountID)
        return id.hasPrefix(prefix) ? String(id.dropFirst(prefix.count)) : nil
    }
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
    /// A reviewer's stand; nil while they haven't reviewed. The interface draws its colour and glyph from it.
    public var review: ReviewState?

    public var id: String { name }

    public init(name: String, avatarURL: URL? = nil, review: ReviewState? = nil) {
        self.name = name
        self.avatarURL = avatarURL
        self.review = review
    }

    private enum CodingKeys: String, CodingKey {
        case name, avatarURL, review
        /// Before 0.10 a reviewer's stand was only the colour of their avatar: read in saved inboxes, never written.
        case tone
    }

    public init(from decoder: any Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        name = try container.decode(String.self, forKey: .name)
        avatarURL = try container.decodeIfPresent(URL.self, forKey: .avatarURL)
        review = try container.decodeIfPresent(ReviewState.self, forKey: .review)
        if review == nil {
            switch try container.decodeIfPresent(Tone.self, forKey: .tone) {
            case .accent, .positive: review = .approved
            case .negative: review = .changesRequested
            default: break
            }
        }
    }

    public func encode(to encoder: any Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(name, forKey: .name)
        try container.encodeIfPresent(avatarURL, forKey: .avatarURL)
        try container.encodeIfPresent(review, forKey: .review)
    }

    public var initials: String {
        let parts = name.split(whereSeparator: { $0 == " " || $0 == "." || $0 == "-" || $0 == "_" })
        return parts.prefix(2).compactMap(\.first).map(String.init).joined().uppercased()
    }
}

/// Where a reviewer stands on a pull request.
public enum ReviewState: String, Hashable, Codable, Sendable {
    case approved, changesRequested
}

/// A badge's id: the ones plugins write and the rules read back are declared here, so a typo is a compile error,
/// not an item quietly in the wrong verb. Open, not an enum: a saved inbox holding an id this version doesn't know
/// still loads. Saved as the plain string, so renaming one is a format change.
public struct BadgeID: RawRepresentable, Hashable, Codable, Sendable {
    public let rawValue: String
    public init(rawValue: String) { self.rawValue = rawValue }

    public init(from decoder: any Decoder) throws {
        rawValue = try decoder.singleValueContainer().decode(String.self)
    }

    public func encode(to encoder: any Encoder) throws {
        var container = encoder.singleValueContainer()
        try container.encode(rawValue)
    }

    public static let approved = BadgeID(rawValue: "approved")
    public static let changes = BadgeID(rawValue: "changes")
    public static let draft = BadgeID(rawValue: "draft")
    public static let conflicts = BadgeID(rawValue: "conflicts")
    public static let checksPassing = BadgeID(rawValue: "checks.passing")
    public static let checksFailing = BadgeID(rawValue: "checks.failing")
    public static let checksRunning = BadgeID(rawValue: "checks.running")
    public static let comments = BadgeID(rawValue: "comments")
    public static let diff = BadgeID(rawValue: "diff")
    public static let status = BadgeID(rawValue: "status")
    public static let priorityHigh = BadgeID(rawValue: "priority.high")
    public static let priorityUrgent = BadgeID(rawValue: "priority.urgent")
    public static let due = BadgeID(rawValue: "due")
    public static let dueToday = BadgeID(rawValue: "due.today")
    public static let overdue = BadgeID(rawValue: "overdue")
    public static let inProgress = BadgeID(rawValue: "inProgress")
    public static let invitation = BadgeID(rawValue: "invitation")
    public static let flagged = BadgeID(rawValue: "flagged")
    public static let event = BadgeID(rawValue: "event")
    public static let reminder = BadgeID(rawValue: "reminder")
    public static let reason = BadgeID(rawValue: "reason")
    public static let times = BadgeID(rawValue: "times")
    public static let started = BadgeID(rawValue: "started")
    public static let link = BadgeID(rawValue: "link")
}

/// A status chip. When `notify` is set, its first appearance on an item triggers a notification.
public struct Badge: Hashable, Codable, Sendable, Identifiable {
    public var id: BadgeID
    public var label: String
    public var symbol: String?
    public var tone: Tone
    public var notify: Notify?

    public struct Notify: Hashable, Codable, Sendable {
        public var title: String
        public init(title: String) { self.title = title }
    }

    public init(id: BadgeID, label: String, symbol: String? = nil, tone: Tone, notify: Notify? = nil) {
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

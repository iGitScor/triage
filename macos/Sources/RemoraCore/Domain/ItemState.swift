import Foundation

/// What the user decided about an item. Kept locally, never sent to the source.
public struct ItemState: Hashable, Codable, Sendable {
    public var pinned = false
    public var done: Mark?
    public var snooze: Snooze?
    public var remindedAt: Date?

    public init(pinned: Bool = false, done: Mark? = nil, snooze: Snooze? = nil, remindedAt: Date? = nil) {
        self.pinned = pinned
        self.done = done
        self.snooze = snooze
        self.remindedAt = remindedAt
    }

    public var isEmpty: Bool { !pinned && done == nil && snooze == nil && remindedAt == nil }

    /// Remembers the item's fingerprint so new activity brings it back.
    public struct Mark: Hashable, Codable, Sendable {
        public var at: Date
        public var fingerprint: String
        /// Set when the user clears the Done tab: the item stays out of the inbox but leaves the list.
        public var clearedAt: Date?

        public init(at: Date, fingerprint: String, clearedAt: Date? = nil) {
            self.at = at
            self.fingerprint = fingerprint
            self.clearedAt = clearedAt
        }
    }
}

public struct Snooze: Hashable, Codable, Sendable {
    public enum Mode: String, Codable, Sendable, CaseIterable {
        /// Hide the item until the time comes.
        case hide
        /// Keep the item visible and send a notification at that time.
        case remind
    }

    public var until: Date
    public var mode: Mode
    public var note: String?
    public var fingerprint: String
    /// Why it was snoozed, when the user said so.
    public var reason: SnoozeReason?
    /// Comes back on any new activity, whatever the global setting.
    public var untilNews: Bool?

    public init(
        until: Date,
        mode: Mode,
        note: String? = nil,
        fingerprint: String,
        reason: SnoozeReason? = nil,
        untilNews: Bool? = nil
    ) {
        self.until = until
        self.mode = mode
        self.note = note
        self.fingerprint = fingerprint
        self.reason = reason
        self.untilNews = untilNews
    }
}

/// Why something gets postponed. Each reason leads to a different suggestion.
public enum SnoozeReason: String, Codable, CaseIterable, Sendable {
    case waiting, noTime, focus, notUrgent, motivation

    public var title: String {
        switch self {
        case .waiting: L("Waiting for someone")
        case .noTime: L("No time now")
        case .focus: L("Needs focus")
        case .notUrgent: L("Not urgent")
        case .motivation: L("Not feeling it")
        }
    }

    public var symbol: String {
        switch self {
        case .waiting: "person.2"
        case .noTime: "clock"
        case .focus: "brain.head.profile"
        case .notUrgent: "tortoise"
        case .motivation: "battery.25percent"
        }
    }
}

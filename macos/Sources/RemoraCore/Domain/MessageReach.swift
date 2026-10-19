import Foundation

/// How directly a message reached you (Slack, Teams), to order To reply, first to last: a one-to-one conversation,
/// a mention of you by name, a reply in a thread you started, a group conversation, a reply in a thread you only
/// wrote in. Only the order: everything stays in To reply and counted. A message from another tool (a GitHub, Jira
/// or Linear comment) stands with the mentions. As Windows (`MessageReach`).
public enum MessageReach: String, Codable, Sendable, CaseIterable, Comparable {
    case direct, mention, ownThread, group, thread

    public static func < (a: MessageReach, b: MessageReach) -> Bool {
        allCases.firstIndex(of: a)! < allCases.firstIndex(of: b)!
    }

    /// Unanswered this long, a one-to-one message or a mention of you says so.
    public static let longWait: TimeInterval = .day
}

extension InboxItem {
    /// What its row says of a message left in To reply: the same person wrote again since you last answered (only
    /// while their last message still asks something: a "never mind" went to Done), and a one-to-one message or a
    /// mention of you unanswered for a day. Elsewhere, neither: a follow-up that closes the conversation lifts
    /// nothing.
    func markedForReply(now: Date) -> InboxItem {
        var item = self
        guard item.bundle == .reply else {
            item.followedUp = nil
            return item
        }
        if item.followedUp == true {
            item.badges.append(Badge(id: .followedUp, label: L("Followed up"), symbol: "arrow.uturn.up", tone: .accent))
        }
        if let reach, reach <= .mention, now.timeIntervalSince(item.date) > MessageReach.longWait {
            item.badges.append(Badge(id: .age, label: L("Waiting over a day"), symbol: "clock", tone: .warning))
        }
        return item
    }
}

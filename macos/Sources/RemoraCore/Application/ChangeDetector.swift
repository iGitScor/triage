import Foundation

public struct Notice: Equatable, Sendable {
    public enum Kind: Sendable { case arrival, statusChange, reminder }

    public var kind: Kind
    public var itemID: String
    public var title: String
    public var subtitle: String
    public var body: String
    public var url: URL?

    public init(kind: Kind, itemID: String, title: String, subtitle: String, body: String, url: URL?) {
        self.kind = kind
        self.itemID = itemID
        self.title = title
        self.subtitle = subtitle
        self.body = body
        self.url = url
    }

    /// The summary of what arrived while Remora was closed: no item, so no Done or Snooze buttons.
    public var isSummary: Bool { itemID.hasSuffix(ChangeDetector.whileClosedSuffix) }

    /// Keeps what kind of notice it is and where it comes from, hides what was written.
    public func redacted() -> Notice {
        var notice = self
        notice.body = L(kind == .reminder ? "Reminder" : "New activity")
        return notice
    }
}

/// Compares two snapshots of the same account and tells what is worth a notification.
public enum ChangeDetector {
    public static func notices(previous: [InboxItem]?, current: [InboxItem]) -> [Notice] {
        guard let previous else { return [] }
        let before = Dictionary(previous.map { ($0.id, $0) }, uniquingKeysWith: { first, _ in first })

        return current.flatMap { item -> [Notice] in
            guard let old = before[item.id] else {
                guard item.needsAction else { return [] }
                return [
                    Notice(
                        kind: .arrival,
                        itemID: item.id,
                        title: L(item.bundle.title),
                        subtitle: item.context,
                        body: item.title,
                        url: item.url
                    )
                ]
            }
            return item.badges
                .filter { $0.notify != nil && !old.hasBadge($0.id) }
                .map { badge in
                    Notice(
                        kind: .statusChange,
                        itemID: item.id,
                        title: badge.notify?.title ?? badge.label,
                        subtitle: item.context,
                        body: item.title,
                        url: item.url
                    )
                }
        }
    }

    /// Ends the item ID of the summary notice: it points to no item, opening it opens the inbox.
    public static let whileClosedSuffix = "/while-closed"

    /// The notices of an account's first sync after launch, compared with its cached items: what arrived while
    /// Remora was closed. More than `limit` become one summary, so a week away doesn't bring a flood.
    public static func whileClosed(_ notices: [Notice], accountID: String, accountName: String, limit: Int = 3)
        -> [Notice]
    {
        guard notices.count > limit else { return notices }
        return [
            Notice(
                kind: .arrival,
                itemID: accountID + whileClosedSuffix,
                title: L("%d new while Remora was closed", notices.count),
                subtitle: accountName,
                body: L("Open Remora to see them."),
                url: nil
            )
        ]
    }
}

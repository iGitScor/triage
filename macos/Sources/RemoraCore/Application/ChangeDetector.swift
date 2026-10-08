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
}

/// Compares two snapshots of the same account and tells what is worth a notification.
public enum ChangeDetector {
    public static func notices(previous: [InboxItem]?, current: [InboxItem]) -> [Notice] {
        guard let previous else { return [] }
        let before = Dictionary(previous.map { ($0.id, $0) }, uniquingKeysWith: { first, _ in first })

        return current.flatMap { item -> [Notice] in
            guard let old = before[item.id] else {
                guard item.needsAction else { return [] }
                return [Notice(
                    kind: .arrival,
                    itemID: item.id,
                    title: L(item.bundle.title),
                    subtitle: item.context,
                    body: item.title,
                    url: item.url
                )]
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
}

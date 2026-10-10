import Foundation

@testable import RemoraCore

let account = UUID(uuidString: "00000000-0000-0000-0000-000000000001")!
let now = Date(timeIntervalSince1970: 1_800_000_000)

func makeItem(
    _ id: String,
    bundle: InboxBundle = .reviews,
    badges: [Badge] = [],
    date: Date = now,
    needsAction: Bool = false
) -> InboxItem {
    InboxItem(
        id: id, accountID: account, pluginID: "test", bundle: bundle,
        title: "Title \(id)", context: "repo #\(id)", badges: badges, date: date, needsAction: needsAction
    )
}

let approved = Badge(id: "approved", label: "Approved", tone: .accent, notify: .init(title: "Approved"))

import Foundation
import RemoraCore

/// The repositories you maintain, read for their checks and security alerts. Same rules on Windows (`plugin.rs`).
enum Maintained {
    /// At most: each repository costs a few requests a refresh.
    static let limit = 10

    /// `owner/name` (`group/subgroup/name` when `nested`, as on GitLab), separated by commas, spaces or lines; the
    /// same one twice counts once.
    static func repositories(_ value: String?, nested: Bool) -> [String] {
        var found: [String] = []
        let allowed = CharacterSet.alphanumerics.union(CharacterSet(charactersIn: "-_."))
        for raw in (value ?? "").split(whereSeparator: { $0 == "," || $0.isWhitespace }) {
            let name = raw.trimmingCharacters(in: CharacterSet(charactersIn: "/"))
            let parts = name.split(separator: "/", omittingEmptySubsequences: false)
            let valid =
                (parts.count == 2 || nested && parts.count > 2)
                && parts.allSatisfy { !$0.isEmpty && $0.unicodeScalars.allSatisfy(allowed.contains) }
            if valid && !found.contains(where: { $0.caseInsensitiveCompare(name) == .orderedSame }) {
                found.append(name)
            }
        }
        return Array(found.prefix(limit))
    }

    /// The default branch is red: to fix, said by the checks badge (and notified once).
    static func failing(
        accountID: UUID, pluginID: String, local: String, title: String, context: String, preview: String?, url: URL?,
        date: Date
    ) -> InboxItem {
        InboxItem(
            id: InboxItem.makeID(account: accountID, local: local),
            accountID: accountID,
            pluginID: pluginID,
            bundle: .fix,
            title: title,
            context: context,
            preview: preview,
            url: url,
            badges: [
                Badge(
                    id: .checksFailing, label: L("Checks failed"), symbol: "xmark.octagon.fill", tone: .negative,
                    notify: .init(title: L("Checks failed")))
            ],
            date: date,
            needsAction: true
        )
    }
}

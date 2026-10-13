import Foundation

/// `remora://` links (IDEA-06): what Shortcuts, Raycast, Alfred, a Stream Deck or a script may ask of Remora. Only
/// what the user then sees: open the inbox (on a tab), open the reminder panel filled in, refresh. Nothing that marks
/// an item done or snoozes it without the user. Same rule on Windows (`link.rs`).
public enum RemoraLink: Equatable, Sendable {
    /// `remora://open`, `remora://open?tab=waiting`: the inbox tab's id (`inProgress`, `myTurn`…).
    case open(tab: String?)
    /// `remora://remind?text=Call%20Erin&at=2026-10-14T09:00`: the reminder panel, filled in, for the user to save.
    case remind(text: String, at: Date?)
    /// `remora://refresh`.
    case refresh

    public static let scheme = "remora"
    /// A reminder's text from a link is cut there: a link can't fill the panel with a page of text.
    public static let textLimit = 200
    static let tabs = [
        "in-progress": "inProgress", "my-turn": "myTurn", "waiting": "waiting", "snoozed": "snoozed", "done": "done",
    ]

    /// What a `remora://` link asks, or nil for anything else (another scheme, an unknown action). A time already
    /// past is left for the user to pick.
    public init?(_ url: URL, now: Date = .now) {
        guard url.scheme?.lowercased() == Self.scheme else { return nil }
        // `remora://open?…`: the action is the host; `remora:open?…`, the path.
        let action = (url.host ?? url.path).trimmingCharacters(in: CharacterSet(charactersIn: "/")).lowercased()
        // `+` is a space in a query, as on Windows.
        let raw = URLComponents(url: url, resolvingAgainstBaseURL: false)?.percentEncodedQuery ?? ""
        let value = { (name: String) -> String? in
            raw.split(separator: "&").compactMap { pair -> (String, String)? in
                let parts = pair.split(separator: "=", maxSplits: 1)
                guard parts.count == 2 else { return nil }
                let decode = { (text: Substring) in
                    let spaced = text.replacingOccurrences(of: "+", with: " ")
                    return spaced.removingPercentEncoding ?? spaced
                }
                return (decode(parts[0]), decode(parts[1]))
            }.first { $0.0 == name }?.1
        }
        switch action {
        case "open":
            let tab = value("tab").flatMap { Self.tabs[$0.trimmingCharacters(in: .whitespaces).lowercased()] }
            self = .open(tab: tab)
        case "remind":
            let text = String(
                (value("text") ?? "").trimmingCharacters(in: .whitespacesAndNewlines).prefix(Self.textLimit))
            let at = value("at").flatMap(Self.time).flatMap { $0 > now ? $0 : nil }
            self = .remind(text: text, at: at)
        case "refresh": self = .refresh
        default: return nil
        }
    }

    /// "2026-10-14T09:00:00+02:00", or "2026-10-14T09:00" on this Mac's clock.
    static func time(_ raw: String) -> Date? {
        let raw = raw.trimmingCharacters(in: .whitespaces)
        if let date = Date(iso8601: raw) { return date }
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.timeZone = .current
        for format in ["yyyy-MM-dd'T'HH:mm", "yyyy-MM-dd'T'HH:mm:ss", "yyyy-MM-dd HH:mm"] {
            formatter.dateFormat = format
            if let date = formatter.date(from: raw) { return date }
        }
        return nil
    }
}

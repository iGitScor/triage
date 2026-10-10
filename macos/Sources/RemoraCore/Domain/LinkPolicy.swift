import Foundation

/// Which links from the tools Remora opens. Item links come from server fields, so a hostile or broken server could
/// send `file:`, `smb:` or a custom scheme: only web pages and the tools' own desktop apps are opened.
public enum LinkPolicy {
    /// Schemes of the desktop apps Remora hands links to (Slack, Linear).
    public static let appSchemes: Set<String> = ["slack", "linear"]

    /// An https page, or an http one on `httpHosts` (a self-hosted server the user connected over http).
    public static func isWebLink(_ url: URL, httpHosts: Set<String> = []) -> Bool {
        guard let host = url.host?.lowercased(), !host.isEmpty else { return false }
        switch url.scheme?.lowercased() {
        case "https": return true
        case "http": return httpHosts.contains(host)
        default: return false
        }
    }

    public static func isAppLink(_ url: URL) -> Bool {
        appSchemes.contains(url.scheme?.lowercased() ?? "")
    }

    /// The hosts an account reaches over http: its own host, when the user entered an `http://` address.
    public static func httpHosts(of account: Account?) -> Set<String> {
        guard let raw = account?.settings["host"]?.trimmingCharacters(in: .whitespaces).lowercased(),
              raw.hasPrefix("http://"), let host = URL(string: raw)?.host else { return [] }
        return [host]
    }
}

import Foundation

/// Ticket keys ("CE-1380") in free text. A prefix seen as a real key (a Linear or Jira item's context is its key) is
/// matched in any case, "ce-1380" in a Slack message too. Any other prefix must be written in capitals and not be a
/// version or an encoding ("UTF-8", "SHA-256", "Q4-2025"). A key starts a word: "vue-router-4" has none.
/// A CVE counts whole ("CVE-2024-3094"), never by its year.
public struct TicketKeys: Sendable {
    /// Upper-cased prefixes of real keys.
    public var known: Set<String>

    static let notTickets: Set<String> = [
        "UTF", "UCS", "SHA", "MD", "AES", "RSA", "TLS", "SSL", "HTTP", "ISO", "IEC", "RFC", "PEP", "ES", "ECMA",
        "COVID", "CWE", "GPT", "IPV", "TCP", "UDP", "Q1", "Q2", "Q3", "Q4", "H1", "H2", "FY", "WIN", "IOS", "ARM",
        "PY", "NODE",
    ]

    public init(known: Set<String> = []) {
        self.known = known
    }

    /// The prefixes of the items whose whole context is a key: Linear identifiers and Jira keys.
    public init(knownIn items: [InboxItem]) {
        known = Set(
            items.compactMap { item in
                guard let match = item.context.wholeMatch(of: #/([A-Z][A-Z0-9]+)-\d+/#) else { return nil }
                return String(match.output.1)
            })
    }

    public func keys(_ text: String) -> Set<String> {
        var keys: Set<String> = []
        for match in text.matches(of: #/(CVE-\d{4}-\d+)/#.ignoresCase())
        where Self.startsWord(match.range, in: text) && Self.endsWord(match.range, in: text) {
            keys.insert(match.output.1.uppercased())
        }
        for match in text.matches(of: #/([A-Za-z][A-Za-z0-9]+)-(\d+)/#) {
            guard Self.startsWord(match.range, in: text), Self.endsWord(match.range, in: text) else { continue }
            let prefix = String(match.output.1)
            let upper = prefix.uppercased()
            guard upper != "CVE" else { continue }
            if known.contains(upper) || (prefix == upper && !Self.notTickets.contains(upper)) {
                keys.insert(upper + "-" + match.output.2)
            }
        }
        return keys
    }

    /// Not inside a word nor a hyphenated name: "xCE-1", "vue-router-4" and "a_b-1" are not keys.
    private static func startsWord(_ range: Range<String.Index>, in text: String) -> Bool {
        guard range.lowerBound > text.startIndex else { return true }
        let before = text[text.index(before: range.lowerBound)]
        return !(before.isLetter || before.isNumber || before == "-" || before == "_")
    }

    /// "SHA-256abc" is not a key; "CE-1380-close-cases" and "CE-1380." are.
    private static func endsWord(_ range: Range<String.Index>, in text: String) -> Bool {
        guard range.upperBound < text.endIndex else { return true }
        let after = text[range.upperBound]
        return !(after.isLetter || after.isNumber || after == "_")
    }
}

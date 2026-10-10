import Foundation

/// Text from the tools as people read it: emoji codes as emoji, and no formatting marks (`*bold*`, `_italic_`,
/// `~strike~`, `` `code` ``, `**bold**`, a quote's `>`). Every plugin's titles and Slack's messages go through it, so
/// the inbox, the sorting rules and the assistant all read the same words. Same rules on Windows (`text.rs`).
public enum Readable {
    public static func text(_ text: String) -> String {
        var result = emoji(text)
        for (pattern, template) in marks {
            // Twice, for a mark inside another (`*_both_*`).
            for _ in 0..<2 {
                result = result.replacingOccurrences(of: pattern, with: template, options: .regularExpression)
            }
        }
        return result
    }

    /// `:tada:` → 🎉. Codes not in the table (a workspace's own emoji) stay as written; skin tones are dropped.
    public static func emoji(_ text: String) -> String {
        guard text.contains(":") else { return text }
        let text = text.replacingOccurrences(of: #":skin-tone-[2-6]:"#, with: "", options: .regularExpression)
        let pattern = try! NSRegularExpression(pattern: #":([a-z0-9_+\-]*[a-z][a-z0-9_+\-]*):"#)
        var result = ""
        var last = text.startIndex
        for match in pattern.matches(in: text, range: NSRange(text.startIndex..., in: text)) {
            guard let whole = Range(match.range, in: text), let name = Range(match.range(at: 1), in: text),
                  let symbol = Emoji.table[String(text[name])] else { continue }
            result += text[last..<whole.lowerBound] + symbol
            last = whole.upperBound
        }
        return result + text[last...]
    }

    /// A mark opens after the start or a non-word character and before a non-space, and closes before the end or a
    /// non-word character, on one line: `snake_case`, `2*3*4` and `* bullet` keep theirs.
    static let marks: [(String, String)] = [
        (#"```\n?"#, ""),
        (#"`([^`\n]+)`"#, "$1"),
        (#"(?<![\p{L}\p{N}])(\*\*|__|~~|[*_~])(?=\S)([^\n]*?\S)\1(?![\p{L}\p{N}])"#, "$2"),
        (#"(?m)^>[ \t]?"#, ""),
    ]
}

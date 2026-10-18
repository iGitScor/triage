import Foundation

/// What leaves the computer for an assistant, cleaned first: tokens and keys someone pasted into a title, e-mail
/// addresses and phone numbers become placeholders. Conservative on purpose: commit hashes, ticket numbers, dates and
/// versions stay, so the titles keep their meaning. The same patterns and cases on Windows (`redaction.rs`).
public enum Redaction {
    /// Each pattern, and what replaces a match (`$1`: the character kept before a phone number).
    static let rules: [(pattern: String, replacement: String)] = [
        // Private keys, whole.
        (#"-----BEGIN [A-Z ]*PRIVATE KEY-----[\s\S]*?(-----END [A-Z ]*PRIVATE KEY-----|$)"#, "[secret]"),
        // Tokens with a known prefix: GitHub, Slack, Anthropic, OpenAI, AWS, Google, Stripe, npm.
        (#"\b(gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,})"#, "[secret]"),
        (#"\bxox[abposr]-[A-Za-z0-9-]{10,}"#, "[secret]"),
        (#"\bsk-(ant-|proj-)?[A-Za-z0-9_-]{20,}"#, "[secret]"),
        (#"\b(AKIA|ASIA)[0-9A-Z]{16}\b"#, "[secret]"),
        (#"\bAIza[0-9A-Za-z_-]{35}"#, "[secret]"),
        (#"\b[rs]k_(live|test)_[0-9A-Za-z]{16,}"#, "[secret]"),
        (#"\bnpm_[A-Za-z0-9]{36}"#, "[secret]"),
        // A JSON web token: three base64url parts, the first two starting as JSON.
        (#"\beyJ[A-Za-z0-9_-]{8,}\.eyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}"#, "[secret]"),
        // Whatever follows "Bearer".
        (#"(?i)\bbearer\s+[A-Za-z0-9._~+/-]{16,}=*"#, "Bearer [secret]"),
        (#"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}"#, "[email]"),
        // Phone numbers: international (+33 6 12 34 56 78, +1 (555) 123-4567) or French national (06 12 34 56 78),
        // after neither a letter, a digit nor a "+", which is kept.
        (#"(^|[^\w+])\+\d{1,3}[ .-]?(\(\d{1,4}\)[ .-]?)?\d{1,4}([ .-]?\d{2,4}){2,4}\b"#, "$1[phone]"),
        (#"(^|[^\w+])0[1-9]([ .-]?\d{2}){4}\b"#, "$1[phone]"),
    ]

    private static let compiled: [(NSRegularExpression, String)] = rules.map {
        (try! NSRegularExpression(pattern: $0.pattern), $0.replacement)
    }

    public static func scrub(_ text: String) -> String {
        compiled.reduce(text) { text, rule in
            rule.0.stringByReplacingMatches(
                in: text, range: NSRange(text.startIndex..., in: text), withTemplate: rule.1)
        }
    }
}

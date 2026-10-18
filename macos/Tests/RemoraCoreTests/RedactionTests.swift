import Testing

@testable import RemoraCore

/// The same cases as Windows and Linux (`redaction.rs`).
struct RedactionTests {
    static let cases: [(String, String)] = [
        ("Token ghp_abcdefghijklmnopqrstuvwxyz0123456789 leaked", "Token [secret] leaked"),
        ("use github_pat_11ABCDEFG0123456789_abcdefghijklmnop", "use [secret]"),
        ("Slack xoxb-1234567890-abcdefghij in the logs", "Slack [secret] in the logs"),
        ("key sk-ant-api03-abcdefghijklmnopqrstuvwxyz", "key [secret]"),
        ("OPENAI sk-proj-ABCDEFGHIJKLMNOPQRSTUVWX", "OPENAI [secret]"),
        ("AWS AKIAIOSFODNN7EXAMPLE in config", "AWS [secret] in config"),
        ("jwt eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N", "jwt [secret]"),
        ("curl -H 'Authorization: Bearer abcdef0123456789abcdef'", "curl -H 'Authorization: Bearer [secret]'"),
        ("Ping erin.doe+work@acme.io about the quote", "Ping [email] about the quote"),
        ("Call me on +33 6 12 34 56 78 tonight", "Call me on [phone] tonight"),
        ("US line +1 (555) 123-4567.", "US line [phone]."),
        ("Rappelle-moi au 06 12 34 56 78", "Rappelle-moi au [phone]"),
        ("-----BEGIN RSA PRIVATE KEY-----\nMIIEow\n-----END RSA PRIVATE KEY----- end", "[secret] end"),
        ("Revert 3f9c2a1b7e4d5c6f8a9b0c1d2e3f4a5b6c7d8e9f", "Revert 3f9c2a1b7e4d5c6f8a9b0c1d2e3f4a5b6c7d8e9f"),
        ("ENG-1234 and #567 due 2026-10-18 at 14:30", "ENG-1234 and #567 due 2026-10-18 at 14:30"),
        ("Bump serde from 1.0.210 to 1.0.228", "Bump serde from 1.0.210 to 1.0.228"),
        ("Order 0612345678901 shipped", "Order 0612345678901 shipped"),
        ("sk-short is fine, so is ask-someone", "sk-short is fine, so is ask-someone"),
    ]

    @Test func secretsEmailsAndPhonesBecomePlaceholders() {
        for (text, cleaned) in Self.cases {
            #expect(Redaction.scrub(text) == cleaned, "\(text)")
        }
    }
}

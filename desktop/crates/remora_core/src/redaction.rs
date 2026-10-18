//! What leaves the computer for an assistant, cleaned first: tokens and keys someone pasted into a title, e-mail
//! addresses and phone numbers become placeholders. Conservative on purpose: commit hashes, ticket numbers, dates and
//! versions stay, so the titles keep their meaning. The same patterns and cases on macOS (`Redaction.swift`).
use regex::Regex;
use std::sync::LazyLock;

/// Each pattern, and what replaces a match (`${1}`: the character kept before a phone number).
const RULES: &[(&str, &str)] = &[
    // Private keys, whole.
    (r"-----BEGIN [A-Z ]*PRIVATE KEY-----[\s\S]*?(-----END [A-Z ]*PRIVATE KEY-----|$)", "[secret]"),
    // Tokens with a known prefix: GitHub, Slack, Anthropic, OpenAI, AWS, Google, Stripe, npm.
    (r"\b(gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,})", "[secret]"),
    (r"\bxox[abposr]-[A-Za-z0-9-]{10,}", "[secret]"),
    (r"\bsk-(ant-|proj-)?[A-Za-z0-9_-]{20,}", "[secret]"),
    (r"\b(AKIA|ASIA)[0-9A-Z]{16}\b", "[secret]"),
    (r"\bAIza[0-9A-Za-z_-]{35}", "[secret]"),
    (r"\b[rs]k_(live|test)_[0-9A-Za-z]{16,}", "[secret]"),
    (r"\bnpm_[A-Za-z0-9]{36}", "[secret]"),
    // A JSON web token: three base64url parts, the first two starting as JSON.
    (r"\beyJ[A-Za-z0-9_-]{8,}\.eyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}", "[secret]"),
    // Whatever follows "Bearer".
    (r"(?i)\bbearer\s+[A-Za-z0-9._~+/-]{16,}=*", "Bearer [secret]"),
    (r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}", "[email]"),
    // Phone numbers: international (+33 6 12 34 56 78, +1 (555) 123-4567) or French national (06 12 34 56 78), after
    // neither a letter, a digit nor a "+", which is kept.
    (r"(^|[^\w+])\+\d{1,3}[ .-]?(\(\d{1,4}\)[ .-]?)?\d{1,4}([ .-]?\d{2,4}){2,4}\b", "${1}[phone]"),
    (r"(^|[^\w+])0[1-9]([ .-]?\d{2}){4}\b", "${1}[phone]"),
];

static COMPILED: LazyLock<Vec<(Regex, &'static str)>> =
    LazyLock::new(|| RULES.iter().map(|(pattern, with)| (Regex::new(pattern).expect(pattern), *with)).collect());

pub fn redact(text: &str) -> String {
    COMPILED.iter().fold(text.to_string(), |text, (rule, with)| rule.replace_all(&text, *with).into_owned())
}

#[cfg(test)]
mod tests {
    use super::redact;

    /// The same cases as macOS (`RedactionTests`).
    #[test]
    fn secrets_emails_and_phones_become_placeholders() {
        for (text, cleaned) in CASES {
            assert_eq!(redact(text), *cleaned, "{text}");
        }
    }

    pub(crate) const CASES: &[(&str, &str)] = &[
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
        // What stays: commit hashes, ticket and PR numbers, dates, versions, times, plain words.
        ("Revert 3f9c2a1b7e4d5c6f8a9b0c1d2e3f4a5b6c7d8e9f", "Revert 3f9c2a1b7e4d5c6f8a9b0c1d2e3f4a5b6c7d8e9f"),
        ("ENG-1234 and #567 due 2026-10-18 at 14:30", "ENG-1234 and #567 due 2026-10-18 at 14:30"),
        ("Bump serde from 1.0.210 to 1.0.228", "Bump serde from 1.0.210 to 1.0.228"),
        ("Order 0612345678901 shipped", "Order 0612345678901 shipped"),
        ("sk-short is fine, so is ask-someone", "sk-short is fine, so is ask-someone"),
    ];
}

//! Why a source failed, so the inbox can say it calmly and offer the fix that applies. Same rules as the
//! macOS app (`SourceFailure.swift`).

use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FailureKind {
    /// No network at all: every source failed to connect in the same refresh.
    Offline,
    /// This host can't be reached (a VPN, DNS, the server down) while others answer.
    Unreachable,
    /// The token was rejected: reconnect the account.
    Auth,
    RateLimited,
    Other,
}

/// A source's last failure: its kind, and what to tell the user (English, translated by the interface).
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Failure {
    pub kind: FailureKind,
    pub message: String,
}

pub const OFFLINE: &str = "Offline: Remora tries again when the connection is back.";
pub const UNREACHABLE: &str = "Can’t reach the server. Check the address, or your VPN if it needs one.";

impl Failure {
    pub fn new(kind: FailureKind, message: impl Into<String>) -> Self {
        let message = match kind {
            FailureKind::Offline => OFFLINE.to_string(),
            FailureKind::Unreachable => UNREACHABLE.to_string(),
            _ => message.into(),
        };
        Failure { kind, message }
    }
}

/// What the inbox footer says about the sources, most important first.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "state")]
pub enum SourcesHealth {
    Fine,
    Offline,
    /// Accounts whose token to reconnect.
    Reconnect {
        accounts: Vec<String>,
    },
    Failing {
        count: usize,
    },
}

impl SourcesHealth {
    pub fn of<'a>(failures: impl IntoIterator<Item = (&'a String, &'a Failure)>) -> Self {
        let failures: Vec<_> = failures.into_iter().collect();
        if !failures.is_empty() && failures.iter().all(|(_, f)| f.kind == FailureKind::Offline) {
            return SourcesHealth::Offline;
        }
        let mut auth: Vec<String> =
            failures.iter().filter(|(_, f)| f.kind == FailureKind::Auth).map(|(id, _)| (*id).clone()).collect();
        if !auth.is_empty() {
            auth.sort();
            return SourcesHealth::Reconnect { accounts: auth };
        }
        // A rate limit is waited out, and the account says until when: not a failure.
        let count =
            failures.iter().filter(|(_, f)| !matches!(f.kind, FailureKind::RateLimited | FailureKind::Offline)).count();
        if count > 0 {
            SourcesHealth::Failing { count }
        } else {
            SourcesHealth::Fine
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_footer_says_the_most_useful_thing() {
        let f = |kind| Failure::new(kind, "x");
        let (a, b, c) = ("a".to_string(), "b".to_string(), "c".to_string());
        assert_eq!(SourcesHealth::of([]), SourcesHealth::Fine);
        assert_eq!(
            SourcesHealth::of([(&a, &f(FailureKind::Offline)), (&b, &f(FailureKind::Offline))]),
            SourcesHealth::Offline
        );
        assert_eq!(
            SourcesHealth::of([(&a, &f(FailureKind::Auth)), (&b, &f(FailureKind::Other))]),
            SourcesHealth::Reconnect { accounts: vec!["a".into()] }
        );
        assert_eq!(
            SourcesHealth::of([(&a, &f(FailureKind::RateLimited))]),
            SourcesHealth::Fine,
            "a rate limit is only waited out"
        );
        assert_eq!(
            SourcesHealth::of([
                (&a, &f(FailureKind::RateLimited)),
                (&b, &f(FailureKind::Other)),
                (&c, &f(FailureKind::Unreachable))
            ]),
            SourcesHealth::Failing { count: 2 }
        );
        assert_eq!(f(FailureKind::Offline).message, OFFLINE, "network failures say what they mean");
    }
}

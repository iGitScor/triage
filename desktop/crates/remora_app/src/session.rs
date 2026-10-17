use remora_plugins::{AccountCache, ETagStore, RENEWED_SECRETS};
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

/// What one account keeps between refreshes, in memory: what its plugin remembers (who the token belongs
/// to) and the answers its conditional requests can reuse. A new one starts whenever the account's settings or token
/// change, so nothing learned with an old token or host is reused.
pub struct AccountSession {
    fingerprint: u64,
    pub cache: Arc<AccountCache>,
    pub etags: Arc<ETagStore>,
}

impl AccountSession {
    /// The session for these settings and secrets: `current` if they haven't changed, else a new one.
    pub fn current(
        sessions: &mut HashMap<String, AccountSession>,
        account_id: &str,
        settings: &HashMap<String, String>,
        secrets: &HashMap<String, String>,
    ) -> (Arc<AccountCache>, Arc<ETagStore>) {
        let fingerprint = fingerprint(settings, secrets);
        let session = sessions
            .entry(account_id.to_string())
            .and_modify(|s| {
                if s.fingerprint != fingerprint {
                    *s = AccountSession::new(fingerprint);
                }
            })
            .or_insert_with(|| AccountSession::new(fingerprint));
        (session.cache.clone(), session.etags.clone())
    }

    /// Secrets the account's plugin renewed since last asked, by account.
    pub fn take_renewed(sessions: &HashMap<String, AccountSession>) -> Vec<(String, HashMap<String, String>)> {
        sessions
            .iter()
            .map(|(id, session)| (id.clone(), session.cache.take_renewed()))
            .filter(|(_, renewed)| !renewed.is_empty())
            .collect()
    }

    fn new(fingerprint: u64) -> Self {
        AccountSession { fingerprint, cache: Default::default(), etags: Default::default() }
    }
}

/// Settings and secrets, hashed in memory to notice a change: never stored. A secret the plugin renews on its own
/// (`RENEWED_SECRETS`) is left out, or every renewal would start the account over.
fn fingerprint(settings: &HashMap<String, String>, secrets: &HashMap<String, String>) -> u64 {
    let mut values: Vec<_> =
        settings.iter().chain(secrets.iter().filter(|(k, _)| !RENEWED_SECRETS.contains(&k.as_str()))).collect();
    values.sort();
    let mut hasher = DefaultHasher::new();
    values.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_token_starts_a_new_session() {
        let mut sessions = HashMap::new();
        let settings = HashMap::from([("host".to_string(), "gitlab.acme.io".to_string())]);
        let token = |t: &str| HashMap::from([("token".to_string(), t.to_string())]);
        let (first, _) = AccountSession::current(&mut sessions, "a", &settings, &token("old"));
        let (same, _) = AccountSession::current(&mut sessions, "a", &settings, &token("old"));
        let (renewed, _) = AccountSession::current(&mut sessions, "a", &settings, &token("new"));
        assert!(Arc::ptr_eq(&first, &same) && !Arc::ptr_eq(&first, &renewed));
        assert!(Arc::ptr_eq(&first, &same));
        assert!(!Arc::ptr_eq(&first, &renewed));
        assert_eq!(sessions.len(), 1);
    }

    /// A new host starts a new session too, with nothing learned from the old one.
    #[test]
    fn a_new_host_starts_a_new_session() {
        let mut sessions = HashMap::new();
        let host = |h: &str| HashMap::from([("host".to_string(), h.to_string())]);
        let token = HashMap::from([("token".to_string(), "t".to_string())]);
        let (first, _) = AccountSession::current(&mut sessions, "a", &host("gitlab.acme.io"), &token);
        let (moved, _) = AccountSession::current(&mut sessions, "a", &host("gitlab.example.org"), &token);
        assert!(!Arc::ptr_eq(&first, &moved));
    }

    /// A refresh token the plugin rotated keeps the session (its cached access token, its ETags); a new sign-in
    /// starts a new one.
    #[test]
    fn a_renewed_refresh_token_keeps_the_session() {
        let mut sessions = HashMap::new();
        let settings = HashMap::new();
        let secrets = |refresh: &str, sign_in: &str| {
            HashMap::from([
                ("refreshToken".to_string(), refresh.to_string()),
                ("signIn".to_string(), sign_in.to_string()),
            ])
        };
        let (first, _) = AccountSession::current(&mut sessions, "a", &settings, &secrets("r1", "s1"));
        first.renew("refreshToken", "r2".into());
        let (after, _) = AccountSession::current(&mut sessions, "a", &settings, &secrets("r2", "s1"));
        assert!(Arc::ptr_eq(&first, &after));
        assert_eq!(
            AccountSession::take_renewed(&sessions),
            [("a".to_string(), HashMap::from([("refreshToken".to_string(), "r2".to_string())]))]
        );
        assert!(AccountSession::take_renewed(&sessions).is_empty(), "taken once");
        let (signed_in_again, _) = AccountSession::current(&mut sessions, "a", &settings, &secrets("r3", "s2"));
        assert!(!Arc::ptr_eq(&first, &signed_in_again));
    }
}

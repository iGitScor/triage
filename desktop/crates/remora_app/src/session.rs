use remora_plugins::{AccountCache, ETagStore};
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

/// What one account keeps between refreshes, in memory (PERF-09): what its plugin remembers (who the token belongs
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

    fn new(fingerprint: u64) -> Self {
        AccountSession { fingerprint, cache: Default::default(), etags: Default::default() }
    }
}

/// Settings and secrets, hashed in memory to notice a change: never stored.
fn fingerprint(settings: &HashMap<String, String>, secrets: &HashMap<String, String>) -> u64 {
    let mut values: Vec<_> = settings.iter().chain(secrets.iter()).collect();
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
        assert!(Arc::ptr_eq(&first, &same));
        assert!(!Arc::ptr_eq(&first, &renewed));
        assert_eq!(sessions.len(), 1);
    }
}

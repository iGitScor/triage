use crate::PluginError;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use remora_core::{Brief, InboxItem, SnoozedItem, TriageSuggestion};
use std::any::Any;
use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The values a user entered for one account of a plugin, secrets included, and what the plugin may keep between
/// refreshes of that account.
pub struct PluginConfig {
    pub account_id: String,
    pub values: HashMap<String, String>,
    pub cache: Arc<AccountCache>,
}

/// Who the token belongs to changes rarely; an hour keeps a renamed user from going unnoticed for long.
pub const IDENTITY_LIFETIME: Duration = Duration::from_secs(3600);

/// What a plugin may keep between refreshes of one account, such as who the token belongs to: asking for it every
/// few minutes costs a request and counts against the tool's rate limit (PERF-09). In memory only, and dropped by the
/// app when the account is reconnected, changed, disconnected or erased. A new one keeps nothing yet, so a plugin
/// built outside a refresh (tests, connecting) asks every time.
#[derive(Default)]
pub struct AccountCache {
    entries: Mutex<HashMap<String, Kept>>,
}

/// A value and when it was loaded.
type Kept = (Instant, Arc<dyn Any + Send + Sync>);

impl AccountCache {
    /// The value kept under `key` if younger than `lifetime`, else what `load` returns, kept for next time. A failed
    /// `load` keeps nothing.
    pub async fn value<T, F, Fut>(&self, key: &str, lifetime: Duration, load: F) -> Result<T, PluginError>
    where
        T: Clone + Send + Sync + 'static,
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T, PluginError>>,
    {
        let kept = self.entries.lock().unwrap().get(key).cloned();
        if let Some((at, value)) = kept {
            if let (true, Some(value)) = (at.elapsed() < lifetime, value.downcast_ref::<T>()) {
                return Ok(value.clone());
            }
        }
        let value = load().await?;
        self.entries.lock().unwrap().insert(key.to_string(), (Instant::now(), Arc::new(value.clone())));
        Ok(value)
    }
}

impl PluginConfig {
    pub fn get(&self, key: &str) -> String {
        self.values.get(key).map(|v| v.trim().to_string()).unwrap_or_default()
    }

    pub fn required(&self, key: &str) -> Result<String, PluginError> {
        let value = self.get(key);
        if value.is_empty() {
            Err(PluginError::MissingField(key.into()))
        } else {
            Ok(value)
        }
    }

    /// "gitlab.acme.io/" → "https://gitlab.acme.io". Tokens travel with every request, so only https is accepted
    /// (plain http only to this computer, for a local test server).
    pub fn url(&self, key: &str) -> Result<String, PluginError> {
        let mut raw = self.required(key)?;
        while raw.ends_with('/') {
            raw.pop();
        }
        if !raw.contains("://") {
            raw = format!("https://{raw}");
        }
        let url = reqwest::Url::parse(&raw).map_err(|_| PluginError::InvalidField(key.into()))?;
        let Some(host) = url.host_str().map(str::to_lowercase).filter(|h| !h.is_empty()) else {
            return Err(PluginError::InvalidField(key.into()));
        };
        match url.scheme() {
            "https" => Ok(raw),
            "http" if ["localhost", "127.0.0.1", "[::1]"].contains(&host.as_str()) => Ok(raw),
            _ => Err(PluginError::InsecureField(key.into())),
        }
    }
}

/// What a source returns on each refresh.
pub struct SourceSnapshot {
    pub identity: String,
    pub items: Vec<InboxItem>,
    /// What the user should know about this fetch (the interface translates it by code): a list cut at its limit, a
    /// permission missing.
    pub remarks: Vec<remora_core::Message>,
}

/// Lists stop at 50 items each: past that, the tool has more than the inbox shows. Same text on macOS.
pub const TRUNCATED: &str = "%@ has more than Remora shows: only the latest 50 of each list are listed.";

pub fn truncated(tool: &str) -> remora_core::Message {
    remora_core::Message::new("truncated", vec![tool.into()], TRUNCATED.replace("%@", tool))
}

#[async_trait]
pub trait SourcePlugin: Send + Sync {
    async fn fetch(&self) -> Result<SourceSnapshot, PluginError>;
}

/// An AI integration that helps triage the inbox (Claude Code or the Claude API). It only ever gets the items
/// `AssistantPolicy::items` lets through.
#[async_trait]
pub trait AssistantPlugin: Send + Sync {
    /// The whole inbox: what to handle first.
    async fn brief(&self, items: &[InboxItem], now: DateTime<Utc>) -> Result<Brief, PluginError>;
    /// One bundle in two short sentences, with a lighter model.
    async fn digest(&self, items: &[InboxItem], topic: &str, now: DateTime<Utc>) -> Result<String, PluginError>;
    /// Proposes what to do with each snoozed item.
    async fn triage(&self, items: &[SnoozedItem], now: DateTime<Utc>) -> Result<Vec<TriageSuggestion>, PluginError>;
}

#[cfg(test)]
mod tests {
    use crate::stub::config;
    use crate::PluginError;

    fn host(value: &str) -> Result<String, PluginError> {
        config(&[("host", value)]).url("host")
    }

    #[tokio::test]
    async fn the_cache_keeps_a_value_for_its_lifetime_and_never_a_failure() {
        use super::AccountCache;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::time::Duration;
        let cache = AccountCache::default();
        let loads = AtomicUsize::new(0);
        let load = || async {
            loads.fetch_add(1, Ordering::SeqCst);
            Ok("alice".to_string())
        };
        assert_eq!(cache.value("user", Duration::from_secs(60), load).await.unwrap(), "alice");
        assert_eq!(cache.value("user", Duration::from_secs(60), load).await.unwrap(), "alice");
        assert_eq!(loads.load(Ordering::SeqCst), 1);
        cache.value("user", Duration::ZERO, load).await.unwrap();
        assert_eq!(loads.load(Ordering::SeqCst), 2, "an expired value is asked again");

        let refused =
            cache.value("other", Duration::from_secs(60), || async { Err::<String, _>(PluginError::Unauthorized) });
        assert_eq!(refused.await, Err(PluginError::Unauthorized));
        assert_eq!(cache.value("other", Duration::from_secs(60), load).await.unwrap(), "alice");
    }

    #[test]
    fn hosts_default_to_https() {
        assert_eq!(host("gitlab.acme.io/").unwrap(), "https://gitlab.acme.io");
        assert_eq!(host("https://github.acme.io//").unwrap(), "https://github.acme.io");
    }

    /// The token goes with every request, so a plain-http host is refused.
    #[test]
    fn plain_http_is_refused_except_on_this_computer() {
        assert_eq!(host("http://gitlab.lan"), Err(PluginError::InsecureField("host".into())));
        assert_eq!(host("ftp://gitlab.lan"), Err(PluginError::InsecureField("host".into())));
        assert_eq!(host("http://localhost:8929").unwrap(), "http://localhost:8929");
        assert_eq!(host("http://127.0.0.1").unwrap(), "http://127.0.0.1");
        assert_eq!(host("https://gitlab acme.io"), Err(PluginError::InvalidField("host".into())));
    }
}

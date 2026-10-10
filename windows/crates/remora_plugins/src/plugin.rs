use crate::PluginError;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use remora_core::{Brief, InboxItem, SnoozedItem, TriageSuggestion};
use std::collections::HashMap;

/// The values a user entered for one account of a plugin, secrets included.
pub struct PluginConfig {
    pub account_id: String,
    pub values: HashMap<String, String>,
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
    /// What the user should know about this fetch, in English (the interface translates it): a list cut at its
    /// limit, a permission missing.
    pub remarks: Vec<String>,
}

/// Lists stop at 50 items each: past that, the tool has more than the inbox shows. Same text on macOS.
pub const TRUNCATED: &str = "%@ has more than Remora shows: only the latest 50 of each list are listed.";

pub fn truncated(tool: &str) -> String {
    TRUNCATED.replace("%@", tool)
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

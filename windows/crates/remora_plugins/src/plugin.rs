use crate::PluginError;
use async_trait::async_trait;
use remora_core::InboxItem;
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
        if value.is_empty() { Err(PluginError::MissingField(key.into())) } else { Ok(value) }
    }

    /// "gitlab.acme.io/" → "https://gitlab.acme.io".
    pub fn url(&self, key: &str) -> Result<String, PluginError> {
        let mut raw = self.required(key)?;
        while raw.ends_with('/') {
            raw.pop();
        }
        if !raw.contains("://") {
            raw = format!("https://{raw}");
        }
        match reqwest::Url::parse(&raw) {
            Ok(url) if url.host_str().is_some() => Ok(raw),
            _ => Err(PluginError::InvalidField(key.into())),
        }
    }
}

/// What a source returns on each refresh.
pub struct SourceSnapshot {
    pub identity: String,
    pub items: Vec<InboxItem>,
}

#[async_trait]
pub trait SourcePlugin: Send + Sync {
    async fn fetch(&self) -> Result<SourceSnapshot, PluginError>;
}

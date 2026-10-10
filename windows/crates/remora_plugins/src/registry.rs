use crate::{github, gitlab, linear, notion, slack, HttpClient, PluginConfig, PluginError, SourcePlugin};
use remora_core::{Account, PluginManifest};
use std::collections::HashMap;
use std::sync::Arc;

/// Every source the Windows app knows about. Adding an integration means adding it here.
pub fn manifests() -> Vec<PluginManifest> {
    vec![github::manifest(), gitlab::manifest(), slack::manifest(), linear::manifest(), notion::manifest()]
}

pub fn manifest(id: &str) -> Option<PluginManifest> {
    manifests().into_iter().find(|m| m.id == id)
}

/// The hosts an account may reach: the plugin's declared ones plus the account's own host (self-hosted).
pub fn allowed_hosts(account: &Account, manifest: &PluginManifest) -> Vec<String> {
    let mut hosts = manifest.egress.hosts.clone();
    if let Some(host) = account.settings.get("host") {
        let raw = if host.contains("://") { host.clone() } else { format!("https://{host}") };
        if let Some(own) = reqwest::Url::parse(&raw).ok().and_then(|u| u.host_str().map(str::to_string)) {
            hosts.push(own);
        }
    }
    hosts
}

/// Builds a plugin. `http` must already be the guarded client for this account.
pub fn make(account: &Account, secrets: &HashMap<String, String>, http: Arc<dyn HttpClient>) -> Result<Box<dyn SourcePlugin>, PluginError> {
    let mut values = account.settings.clone();
    values.extend(secrets.clone());
    let config = PluginConfig { account_id: account.id.clone(), values };
    Ok(match account.plugin_id.as_str() {
        "github" => Box::new(github::GitHubPlugin::new(&config, http)?),
        "gitlab" => Box::new(gitlab::GitLabPlugin::new(&config, http)?),
        "slack" => Box::new(slack::SlackPlugin::new(&config, http)?),
        "linear" => Box::new(linear::LinearPlugin::new(&config, http)?),
        "notion" => Box::new(notion::NotionPlugin::new(&config, http)?),
        other => return Err(PluginError::UnknownPlugin(other.into())),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_every_plugin_declares_its_destinations() {
        let manifests = manifests();
        let mut ids: Vec<&str> = manifests.iter().map(|m| m.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), manifests.len());
        for manifest in &manifests {
            assert!(!manifest.egress.description.is_empty(), "{}", manifest.id);
            assert!(!manifest.egress.hosts.is_empty() || manifest.id == "gitlab", "{} must declare hosts", manifest.id);
        }
    }

    #[test]
    fn self_hosted_accounts_add_their_own_host() {
        let account = Account {
            id: "a".into(), plugin_id: "gitlab".into(), name: None, identity: None,
            settings: HashMap::from([("host".to_string(), "gitlab.acme.io".to_string())]),
        };
        assert!(allowed_hosts(&account, &gitlab::manifest()).contains(&"gitlab.acme.io".to_string()));
    }
}

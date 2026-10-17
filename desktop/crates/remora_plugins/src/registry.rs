use crate::{
    azuredevops, bitbucket, github, gitlab, jira, linear, microsoft, notion, slack, AccountCache, HttpClient,
    PluginConfig, PluginError, SourcePlugin,
};
use remora_core::{Account, PluginManifest};
use std::collections::HashMap;
use std::sync::Arc;

/// Every source the Windows app knows about. Adding an integration means adding it here.
pub fn manifests() -> Vec<PluginManifest> {
    vec![
        github::manifest(),
        gitlab::manifest(),
        bitbucket::manifest(),
        azuredevops::manifest(),
        slack::manifest(),
        linear::manifest(),
        jira::manifest(),
        notion::manifest(),
        microsoft::manifest(),
    ]
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

/// Builds a plugin. `http` must already be the guarded client for this account; `cache` is what it kept from the last
/// refresh (a new one for a plugin that starts from nothing).
pub fn make(
    account: &Account,
    secrets: &HashMap<String, String>,
    http: Arc<dyn HttpClient>,
    cache: Arc<AccountCache>,
) -> Result<Box<dyn SourcePlugin>, PluginError> {
    let mut values = account.settings.clone();
    values.extend(secrets.clone());
    let config = PluginConfig { account_id: account.id.clone(), values, cache };
    Ok(match account.plugin_id.as_str() {
        "github" => Box::new(github::GitHubPlugin::new(&config, http)?),
        "gitlab" => Box::new(gitlab::GitLabPlugin::new(&config, http)?),
        "bitbucket" => Box::new(bitbucket::BitbucketPlugin::new(&config, http)?),
        "azuredevops" => Box::new(azuredevops::AzureDevOpsPlugin::new(&config, http)?),
        "slack" => Box::new(slack::SlackPlugin::new(&config, http)?),
        "linear" => Box::new(linear::LinearPlugin::new(&config, http)?),
        "jira" => Box::new(jira::JiraPlugin::new(&config, http)?),
        "notion" => Box::new(notion::NotionPlugin::new(&config, http)?),
        "microsoft" => Box::new(microsoft::MicrosoftPlugin::new(&config, http)?),
        other => return Err(PluginError::UnknownPlugin(other.into())),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::claude;

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

    /// Each plugin's fetch for the egress test below. Claude Code makes no request: it runs a program on this
    /// computer.
    fn egress_fixture(id: &str) -> Option<crate::stub::EgressFixture> {
        Some(match id {
            "github" => github::tests::egress_fixture(),
            "gitlab" => gitlab::tests::egress_fixture(),
            "bitbucket" => bitbucket::tests::egress_fixture(),
            "azuredevops" => azuredevops::tests::egress_fixture(),
            "slack" => slack::tests::egress_fixture(),
            "linear" => linear::tests::egress_fixture(),
            "jira" => jira::tests::egress_fixture(),
            "notion" => notion::tests::egress_fixture(),
            "microsoft" => microsoft::tests::egress_fixture(),
            claude::api::ID => claude::api::tests::egress_fixture(),
            crate::openai::ID => crate::openai::tests::egress_fixture(),
            _ => return None,
        })
    }

    /// Every source and assistant fetches its fixture through the guard, with the hosts the app allows it:
    /// the fetch succeeds, every host it asked is declared, and its item ids follow `InboxItem::make_id`.
    /// A plugin added without a fixture fails. Same on macOS.
    #[tokio::test]
    async fn plugins_use_only_their_declared_hosts() {
        for manifest in manifests().into_iter().chain(claude::assistant_manifests()) {
            let id = manifest.id.as_str();
            if id == claude::code::ID {
                assert!(manifest.egress.hosts.is_empty(), "Claude Code reaches no host itself");
                continue;
            }
            let fixture = egress_fixture(id).unwrap_or_else(|| panic!("{id} has no fixture: add one"));
            let account = Account {
                id: "acc".into(),
                plugin_id: id.into(),
                name: None,
                identity: None,
                settings: fixture.settings.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
            };
            let allowed = allowed_hosts(&account, &manifest);
            let guarded = Arc::new(crate::GuardedHttpClient::new(fixture.http, allowed.clone()));
            let http = Arc::new(crate::stub::HostRecording::new(guarded));
            if claude::is_assistant(id) {
                let assistant = claude::make_assistant(&account, &HashMap::new(), http.clone(), "en").unwrap();
                let brief = assistant.brief(&[claude::testing::item("a")], claude::testing::now()).await;
                assert!(!brief.unwrap_or_else(|e| panic!("{id}: {e:?}")).summary.is_empty(), "{id}");
            } else {
                let plugin = make(&account, &HashMap::new(), http.clone(), Default::default()).unwrap();
                let snapshot = plugin.fetch().await.unwrap_or_else(|e| panic!("{id}: {e:?}"));
                // A list cut at 50 may be said; a part that couldn't be read may not.
                let failures: Vec<_> = snapshot.remarks.iter().filter(|r| r.code != "truncated").collect();
                assert!(!snapshot.items.is_empty() && failures.is_empty(), "{id}: {failures:?}");
                // Ids start with the account's, so two accounts never share an item, and are unique.
                for item in &snapshot.items {
                    assert!(
                        item.account_id == account.id && item.local_id().is_some_and(|l| !l.is_empty()),
                        "{id}: {}",
                        item.id
                    );
                }
                let ids: std::collections::HashSet<_> = snapshot.items.iter().map(|i| i.id.as_str()).collect();
                assert_eq!(ids.len(), snapshot.items.len(), "{id} repeats an item id");
            }
            let hosts = http.hosts();
            assert!(!hosts.is_empty(), "{id}");
            for host in hosts {
                assert!(remora_core::host_matches(&host, &allowed), "{id} asked {host}, which it doesn't declare");
            }
        }
    }

    #[test]
    fn self_hosted_accounts_add_their_own_host() {
        let account = Account {
            id: "a".into(),
            plugin_id: "gitlab".into(),
            name: None,
            identity: None,
            settings: HashMap::from([("host".to_string(), "gitlab.acme.io".to_string())]),
        };
        assert!(allowed_hosts(&account, &gitlab::manifest()).contains(&"gitlab.acme.io".to_string()));
    }
}

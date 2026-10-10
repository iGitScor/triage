//! The Claude assistant (brief, bundle summaries, triage), a port of the macOS `RemoraPlugins/Claude`: through the
//! Claude Code signed in on this computer (`code`), or an Anthropic API key (`api`). Both send inbox content to an
//! external AI, so both declare `external_ai` and stay off until the user or the organization allows it.

pub mod api;
pub mod code;
pub mod prompt;
pub mod runner;

#[cfg(test)]
pub(crate) mod testing;

pub use api::ClaudePlugin;
pub use code::ClaudeCodePlugin;
pub use runner::{CommandError, CommandRunner, ProcessCommandRunner};

use crate::{AssistantPlugin, HttpClient, PluginConfig, PluginError};
use remora_core::{Account, PluginManifest};
use std::collections::HashMap;
use std::sync::Arc;

/// The assistants, Claude Code first, as in the macOS app.
pub fn assistant_manifests() -> Vec<PluginManifest> {
    vec![code::manifest(), api::manifest()]
}

pub fn is_assistant(id: &str) -> bool {
    assistant_manifests().iter().any(|m| m.id == id)
}

/// Builds an assistant. `http` must already be the guarded client for this account (Claude Code doesn't use it: it
/// runs a local program). `language` is the interface's ("fr", "en"), for the language of the answers.
pub fn make_assistant(
    account: &Account,
    secrets: &HashMap<String, String>,
    http: Arc<dyn HttpClient>,
    language: &str,
) -> Result<Box<dyn AssistantPlugin>, PluginError> {
    let mut values = account.settings.clone();
    values.extend(secrets.clone());
    let config = PluginConfig { account_id: account.id.clone(), values };
    Ok(match account.plugin_id.as_str() {
        code::ID => Box::new(ClaudeCodePlugin::new(&config, language)),
        api::ID => Box::new(ClaudePlugin::new(&config, http, language)?),
        other => return Err(PluginError::UnknownPlugin(other.into())),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stub::StubHttp;

    fn account(plugin_id: &str) -> Account {
        Account { id: "a".into(), plugin_id: plugin_id.into(), name: None, identity: None, settings: HashMap::new() }
    }

    #[test]
    fn both_assistants_declare_where_their_data_goes() {
        let manifests = assistant_manifests();
        assert_eq!(manifests.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), ["claude-code", "claude"]);
        for manifest in &manifests {
            assert!(manifest.egress.external_ai, "{}", manifest.id);
            assert!(!manifest.egress.description.is_empty());
            assert!(
                !manifest.egress.hosts.is_empty() || manifest.id == "claude-code",
                "{} must declare hosts",
                manifest.id
            );
            assert!(!remora_core::CompliancePolicy::default().allows(manifest), "external AI is off by default");
        }
        assert!(is_assistant("claude-code") && !is_assistant("github"));
    }

    #[test]
    fn makes_each_assistant_with_its_secrets() {
        let http: Arc<dyn HttpClient> = Arc::new(StubHttp::paths(&[]));
        let secrets = HashMap::from([("token".to_string(), "k".to_string())]);
        assert!(make_assistant(&account("claude"), &secrets, http.clone(), "en").is_ok());
        assert_eq!(
            make_assistant(&account("claude"), &HashMap::new(), http.clone(), "en").err(),
            Some(PluginError::MissingField("token".into()))
        );
        assert!(make_assistant(&account("claude-code"), &HashMap::new(), http.clone(), "fr").is_ok());
        assert_eq!(
            make_assistant(&account("github"), &HashMap::new(), http, "en").err(),
            Some(PluginError::UnknownPlugin("github".into()))
        );
    }
}

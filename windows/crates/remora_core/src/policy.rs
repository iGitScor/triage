use crate::PluginManifest;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// What may leave the computer. Set by the user, or locked by the organization (Group Policy).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompliancePolicy {
    /// Plugins that may be connected. None means every plugin.
    pub allowed_plugins: Option<HashSet<String>>,
    /// Plugins that send inbox content to an external AI need this. Off by default.
    pub allow_external_ai: bool,
    /// Avatars, only from the hosts of allowed tools.
    pub allow_remote_images: bool,
}

impl Default for CompliancePolicy {
    fn default() -> Self {
        CompliancePolicy { allowed_plugins: None, allow_external_ai: false, allow_remote_images: true }
    }
}

impl CompliancePolicy {
    pub fn allows(&self, manifest: &PluginManifest) -> bool {
        self.refusal(manifest).is_none()
    }

    /// Why a plugin is blocked (English, translated by the UI).
    pub fn refusal(&self, manifest: &PluginManifest) -> Option<&'static str> {
        if let Some(allowed) = &self.allowed_plugins {
            if !allowed.contains(&manifest.id) {
                return Some("Not allowed by your privacy policy.");
            }
        }
        if manifest.egress.external_ai && !self.allow_external_ai {
            return Some("External AI is turned off in Privacy settings.");
        }
        None
    }
}

/// True when `host` is one of `allowed` or a subdomain of one (never a look-alike).
pub fn host_matches(host: &str, allowed: &[String]) -> bool {
    let host = host.to_lowercase();
    !host.is_empty() && allowed.iter().any(|a| {
        let a = a.to_lowercase();
        host == a || host.ends_with(&format!(".{a}"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Egress;

    fn manifest(id: &str, external_ai: bool) -> PluginManifest {
        PluginManifest {
            id: id.into(), name: id.into(), summary: String::new(), fields: vec![], setup_steps: vec![],
            setup_label: String::new(), setup_url: None, logo: None,
            egress: Egress { hosts: vec![], description: "d".into(), external_ai },
        }
    }

    #[test]
    fn external_ai_is_off_by_default() {
        let policy = CompliancePolicy::default();
        assert!(!policy.allows(&manifest("claude", true)));
        assert!(policy.allows(&manifest("github", false)));
        assert!(CompliancePolicy { allow_external_ai: true, ..Default::default() }.allows(&manifest("claude", true)));
    }

    #[test]
    fn allow_list_restricts_sources() {
        let policy = CompliancePolicy { allowed_plugins: Some(HashSet::from(["github".to_string()])), ..Default::default() };
        assert!(policy.allows(&manifest("github", false)));
        assert_eq!(policy.refusal(&manifest("slack", false)), Some("Not allowed by your privacy policy."));
    }

    #[test]
    fn hosts_match_exactly_or_as_subdomains_never_look_alikes() {
        let allowed = vec!["api.linear.app".to_string(), "gitlab.acme.io".to_string()];
        assert!(host_matches("api.linear.app", &allowed));
        assert!(host_matches("eu.gitlab.acme.io", &allowed));
        assert!(!host_matches("api.linear.app.evil.com", &allowed));
        assert!(!host_matches("evil.example.com", &allowed));
        assert!(!host_matches("", &allowed));
    }
}

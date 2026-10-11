use crate::PluginManifest;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::net::IpAddr;

/// The assistant whose server the user (or the organization) chooses: the OpenAI-compatible one.
pub const AI_SERVER_PLUGIN: &str = "openai";
/// Its server when none is set.
pub const DEFAULT_AI_SERVER: &str = "https://api.openai.com/v1";

pub const EXTERNAL_AI_OFF: &str = "External AI is turned off in Privacy settings.";
pub const SERVER_NOT_ALLOWED: &str = "Your organization doesn’t allow this AI server.";
pub const LOCAL_AI_NOT_ALLOWED: &str = "Your organization doesn’t allow an AI server on this computer.";

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
    /// `AllowedAIServers`: URL prefixes the OpenAI-compatible assistant may use. None: any. Empty: none (an
    /// unreadable value, applied as "deny").
    #[serde(default)]
    pub allowed_ai_servers: Option<Vec<String>>,
    /// `AIServer`: the server the organization set; it replaces the user's. Empty: unreadable, so refused.
    #[serde(default)]
    pub ai_server: Option<String>,
    /// `AllowLocalAI`: false refuses a server on this computer; true allows it even with external AI off.
    #[serde(default)]
    pub allow_local_ai: Option<bool>,
    /// The organization turned external AI off: a server on this computer then needs `AllowLocalAI`, since a local
    /// proxy could forward to the cloud.
    #[serde(default)]
    pub external_ai_managed_off: bool,
}

impl Default for CompliancePolicy {
    fn default() -> Self {
        CompliancePolicy {
            allowed_plugins: None,
            allow_external_ai: false,
            allow_remote_images: true,
            allowed_ai_servers: None,
            ai_server: None,
            allow_local_ai: None,
            external_ai_managed_off: false,
        }
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
            // A server on this computer doesn't need external AI: the account's server decides (`account_refusal`).
            if manifest.id == AI_SERVER_PLUGIN && self.local_ai_allowed() {
                return None;
            }
            return Some(EXTERNAL_AI_OFF);
        }
        None
    }

    /// Why an account is blocked: its plugin's refusal, and for the OpenAI-compatible assistant, its server's.
    pub fn account_refusal(
        &self,
        manifest: &PluginManifest,
        settings: &HashMap<String, String>,
    ) -> Option<&'static str> {
        self.refusal(manifest).or_else(|| {
            (manifest.id == AI_SERVER_PLUGIN).then(|| self.server_refusal(&self.ai_server_for(settings))).flatten()
        })
    }

    /// The server the OpenAI-compatible assistant uses: the organization's, else the account's, else OpenAI.
    pub fn ai_server_for(&self, settings: &HashMap<String, String>) -> String {
        if let Some(forced) = &self.ai_server {
            return forced.trim().to_string();
        }
        settings
            .get("host")
            .map(|h| h.trim().to_string())
            .filter(|h| !h.is_empty())
            .unwrap_or_else(|| DEFAULT_AI_SERVER.into())
    }

    /// Why a server can't be used: not in `AllowedAIServers`, a local one refused, or external AI off.
    pub fn server_refusal(&self, server: &str) -> Option<&'static str> {
        let url = ServerUrl::parse(server);
        if self.ai_server.as_ref().is_some_and(|forced| ServerUrl::parse(forced).is_none()) {
            return Some(SERVER_NOT_ALLOWED);
        }
        if let Some(prefixes) = &self.allowed_ai_servers {
            if !url.as_ref().is_some_and(|url| prefixes.iter().any(|p| url.starts_with_prefix(p))) {
                return Some(SERVER_NOT_ALLOWED);
            }
        }
        // An address Remora can't read is refused by the plugin itself, with what's wrong.
        let url = url?;
        if is_loopback(&url.host) {
            (!self.local_ai_allowed()).then_some(LOCAL_AI_NOT_ALLOWED)
        } else {
            (!self.allow_external_ai).then_some(EXTERNAL_AI_OFF)
        }
    }

    /// A server on this computer: off with `AllowLocalAI` false, on with true; unset, off only when the organization
    /// turned external AI off.
    pub fn local_ai_allowed(&self) -> bool {
        self.allow_local_ai.unwrap_or(!self.external_ai_managed_off)
    }
}

/// `localhost`, `127.0.0.0/8` or `::1` (brackets allowed): a server on this computer.
pub fn is_loopback(host: &str) -> bool {
    let host = host.trim().trim_start_matches('[').trim_end_matches(']').to_lowercase();
    host == "localhost" || host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
}

/// An http(s) address, as `AllowedAIServers` compares them: scheme, host (lowercase), port (443 or 80 when
/// absent) and path segments. Anything else (credentials in it, a query, another scheme) is unreadable.
#[derive(Clone, Debug, PartialEq)]
pub struct ServerUrl {
    pub scheme: String,
    pub host: String,
    pub port: u16,
    pub segments: Vec<String>,
}

impl ServerUrl {
    pub fn parse(raw: &str) -> Option<Self> {
        let (scheme, rest) = raw.trim().split_once("://")?;
        let scheme = scheme.to_lowercase();
        let default_port = match scheme.as_str() {
            "https" => 443,
            "http" => 80,
            _ => return None,
        };
        if rest.contains(['?', '#', '@', ' ']) {
            return None;
        }
        let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
        let (host, port) = if let Some(inner) = authority.strip_prefix('[') {
            let (ip, after) = inner.split_once(']')?;
            (format!("[{ip}]"), after.strip_prefix(':'))
        } else {
            match authority.rsplit_once(':') {
                Some((host, port)) => (host.to_string(), Some(port)),
                None => (authority.to_string(), None),
            }
        };
        let port = match port {
            Some(port) => port.parse().ok()?,
            None => default_port,
        };
        let host = host.to_lowercase();
        if host.is_empty() || host == "[]" {
            return None;
        }
        let segments = path.split('/').filter(|s| !s.is_empty()).map(str::to_string).collect();
        Some(ServerUrl { scheme, host, port, segments })
    }

    /// Same scheme, host and port, and the prefix's path segments first: `/openai/` allows `/openai/v1`, never
    /// `/openaix`. Trailing slashes don't matter.
    pub fn starts_with_prefix(&self, prefix: &str) -> bool {
        ServerUrl::parse(prefix).is_some_and(|p| {
            p.scheme == self.scheme
                && p.host == self.host
                && p.port == self.port
                && self.segments.len() >= p.segments.len()
                && self.segments.iter().zip(&p.segments).all(|(a, b)| a == b)
        })
    }
}

/// True when `host` is one of `allowed` or a subdomain of one (never a look-alike).
pub fn host_matches(host: &str, allowed: &[String]) -> bool {
    let host = host.to_lowercase();
    !host.is_empty()
        && allowed.iter().any(|a| {
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
            id: id.into(),
            name: id.into(),
            summary: String::new(),
            fields: vec![],
            setup_steps: vec![],
            setup_label: String::new(),
            setup_url: None,
            logo: None,
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
        let policy =
            CompliancePolicy { allowed_plugins: Some(HashSet::from(["github".to_string()])), ..Default::default() };
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

    fn openai() -> PluginManifest {
        manifest(AI_SERVER_PLUGIN, true)
    }

    fn server(host: &str) -> HashMap<String, String> {
        HashMap::from([("host".to_string(), host.to_string())])
    }

    #[test]
    fn server_prefixes_match_scheme_host_port_and_whole_segments() {
        let url = |raw: &str| ServerUrl::parse(raw).unwrap();
        let azure = url("https://acme.openai.azure.com/openai/v1");
        assert!(azure.starts_with_prefix("https://acme.openai.azure.com/openai/"));
        assert!(azure.starts_with_prefix("https://ACME.openai.azure.com/openai"), "host case and trailing slash");
        assert!(azure.starts_with_prefix("https://acme.openai.azure.com:443"), "default port");
        assert!(!url("https://acme.openai.azure.com/openaix/v1")
            .starts_with_prefix("https://acme.openai.azure.com/openai/"));
        assert!(!azure.starts_with_prefix("http://acme.openai.azure.com/openai"), "scheme");
        assert!(!azure.starts_with_prefix("https://other.openai.azure.com/openai"));
        assert!(!azure.starts_with_prefix("https://acme.openai.azure.com:8443/openai"), "port");
        assert!(url("http://localhost:11434/v1").starts_with_prefix("http://localhost:11434"));
        assert!(!url("http://localhost:8080/v1").starts_with_prefix("http://localhost:11434"));
        assert!(url("http://[::1]:11434/v1").starts_with_prefix("http://[::1]:11434/"));
        assert!(!azure.starts_with_prefix("not a url"));
        for bad in ["ftp://x", "https://", "https://user@x", "https://x/?q=1", "x.com", "https://x:port"] {
            assert_eq!(ServerUrl::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn loopback_is_this_computer_only() {
        for host in ["localhost", "LOCALHOST", "127.0.0.1", "127.8.9.10", "::1", "[::1]"] {
            assert!(is_loopback(host), "{host}");
        }
        for host in ["localhost.evil.com", "128.0.0.1", "10.0.0.1", "api.openai.com", ""] {
            assert!(!is_loopback(host), "{host}");
        }
    }

    #[test]
    fn allowed_ai_servers_restrict_where_the_assistant_goes() {
        let policy = CompliancePolicy {
            allow_external_ai: true,
            allowed_ai_servers: Some(vec!["https://acme.openai.azure.com/openai/".into()]),
            ..Default::default()
        };
        assert_eq!(policy.account_refusal(&openai(), &server("https://acme.openai.azure.com/openai/v1")), None);
        assert_eq!(policy.account_refusal(&openai(), &server("https://api.openai.com/v1")), Some(SERVER_NOT_ALLOWED));
        assert_eq!(policy.account_refusal(&openai(), &HashMap::new()), Some(SERVER_NOT_ALLOWED), "OpenAI by default");
        let unreadable =
            CompliancePolicy { allow_external_ai: true, allowed_ai_servers: Some(vec![]), ..Default::default() };
        assert_eq!(
            unreadable.account_refusal(&openai(), &server("https://api.openai.com/v1")),
            Some(SERVER_NOT_ALLOWED)
        );
        // Other assistants and tools don't have a server to check.
        assert_eq!(policy.account_refusal(&manifest("claude", true), &server("https://api.openai.com/v1")), None);
    }

    #[test]
    fn the_organizations_server_replaces_the_users() {
        let forced = CompliancePolicy {
            allow_external_ai: true,
            ai_server: Some("https://acme.openai.azure.com/openai/v1".into()),
            ..Default::default()
        };
        assert_eq!(forced.ai_server_for(&server("https://evil.example/v1")), "https://acme.openai.azure.com/openai/v1");
        assert_eq!(forced.account_refusal(&openai(), &server("https://evil.example/v1")), None);
        let outside =
            CompliancePolicy { allowed_ai_servers: Some(vec!["https://api.openai.com/v1".into()]), ..forced.clone() };
        assert_eq!(outside.account_refusal(&openai(), &HashMap::new()), Some(SERVER_NOT_ALLOWED), "not in the list");
        let unreadable = CompliancePolicy { ai_server: Some(String::new()), ..forced };
        assert_eq!(unreadable.account_refusal(&openai(), &HashMap::new()), Some(SERVER_NOT_ALLOWED));
    }

    /// AllowLocalAI × external AI (the user's, or the organization's).
    #[test]
    fn local_servers_follow_allow_local_ai() {
        let local = server("http://localhost:11434/v1");
        let cloud = server("https://api.openai.com/v1");
        let case = |allow_local_ai, external, managed_off| CompliancePolicy {
            allow_local_ai,
            allow_external_ai: external,
            external_ai_managed_off: managed_off,
            ..Default::default()
        };
        // Unset: on, even with the user's external AI off; off when the organization turned external AI off.
        assert_eq!(case(None, false, false).account_refusal(&openai(), &local), None);
        assert_eq!(case(None, false, false).account_refusal(&openai(), &cloud), Some(EXTERNAL_AI_OFF));
        assert_eq!(case(None, false, false).refusal(&openai()), None, "can be connected, for a local server");
        assert_eq!(case(None, false, true).account_refusal(&openai(), &local), Some(EXTERNAL_AI_OFF));
        assert_eq!(case(None, true, false).account_refusal(&openai(), &local), None);
        // True: on, whatever external AI says.
        assert_eq!(case(Some(true), false, true).account_refusal(&openai(), &local), None);
        assert_eq!(case(Some(true), false, true).account_refusal(&openai(), &cloud), Some(EXTERNAL_AI_OFF));
        // False: off, even with external AI on.
        assert_eq!(case(Some(false), true, false).account_refusal(&openai(), &local), Some(LOCAL_AI_NOT_ALLOWED));
        assert_eq!(case(Some(false), true, false).account_refusal(&openai(), &cloud), None);
        assert_eq!(case(Some(false), false, false).refusal(&openai()), Some(EXTERNAL_AI_OFF));
        // Claude always needs external AI.
        assert_eq!(case(Some(true), false, false).refusal(&manifest("claude", true)), Some(EXTERNAL_AI_OFF));
    }
}

//! Which links Remora opens. Item links come from server fields, so a hostile or broken server could send
//! `file:`, UNC or custom-scheme URLs: only web pages and the tools' own desktop apps are opened. Same rules as
//! `LinkPolicy` on macOS.

use crate::PluginConfig;
use remora_core::{Account, PluginManifest};
use std::collections::HashMap;

/// Schemes of the desktop apps Remora hands links to (Slack, Linear).
pub const APP_SCHEMES: [&str; 2] = ["slack", "linear"];

/// An https page, or an http one on `http_hosts` (a self-hosted server the user connected over http).
pub fn is_web_link(url: &str, http_hosts: &[String]) -> bool {
    let Ok(url) = reqwest::Url::parse(url) else { return false };
    let Some(host) = url.host_str().map(str::to_lowercase) else { return false };
    match url.scheme() {
        "https" => true,
        "http" => http_hosts.contains(&host),
        _ => false,
    }
}

pub fn is_app_link(url: &str) -> bool {
    reqwest::Url::parse(url).is_ok_and(|u| APP_SCHEMES.contains(&u.scheme()))
}

/// The hosts an account reaches over http: its own host, when the user entered an `http://` address.
pub fn http_hosts(account: Option<&Account>) -> Vec<String> {
    account
        .and_then(|a| a.settings.get("host"))
        .map(|h| h.trim().to_lowercase())
        .filter(|h| h.starts_with("http://"))
        .and_then(|h| reqwest::Url::parse(&h).ok()?.host_str().map(str::to_string))
        .into_iter()
        .collect()
}

/// The plugin's token page, `{host}` replaced by the host typed in the form (else the field's default).
/// Only https pages: the link is built from what the user typed.
pub fn setup_url(manifest: &PluginManifest, host: Option<&str>) -> Option<String> {
    let template = manifest.setup_url.as_deref()?;
    let url = if template.contains("{host}") {
        let typed = host.map(str::trim).filter(|h| !h.is_empty());
        let default = manifest.fields.iter().find(|f| f.key == "host").map(|f| f.default_value.clone());
        let raw = typed.map(str::to_string).or(default)?;
        let config = PluginConfig { account_id: String::new(), values: HashMap::from([("host".to_string(), raw)]) };
        template.replace("{host}", &config.url("host").ok()?)
    } else {
        template.to_string()
    };
    is_web_link(&url, &[]).then_some(url)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{github, gitlab, linear, notion};

    #[test]
    fn opens_only_web_pages_and_tool_apps() {
        assert!(is_web_link("https://github.com/acme/app/pull/1", &[]));
        for url in ["file:///C:/Windows/System32/calc.exe", "\\\\evil\\share", "ms-settings:privacy", "javascript:alert(1)", "ftp://h/x"] {
            assert!(!is_web_link(url, &[]), "{url}");
        }
        assert!(is_app_link("slack://channel?team=T1&id=C1"));
        assert!(is_app_link("linear:/acme/issue/ENG-1"));
        assert!(!is_app_link("ms-settings:privacy"));
        assert!(!is_app_link("file:///C:/x"));
    }

    #[test]
    fn http_only_on_the_accounts_own_host() {
        let account = |host: &str| Account {
            id: "a".into(), plugin_id: "gitlab".into(), name: None, identity: None,
            settings: HashMap::from([("host".to_string(), host.to_string())]),
        };
        let hosts = http_hosts(Some(&account("http://gitlab.lan")));
        assert!(is_web_link("http://gitlab.lan/a/-/merge_requests/3", &hosts));
        assert!(!is_web_link("http://elsewhere.io/x", &hosts));
        assert!(http_hosts(Some(&account("gitlab.com"))).is_empty());
    }

    #[test]
    fn setup_links_use_the_typed_host() {
        assert_eq!(setup_url(&github::manifest(), None).as_deref(), Some("https://github.com/settings/tokens/new?scopes=repo,read:org&description=Remora"));
        assert_eq!(
            setup_url(&gitlab::manifest(), Some("gitlab.acme.io/")).as_deref(),
            Some("https://gitlab.acme.io/-/user_settings/personal_access_tokens?name=Remora&scopes=read_api")
        );
        assert_eq!(setup_url(&gitlab::manifest(), Some("  ")).as_deref(), Some("https://gitlab.com/-/user_settings/personal_access_tokens?name=Remora&scopes=read_api"));
        assert_eq!(setup_url(&gitlab::manifest(), Some("http://gitlab.lan")), None);
        assert_eq!(setup_url(&gitlab::manifest(), Some("file:///C:/x")), None);
        assert_eq!(setup_url(&linear::manifest(), Some("ignored")).as_deref(), Some("https://linear.app/settings/account/security"));
        assert_eq!(setup_url(&notion::manifest(), None).as_deref(), Some("https://www.notion.so/developers/tokens"));
    }
}

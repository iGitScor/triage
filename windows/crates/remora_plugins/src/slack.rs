use crate::{decode, url_with_query, HttpClient, PluginConfig, PluginError, Request, SourcePlugin, SourceSnapshot};
use async_trait::async_trait;
use chrono::{DateTime, Duration, TimeZone, Utc};
use regex::Regex;
use remora_core::{ConfigField, Egress, InboxBundle, InboxItem, Person, PluginManifest};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;

pub fn manifest() -> PluginManifest {
    PluginManifest {
        id: "slack".into(),
        name: "Slack".into(),
        summary: "Mentions and direct messages from the last few days.".into(),
        fields: vec![
            ConfigField::token("User OAuth token", "A user token (xoxp-…) with the search:read scope."),
            ConfigField { key: "days".into(), label: "Look back (days)".into(), placeholder: "3".into(), default_value: "3".into(), is_secret: false, is_optional: false, help: None },
        ],
        setup_steps: vec![
            "Click “Create the Slack app”, pick your workspace, then Next and Create. The app comes pre-filled.".into(),
            "In the app page, open “Install App” and click “Install to <workspace>”, then Allow.".into(),
            "Copy the “User OAuth Token” (it starts with xoxp-) and paste it below.".into(),
        ],
        setup_label: "Create the Slack app".into(),
        setup_url: Some(app_manifest_url()),
        egress: Egress { hosts: vec!["slack.com".into()], description: "Searches your recent mentions and direct messages in Slack.".into(), external_ai: false },
        logo: Some("slack".into()),
    }
}

/// Opens Slack's "create app" flow pre-filled with the one scope Remora needs.
pub fn app_manifest_url() -> String {
    let manifest = r#"{"display_information":{"name":"Remora","description":"Mentions and DMs in your inbox"},"oauth_config":{"scopes":{"user":["search:read"]}},"settings":{"org_deploy_enabled":false,"socket_mode_enabled":false,"token_rotation_enabled":false}}"#;
    url_with_query("https://api.slack.com", "apps", &[("new_app", "1"), ("manifest_json", manifest)])
}

/// Recent mentions and direct messages, found with Slack's search API.
pub struct SlackPlugin {
    account_id: String,
    token: String,
    days: i64,
    http: Arc<dyn HttpClient>,
}

impl SlackPlugin {
    pub fn new(config: &PluginConfig, http: Arc<dyn HttpClient>) -> Result<Self, PluginError> {
        Ok(SlackPlugin { account_id: config.account_id.clone(), token: config.required("token")?, days: config.get("days").parse().unwrap_or(3).max(1), http })
    }

    async fn call<T: DeserializeOwned>(&self, method: &str, query: &[(&str, &str)]) -> Result<T, PluginError> {
        let mut query = query.to_vec();
        if method == "search.messages" {
            query.extend([("sort", "timestamp"), ("sort_dir", "desc"), ("count", "40")]);
        }
        let request = Request::get(url_with_query("https://slack.com/api", method, &query)).header("Authorization", format!("Bearer {}", self.token));
        let envelope: serde_json::Value = decode(self.http.as_ref(), request).await?;
        if envelope.get("ok").and_then(|v| v.as_bool()) != Some(true) {
            let error = envelope.get("error").and_then(|v| v.as_str()).unwrap_or("unknown error");
            return Err(if matches!(error, "invalid_auth" | "not_authed") { PluginError::Unauthorized } else { PluginError::Api(format!("Slack: {error}")) });
        }
        serde_json::from_value(envelope).map_err(|e| PluginError::Decode(e.to_string()))
    }

    fn snapshot(mentions: Vec<Match>, direct: Vec<Match>, identity: String, me: &str, team: Option<&str>, account_id: &str) -> SourceSnapshot {
        let mut items: Vec<InboxItem> = mentions
            .iter()
            .filter(|m| !m.channel.is_direct())
            .map(|m| m.item(account_id, InboxBundle::mentions(), &format!("{}/{}", m.channel.id, m.ts), me, team))
            .collect();
        let mut latest: HashMap<&str, &Match> = HashMap::new();
        for m in direct.iter().filter(|m| m.channel.is_direct()) {
            let entry = latest.entry(m.channel.id.as_str()).or_insert(m);
            if m.date() > entry.date() {
                *entry = m;
            }
        }
        items.extend(latest.values().map(|m| m.item(account_id, InboxBundle::direct_messages(), &m.channel.id, me, team)));
        SourceSnapshot { identity, items }
    }
}

#[async_trait]
impl SourcePlugin for SlackPlugin {
    async fn fetch(&self) -> Result<SourceSnapshot, PluginError> {
        let auth: Auth = self.call("auth.test", &[]).await?;
        let since = (Utc::now() - Duration::days(self.days)).format("%Y-%m-%d").to_string();
        let me = format!("<@{}>", auth.user_id);
        let mentions_query = format!("{me} -from:{me} after:{since}");
        let direct_query = format!("to:me -from:{me} after:{since}");
        let mentions_params = [("query", mentions_query.as_str())];
        let direct_params = [("query", direct_query.as_str())];
        let (mentions, direct) = futures::join!(
            self.call::<Search>("search.messages", &mentions_params),
            self.call::<Search>("search.messages", &direct_params)
        );
        Ok(Self::snapshot(
            mentions?.messages.matches,
            direct?.messages.matches,
            format!("{} · {}", auth.user, auth.team),
            &auth.user_id,
            auth.team_id.as_deref(),
            &self.account_id,
        ))
    }
}

#[derive(Deserialize)]
struct Auth {
    user_id: String,
    user: String,
    team: String,
    team_id: Option<String>,
}

#[derive(Deserialize)]
struct Search {
    messages: Messages,
}

#[derive(Deserialize)]
struct Messages {
    matches: Vec<Match>,
}

#[derive(Deserialize)]
struct Channel {
    id: String,
    name: Option<String>,
    is_im: Option<bool>,
    is_mpim: Option<bool>,
}

impl Channel {
    fn is_direct(&self) -> bool {
        self.is_im == Some(true) || self.is_mpim == Some(true)
    }

    fn label(&self) -> String {
        if self.is_im == Some(true) {
            "Direct message".into()
        } else if self.is_mpim == Some(true) {
            "Group message".into()
        } else {
            format!("#{}", self.name.as_deref().unwrap_or(&self.id))
        }
    }
}

#[derive(Deserialize)]
struct Match {
    ts: String,
    text: String,
    permalink: Option<String>,
    username: Option<String>,
    channel: Channel,
}

impl Match {
    fn date(&self) -> DateTime<Utc> {
        let seconds = self.ts.parse::<f64>().unwrap_or(0.0);
        Utc.timestamp_opt(seconds as i64, 0).single().unwrap_or_default()
    }

    /// Slack documents app links for channels and DMs, not single messages.
    fn app_url(&self, team: Option<&str>) -> Option<String> {
        team.map(|team| format!("slack://channel?team={team}&id={}", self.channel.id))
    }

    fn item(&self, account_id: &str, bundle: InboxBundle, id: &str, me: &str, team: Option<&str>) -> InboxItem {
        let plain = plain_text(&self.text, me);
        let lines: Vec<&str> = plain.lines().filter(|l| !l.trim().is_empty()).collect();
        let preview = lines.iter().skip(1).copied().collect::<Vec<_>>().join(" ");
        InboxItem {
            id: format!("{account_id}/{id}"),
            account_id: account_id.into(),
            plugin_id: "slack".into(),
            bundle,
            title: lines.first().map(|l| l.to_string()).unwrap_or_else(|| "(no text)".into()),
            context: self.channel.label(),
            preview: (!preview.is_empty()).then_some(preview),
            url: self.permalink.clone(),
            app_url: self.app_url(team),
            author: self.username.as_deref().map(Person::named),
            participants: vec![],
            badges: vec![],
            date: self.date(),
            needs_action: true,
            priority: None,
            due: None,
        }
    }
}

/// Slack mrkdwn (`<@U1|alice>`, `<https://x|label>`, `&amp;`) to readable text.
pub fn plain_text(text: &str, me: &str) -> String {
    let mut result = text.replace(&format!("<@{me}>"), "@you");
    let rules = [
        (r"<@[A-Z0-9]+\|([^>]+)>", "@$1"),
        (r"<@([A-Z0-9]+)>", "@someone"),
        (r"<#[A-Z0-9]+\|([^>]*)>", "#$1"),
        (r"<!(here|channel|everyone)[^>]*>", "@$1"),
        (r"<(https?://[^|>]+)\|([^>]+)>", "$2"),
        (r"<(https?://[^>]+)>", "$1"),
    ];
    for (pattern, template) in rules {
        result = Regex::new(pattern).expect("valid pattern").replace_all(&result, template).into_owned();
    }
    result.replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stub::*;

    #[test]
    fn cleans_mrkdwn() {
        let text = "<@UME> see <https://x.io|the doc> &amp; ping <@U2|erin> in <#C1|general> <!here>";
        assert_eq!(plain_text(text, "UME"), "@you see the doc & ping @erin in #general @here");
    }

    #[tokio::test]
    async fn keeps_the_latest_message_per_conversation_with_app_links() {
        let search = r#"{"ok": true, "messages": {"matches": [
          {"ts": "1700000100.0", "text": "second", "username": "carol", "channel": {"id": "D1", "is_im": true}},
          {"ts": "1700000000.0", "text": "first", "username": "carol", "channel": {"id": "D1", "is_im": true}},
          {"ts": "1700000050.0", "text": "hey <@UME>\nmore", "username": "erin", "channel": {"id": "C1", "name": "dev"}}
        ]}}"#;
        let http = StubHttp::paths(&[("/auth.test", r#"{"ok": true, "user_id": "UME", "user": "alice", "team": "Acme", "team_id": "T42"}"#), ("/search.messages", search)]);
        let snapshot = SlackPlugin::new(&config(&[("token", "xoxp")]), Arc::new(http)).unwrap().fetch().await.unwrap();

        let direct: Vec<&InboxItem> = snapshot.items.iter().filter(|i| i.bundle == InboxBundle::direct_messages()).collect();
        assert_eq!(direct.iter().map(|i| i.title.as_str()).collect::<Vec<_>>(), ["second"]);
        assert_eq!(direct[0].app_url.as_deref(), Some("slack://channel?team=T42&id=D1"));
        let mention = snapshot.items.iter().find(|i| i.bundle == InboxBundle::mentions()).unwrap();
        assert_eq!((mention.title.as_str(), mention.preview.as_deref(), mention.context.as_str()), ("hey @you", Some("more"), "#dev"));
    }

    #[tokio::test]
    async fn reports_slack_errors() {
        let http = StubHttp::paths(&[("/auth.test", r#"{"ok": false, "error": "invalid_auth"}"#)]);
        let result = SlackPlugin::new(&config(&[("token", "bad")]), Arc::new(http)).unwrap().fetch().await;
        assert_eq!(result.err(), Some(PluginError::Unauthorized));
    }

    #[test]
    fn app_manifest_link_requests_search_scope() {
        let url = reqwest::Url::parse(&app_manifest_url()).unwrap();
        let manifest = url.query_pairs().find(|(k, _)| k == "manifest_json").map(|(_, v)| v.into_owned()).unwrap();
        let json: serde_json::Value = serde_json::from_str(&manifest).unwrap();
        assert_eq!(json["oauth_config"]["scopes"]["user"], serde_json::json!(["search:read"]));
    }
}

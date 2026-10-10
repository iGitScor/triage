use crate::{decode, url_with_query, HttpClient, PluginConfig, PluginError, Request, SourcePlugin, SourceSnapshot};
use async_trait::async_trait;
use chrono::{DateTime, Datelike, Duration, FixedOffset, Local, TimeZone, Utc};
use regex::Regex;
use remora_core::{ConfigField, Egress, InboxBundle, InboxItem, Person, PluginManifest};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock};

pub fn manifest() -> PluginManifest {
    PluginManifest {
        id: "slack".into(),
        name: "Slack".into(),
        summary: "Mentions, direct messages and replies in your threads from the last few days.".into(),
        fields: vec![
            ConfigField::token(
                "User OAuth token",
                "A user token (xoxp-…) with search:read, plus channels:history and groups:history for thread replies.",
            ),
            ConfigField {
                key: "days".into(),
                label: "Look back (days)".into(),
                placeholder: "3".into(),
                default_value: "3".into(),
                is_secret: false,
                is_optional: false,
                help: None,
            },
        ],
        setup_steps: vec![
            "Click “Create the Slack app”, pick your workspace, then Next and Create. The app comes pre-filled.".into(),
            "In the app page, open “Install App” and click “Install to <workspace>”, then Allow.".into(),
            "Copy the “User OAuth Token” (it starts with xoxp-) and paste it below.".into(),
        ],
        setup_label: "Create the Slack app".into(),
        setup_url: Some(app_manifest_url()),
        egress: Egress {
            hosts: vec!["slack.com".into()],
            description:
                "Searches your recent mentions and direct messages in Slack, and reads the threads you wrote in.".into(),
            external_ai: false,
        },
        logo: Some("slack".into()),
    }
}

/// Opens Slack's "create app" flow pre-filled with the read-only scopes Remora needs: search, and the channels'
/// history for replies in your threads.
pub fn app_manifest_url() -> String {
    let manifest = r#"{"display_information":{"name":"Remora","description":"Mentions and DMs in your inbox"},"oauth_config":{"scopes":{"user":["search:read","channels:history","groups:history"]}},"settings":{"org_deploy_enabled":false,"socket_mode_enabled":false,"token_rotation_enabled":false}}"#;
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
        Ok(SlackPlugin {
            account_id: config.account_id.clone(),
            token: config.required("token")?,
            days: config.get("days").parse().unwrap_or(3).max(1),
            http,
        })
    }

    async fn call<T: DeserializeOwned>(&self, method: &str, query: &[(&str, &str)]) -> Result<T, PluginError> {
        let mut query = query.to_vec();
        if method == "search.messages" {
            query.extend([("sort", "timestamp"), ("sort_dir", "desc"), ("count", "40")]);
        }
        let request = Request::get(url_with_query("https://slack.com/api", method, &query))
            .header("Authorization", format!("Bearer {}", self.token));
        let envelope: serde_json::Value = decode(self.http.as_ref(), request).await?;
        if envelope.get("ok").and_then(|v| v.as_bool()) != Some(true) {
            let error = envelope.get("error").and_then(|v| v.as_str()).unwrap_or("unknown error");
            return Err(if matches!(error, "invalid_auth" | "not_authed") {
                PluginError::Unauthorized
            } else {
                PluginError::Api(format!("Slack: {error}"))
            });
        }
        serde_json::from_value(envelope).map_err(|e| PluginError::Decode(e.to_string()))
    }

    fn snapshot(
        mentions: Vec<Match>,
        direct: Vec<Match>,
        identity: String,
        me: &str,
        team: Option<&str>,
        account_id: &str,
    ) -> SourceSnapshot {
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
        items.extend(
            latest.values().map(|m| m.item(account_id, InboxBundle::direct_messages(), &m.channel.id, me, team)),
        );
        SourceSnapshot { identity, items, remarks: vec![] }
    }

    /// Replies in the threads you wrote in lately, posted after your last message there: what search
    /// can't find, since nobody mentioned you. The 10 most recent channel threads, one `conversations.replies` each;
    /// replies that mention you are already mentions, and direct messages are already there. Without the history
    /// scopes, a remark says how to add them. Same rules as macOS.
    async fn thread_replies(&self, mine: Vec<Match>, me: &str, team: Option<&str>) -> (Vec<InboxItem>, Vec<String>) {
        let mut latest: HashMap<(String, String), Match> = HashMap::new();
        for message in mine.into_iter().filter(|m| !m.channel.is_direct()) {
            let key = (message.channel.id.clone(), message.thread_ts());
            if latest.get(&key).is_none_or(|known| known.date() < message.date()) {
                latest.insert(key, message);
            }
        }
        let mut threads: Vec<Match> = latest.into_values().collect();
        threads.sort_by_key(|m| std::cmp::Reverse(m.date()));
        let mut items = vec![];
        for thread in threads.into_iter().take(10) {
            let thread_ts = thread.thread_ts();
            let query = [
                ("channel", thread.channel.id.as_str()),
                ("ts", thread_ts.as_str()),
                ("oldest", thread.ts.as_str()),
                ("limit", "50"),
            ];
            let replies = match self.call::<Replies>("conversations.replies", &query).await {
                Ok(replies) => replies,
                Err(PluginError::Api(message)) if message.ends_with("missing_scope") => {
                    return (items, vec![MISSING_HISTORY.to_string()])
                }
                Err(_) => continue,
            };
            let mention = format!("<@{me}>");
            let reply = replies
                .messages
                .into_iter()
                .filter(|r| {
                    r.ts != thread.ts
                        && r.user.as_deref() != Some(me)
                        && r.bot_id.is_none()
                        && !r.text.contains(&mention)
                })
                .max_by_key(Reply::date);
            if let Some(reply) = reply {
                items.push(reply.item(&thread, &self.account_id, me, team));
            }
        }
        (items, vec![])
    }
}

/// Shown when the token can't read threads. Same text on macOS.
const MISSING_HISTORY: &str = "Slack: replies in your threads need the channels:history and groups:history permissions. Add them to the Remora app in Slack, reinstall it, and paste the new token.";

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
        let mine_query = format!("from:{me} is:thread after:{since}");
        let mine_params = [("query", mine_query.as_str())];
        let (mentions, direct, mine) = futures::join!(
            self.call::<Search>("search.messages", &mentions_params),
            self.call::<Search>("search.messages", &direct_params),
            self.call::<Search>("search.messages", &mine_params)
        );
        let mut snapshot = Self::snapshot(
            mentions?.messages.matches,
            direct?.messages.matches,
            format!("{} · {}", auth.user, auth.team),
            &auth.user_id,
            auth.team_id.as_deref(),
            &self.account_id,
        );
        let mine = mine.map(|s| s.messages.matches).unwrap_or_default();
        let (threads, remarks) = self.thread_replies(mine, &auth.user_id, auth.team_id.as_deref()).await;
        snapshot.items.extend(threads);
        snapshot.remarks.extend(remarks);
        Ok(snapshot)
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

/// A thread's messages from `conversations.replies`, the root first.
#[derive(Deserialize)]
struct Replies {
    messages: Vec<Reply>,
}

#[derive(Deserialize)]
struct Reply {
    ts: String,
    #[serde(default)]
    text: String,
    user: Option<String>,
    bot_id: Option<String>,
    #[serde(default)]
    user_profile: Option<Profile>,
}

#[derive(Deserialize)]
struct Profile {
    display_name: Option<String>,
    real_name: Option<String>,
}

impl Reply {
    fn date(&self) -> DateTime<Utc> {
        let seconds = self.ts.parse::<f64>().unwrap_or(0.0);
        Utc.timestamp_opt(seconds as i64, 0).single().unwrap_or_default()
    }

    /// "Thread in #design": one item per thread, its latest reply, opening the thread where you wrote.
    fn item(&self, thread: &Match, account_id: &str, me: &str, team: Option<&str>) -> InboxItem {
        let plain = plain_text(&self.text, me, &DateStyle::system());
        let lines: Vec<&str> = plain.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
        let preview = lines.iter().skip(1).copied().collect::<Vec<_>>().join(" ");
        let name = self
            .user_profile
            .as_ref()
            .and_then(|p| [&p.display_name, &p.real_name].into_iter().flatten().find(|n| !n.is_empty()).cloned());
        InboxItem {
            id: format!("{account_id}/{}/thread/{}", thread.channel.id, thread.thread_ts()),
            account_id: account_id.into(),
            plugin_id: "slack".into(),
            bundle: InboxBundle::mentions(),
            title: lines.first().map(|l| l.to_string()).unwrap_or_else(|| "(no text)".into()),
            context: format!("Thread in {}", thread.channel.label()),
            preview: (!preview.is_empty()).then_some(preview),
            url: thread.permalink.clone(),
            app_url: thread.app_url(team),
            author: Some(Person::named(&name.unwrap_or_else(|| "Someone".into()))),
            participants: vec![],
            badges: vec![],
            date: self.date(),
            needs_action: true,
            priority: None,
            due: None,
            expires: None,
            changes: None,
            suggested_people: None,
        }
    }
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

/// What apps put in a message besides (or instead of) `text`.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Attachment {
    fallback: Option<String>,
    pretext: Option<String>,
    title: Option<String>,
    text: Option<String>,
}

impl Attachment {
    fn words(&self) -> String {
        let parts: Vec<&str> = [&self.pretext, &self.title, &self.text]
            .into_iter()
            .flatten()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        if parts.is_empty() {
            self.fallback.clone().unwrap_or_default()
        } else {
            parts.join("\n")
        }
    }
}

#[derive(Deserialize)]
struct Match {
    ts: String,
    #[serde(default)]
    text: String,
    permalink: Option<String>,
    username: Option<String>,
    channel: Channel,
    bot_id: Option<String>,
    subtype: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    attachments: Vec<Attachment>,
    #[serde(default)]
    blocks: serde_json::Value,
}

/// Attachments in a shape we don't expect are ignored rather than failing the whole search.
fn lenient<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Vec<Attachment>, D::Error> {
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(serde_json::from_value(value).unwrap_or_default())
}

/// Every string under a "text" key, at any depth: the words of Slack's Block Kit blocks.
fn text_leaves(value: &serde_json::Value, out: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, child) in map {
                match (key.as_str(), child) {
                    ("text", serde_json::Value::String(s)) => out.push(s.clone()),
                    _ => text_leaves(child, out),
                }
            }
        }
        serde_json::Value::Array(items) => items.iter().for_each(|i| text_leaves(i, out)),
        _ => {}
    }
}

impl Match {
    /// The thread this message is in: its permalink carries `thread_ts` for a reply; a thread's first message is
    /// its own root.
    fn thread_ts(&self) -> String {
        self.permalink
            .as_deref()
            .and_then(|link| reqwest::Url::parse(link).ok())
            .and_then(|url| url.query_pairs().find(|(k, _)| k == "thread_ts").map(|(_, v)| v.into_owned()))
            .unwrap_or_else(|| self.ts.clone())
    }

    fn date(&self) -> DateTime<Utc> {
        let seconds = self.ts.parse::<f64>().unwrap_or(0.0);
        Utc.timestamp_opt(seconds as i64, 0).single().unwrap_or_default()
    }

    /// Slack documents app links for channels and DMs, not single messages. The ids come from the server, so
    /// they're encoded as query values: an id can't add a parameter or change the link.
    fn app_url(&self, team: Option<&str>) -> Option<String> {
        let mut url = reqwest::Url::parse("slack://channel").ok()?;
        url.query_pairs_mut().append_pair("team", team?).append_pair("id", &self.channel.id);
        Some(url.to_string())
    }

    /// Apps (Google Calendar, Jira, GitHub…) post with a bot id, or with an empty `text` and their content in
    /// attachments or blocks. They inform; nobody waits for a reply.
    fn is_from_app(&self) -> bool {
        self.bot_id.is_some() || self.subtype.as_deref() == Some("bot_message") || self.text.trim().is_empty()
    }

    /// The message's words: its text, else what its attachments or blocks say.
    fn words(&self) -> String {
        if !self.text.trim().is_empty() {
            return self.text.clone();
        }
        let from_attachments: Vec<String> =
            self.attachments.iter().map(Attachment::words).filter(|w| !w.is_empty()).collect();
        if !from_attachments.is_empty() {
            return from_attachments.join("\n");
        }
        let mut leaves = vec![];
        text_leaves(&self.blocks, &mut leaves);
        leaves.join("\n")
    }

    fn item(&self, account_id: &str, bundle: InboxBundle, id: &str, me: &str, team: Option<&str>) -> InboxItem {
        let from_app = self.is_from_app();
        let plain = plain_text(&self.words(), me, &DateStyle::system());
        let lines: Vec<&str> = plain.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
        let preview = lines.iter().skip(1).copied().collect::<Vec<_>>().join(" ");
        InboxItem {
            id: format!("{account_id}/{id}"),
            account_id: account_id.into(),
            plugin_id: "slack".into(),
            bundle: if from_app { InboxBundle::read() } else { bundle },
            title: lines.first().map(|l| l.to_string()).unwrap_or_else(|| "(no text)".into()),
            context: self.channel.label(),
            preview: (!preview.is_empty()).then_some(preview),
            url: self.permalink.clone(),
            app_url: self.app_url(team),
            author: self.username.as_deref().map(Person::named),
            participants: vec![],
            badges: vec![],
            date: self.date(),
            needs_action: !from_app,
            priority: None,
            due: None,
            expires: self.expires(),
            changes: None,
            suggested_people: None,
        }
    }

    /// An app's message about a time to come (a calendar reminder) is over once that time comes: the first time
    /// it mentions after it was posted. People's messages never expire.
    fn expires(&self) -> Option<DateTime<Utc>> {
        if !self.is_from_app() {
            return None;
        }
        times(&self.words()).into_iter().filter(|t| *t > self.date()).min()
    }
}

/// How Slack's date tags read: in the system's language and time zone.
pub struct DateStyle {
    /// None: the system's time zone.
    pub offset: Option<FixedOffset>,
    pub french: bool,
    /// US English: 12-hour times and month-first dates.
    pub us: bool,
    /// What `{ago}` counts from.
    pub now: DateTime<Utc>,
}

impl DateStyle {
    pub fn system() -> Self {
        let locale = sys_locale::get_locale().unwrap_or_default().to_lowercase().replace('_', "-");
        DateStyle {
            offset: None,
            french: locale.starts_with("fr"),
            us: locale == "en-us" || locale == "en",
            now: Utc::now(),
        }
    }
}

static RULES: LazyLock<Vec<(Regex, &str)>> = LazyLock::new(|| {
    [
        (r"<@[A-Z0-9]+\|([^>]+)>", "@$1"),
        (r"<@([A-Z0-9]+)>", "@someone"),
        (r"<#[A-Z0-9]+\|([^>]*)>", "#$1"),
        (r"<!(here|channel|everyone)[^>]*>", "@$1"),
        (r"<!subteam\^[A-Z0-9]+\|([^>]+)>", "$1"),
        (r"<!subteam\^[A-Z0-9]+>", "@team"),
        (r"<!date\^[^>]*>", ""),
        (r"<(?:https?|mailto|tel):[^|>]+\|([^>]+)>", "$1"),
        (r"<(https?://[^>]+)>", "$1"),
        (r"<(?:mailto|tel):([^>]+)>", "$1"),
    ]
    .into_iter()
    .map(|(pattern, template)| (Regex::new(pattern).expect("valid pattern"), template))
    .collect()
});
/// `<!date^seconds^format|fallback>` and `<!date^seconds^format^link|fallback>`.
static DATE_TAG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<!date\^(\d+)\^([^^|>]*)(?:\^[^|>]*)?(?:\|([^>]*))?>").expect("valid pattern"));
static TOKEN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\{([a-z_]+)\}").expect("valid pattern"));

/// Slack mrkdwn to readable text: people, channels, links, emails and phone numbers by their label, dates in your
/// language and time zone (`<!date^1791555300^{time}|4:15 PM>` → 16:15), then `text::readable` (emoji, no marks).
/// Same rules on macOS (`SlackText`).
pub fn plain_text(text: &str, me: &str, style: &DateStyle) -> String {
    let result = text.replace(&format!("<@{me}>"), "@you");
    let mut result = DATE_TAG
        .replace_all(&result, |c: &regex::Captures| {
            let fallback = c.get(3).map(|m| m.as_str());
            c[1].parse::<i64>().ok().and_then(|s| Utc.timestamp_opt(s, 0).single()).map_or_else(
                || fallback.unwrap_or_default().to_string(),
                |date| formatted(date, &c[2], fallback, style),
            )
        })
        .into_owned();
    for (pattern, template) in RULES.iter() {
        result = pattern.replace_all(&result, *template).into_owned();
    }
    remora_core::text::readable(&result.replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&"))
}

/// The times a message's date tags point at.
pub fn times(text: &str) -> Vec<DateTime<Utc>> {
    DATE_TAG
        .captures_iter(text)
        .filter_map(|c| c[1].parse::<i64>().ok())
        .filter_map(|s| Utc.timestamp_opt(s, 0).single())
        .collect()
}

/// Slack's tokens ({time}, {date_short}, {date_pretty}, {ago}…) in place; an unknown one gives the fallback.
pub fn formatted(date: DateTime<Utc>, format: &str, fallback: Option<&str>, style: &DateStyle) -> String {
    let mut unknown = false;
    let result = TOKEN.replace_all(format, |c: &regex::Captures| {
        token(&c[1], date, style).unwrap_or_else(|| {
            unknown = true;
            String::new()
        })
    });
    if unknown {
        fallback.unwrap_or_default().to_string()
    } else {
        result.into_owned()
    }
}

const MONTHS_FR: [&str; 12] = [
    "janvier",
    "février",
    "mars",
    "avril",
    "mai",
    "juin",
    "juillet",
    "août",
    "septembre",
    "octobre",
    "novembre",
    "décembre",
];
const MONTHS_FR_SHORT: [&str; 12] =
    ["janv.", "févr.", "mars", "avr.", "mai", "juin", "juil.", "août", "sept.", "oct.", "nov.", "déc."];
const DAYS_FR: [&str; 7] = ["lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi", "dimanche"];

fn token(name: &str, date: DateTime<Utc>, style: &DateStyle) -> Option<String> {
    let local = |d: DateTime<Utc>| d.with_timezone(&style.offset.unwrap_or_else(|| *Local::now().offset()));
    let at = local(date);
    let (month, day, year) = (at.month0() as usize, at.day(), at.year());
    let date_text = |long: bool, weekday: bool| {
        let base = match (style.french, style.us, long) {
            (true, _, true) => format!("{day} {} {year}", MONTHS_FR[month]),
            (true, _, false) => format!("{day} {} {year}", MONTHS_FR_SHORT[month]),
            (false, true, true) => at.format("%B %-d, %Y").to_string(),
            (false, true, false) => at.format("%b %-d, %Y").to_string(),
            (false, false, true) => at.format("%-d %B %Y").to_string(),
            (false, false, false) => at.format("%-d %b %Y").to_string(),
        };
        match (weekday, style.french) {
            (false, _) => base,
            (true, true) => format!("{} {base}", DAYS_FR[at.weekday().num_days_from_monday() as usize]),
            (true, false) => format!("{}, {base}", at.format("%A")),
        }
    };
    let pretty = |fallback: String| {
        let days = (at.date_naive() - local(style.now).date_naive()).num_days();
        match (days, style.french) {
            (0, false) => "Today".into(),
            (1, false) => "Tomorrow".into(),
            (-1, false) => "Yesterday".into(),
            (0, true) => "aujourd’hui".into(),
            (1, true) => "demain".into(),
            (-1, true) => "hier".into(),
            _ => fallback,
        }
    };
    Some(match name {
        "time" if style.us => at.format("%-I:%M %p").to_string(),
        "time" => at.format("%H:%M").to_string(),
        "time_secs" if style.us => at.format("%-I:%M:%S %p").to_string(),
        "time_secs" => at.format("%H:%M:%S").to_string(),
        "date_num" => at.format("%Y-%m-%d").to_string(),
        "date" => date_text(true, false),
        "date_short" => date_text(false, false),
        "date_long" => date_text(true, true),
        "date_pretty" => pretty(date_text(true, false)),
        "date_short_pretty" => pretty(date_text(false, false)),
        "date_long_pretty" => pretty(date_text(true, true)),
        "ago" => ago(style.now - date, style.french),
        _ => return None,
    })
}

/// "2 hours ago", "in 5 minutes"; "il y a 2 heures", "dans 5 minutes".
fn ago(elapsed: Duration, french: bool) -> String {
    let seconds = elapsed.num_seconds();
    let n = seconds.abs();
    let (count, unit_en, unit_fr) = match n {
        0..60 => return if french { "maintenant".into() } else { "now".into() },
        60..3_600 => (n / 60, "minute", "minute"),
        3_600..86_400 => (n / 3_600, "hour", "heure"),
        _ => (n / 86_400, "day", "jour"),
    };
    let plural = if count > 1 { "s" } else { "" };
    match (french, seconds > 0) {
        (false, true) => format!("{count} {unit_en}{plural} ago"),
        (false, false) => format!("in {count} {unit_en}{plural}"),
        (true, true) => format!("il y a {count} {unit_fr}{plural}"),
        (true, false) => format!("dans {count} {unit_fr}{plural}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stub::*;

    #[test]
    fn cleans_mrkdwn() {
        let text = "<@UME> see <https://x.io|the doc> &amp; ping <@U2|erin> in <#C1|general> <!here>";
        assert_eq!(plain_text(text, "UME", &paris()), "@you see the doc & ping @erin in #general @here");
    }

    /// Ids from the server can't add parameters to the app link.
    #[test]
    fn app_links_encode_the_ids() {
        let message: Match =
            serde_json::from_str(r#"{"ts": "1.0", "channel": {"id": "D1&team=EVIL#x", "is_im": true}}"#).unwrap();
        let url = message.app_url(Some("T42")).unwrap();
        assert_eq!(url, "slack://channel?team=T42&id=D1%26team%3DEVIL%23x");
        assert!(crate::links::is_app_link(&url));
    }

    /// The event in the screenshot: 16:15 to 17:15 in Paris on 9 October 2026.
    fn start() -> DateTime<Utc> {
        Utc.timestamp_opt(1_791_555_300, 0).unwrap()
    }

    fn paris() -> DateStyle {
        DateStyle { offset: FixedOffset::east_opt(2 * 3_600), french: false, us: false, now: start() }
    }

    /// A Google Calendar reminder, as Slack sends it. Same case as macOS.
    #[test]
    fn calendar_reminder_reads_in_your_language_and_time_zone() {
        let text = ":loudspeaker: _1 minute until this event:_\n<!date^1791555300^{time}|4:15 PM> - <!date^1791558900^{time}|5:15 PM> \
                    Staging Movidone *Guests:* You _(organizer)_, <mailto:lou@acme.io|Lou Martin>";
        let french = DateStyle { french: true, ..paris() };
        assert_eq!(
            plain_text(text, "UME", &french),
            "📢 1 minute until this event:\n16:15 - 17:15 Staging Movidone Guests: You (organizer), Lou Martin"
        );
        let us = DateStyle { offset: FixedOffset::west_opt(4 * 3_600), us: true, ..paris() };
        assert!(plain_text(text, "UME", &us).contains("10:15 AM - 11:15 AM"));
    }

    #[test]
    fn date_tokens() {
        let style = DateStyle { now: start() + Duration::hours(2), ..paris() };
        let format = |f: &str| formatted(start(), f, Some("fallback"), &style);
        assert_eq!(format("{date_num} {time}"), "2026-10-09 16:15");
        assert_eq!(format("{date_short}"), "9 Oct 2026");
        assert!(format("{date_long}").starts_with("Friday"));
        assert_eq!(format("{date_pretty}"), "Today");
        assert_eq!(format("{ago}"), "2 hours ago");
        assert_eq!(format("on {nonsense}"), "fallback", "a token Slack may add later");
        assert_eq!(
            formatted(start(), "{date_long}", None, &DateStyle { french: true, ..paris() }),
            "vendredi 9 octobre 2026"
        );
        assert_eq!(plain_text("<!date^1791555300^{date_num}^https://x.io|x>", "", &style), "2026-10-09", "with a link");
        assert_eq!(
            plain_text("<tel:+33100000000|Call Lou>, <!subteam^S1|@design>, <mailto:a@b.io>", "", &style),
            "Call Lou, @design, a@b.io"
        );
    }

    /// An app's reminder is over once the event starts; a person's message never expires.
    #[tokio::test]
    async fn app_reminders_expire_when_the_event_starts() {
        let search = r#"{"ok": true, "messages": {"matches": [
          {"ts": "1791555240.0", "text": "_1 minute until this event:_ <!date^1791555300^{time}|4:15 PM> - <!date^1791558900^{time}|5:15 PM>",
           "username": "Google Calendar", "bot_id": "B1", "channel": {"id": "D7", "is_im": true}},
          {"ts": "1791555000.0", "text": "Shall we meet at <!date^1791555300^{time}|4:15 PM>?", "username": "carol", "channel": {"id": "D1", "is_im": true}}
        ]}}"#;
        let http = StubHttp::paths(&[
            ("/auth.test", r#"{"ok": true, "user_id": "UME", "user": "alice", "team": "Acme"}"#),
            ("/search.messages", search),
        ]);
        let items =
            SlackPlugin::new(&config(&[("token", "xoxp")]), Arc::new(http)).unwrap().fetch().await.unwrap().items;
        let calendar = items.iter().find(|i| i.author.as_ref().is_some_and(|a| a.name == "Google Calendar")).unwrap();
        assert_eq!(calendar.expires, Some(start()));
        assert_eq!(items.iter().find(|i| i.author.as_ref().is_some_and(|a| a.name == "carol")).unwrap().expires, None);

        let assembler = remora_core::InboxAssembler { wake_on_activity: true };
        assert_eq!(assembler.placement(calendar, None, start() - Duration::seconds(30)), remora_core::Placement::Inbox);
        assert_eq!(assembler.placement(calendar, None, start()), remora_core::Placement::Cleared);
        let pinned = remora_core::ItemState { pinned: true, ..Default::default() };
        assert_eq!(
            assembler.placement(calendar, Some(&pinned), start()),
            remora_core::Placement::Inbox,
            "pinned stays"
        );
    }

    #[tokio::test]
    async fn keeps_the_latest_message_per_conversation_with_app_links() {
        let search = r#"{"ok": true, "messages": {"matches": [
          {"ts": "1700000100.0", "text": "second", "username": "carol", "channel": {"id": "D1", "is_im": true}},
          {"ts": "1700000000.0", "text": "first", "username": "carol", "channel": {"id": "D1", "is_im": true}},
          {"ts": "1700000050.0", "text": "hey <@UME>\nmore", "username": "erin", "channel": {"id": "C1", "name": "dev"}}
        ]}}"#;
        let http = StubHttp::paths(&[
            ("/auth.test", r#"{"ok": true, "user_id": "UME", "user": "alice", "team": "Acme", "team_id": "T42"}"#),
            ("/search.messages", search),
        ]);
        let snapshot = SlackPlugin::new(&config(&[("token", "xoxp")]), Arc::new(http)).unwrap().fetch().await.unwrap();

        let direct: Vec<&InboxItem> =
            snapshot.items.iter().filter(|i| i.bundle == InboxBundle::direct_messages()).collect();
        assert_eq!(direct.iter().map(|i| i.title.as_str()).collect::<Vec<_>>(), ["second"]);
        assert_eq!(direct[0].app_url.as_deref(), Some("slack://channel?team=T42&id=D1"));
        let mention = snapshot.items.iter().find(|i| i.bundle == InboxBundle::mentions()).unwrap();
        assert_eq!(
            (mention.title.as_str(), mention.preview.as_deref(), mention.context.as_str()),
            ("hey @you", Some("more"), "#dev")
        );
    }

    /// Google Calendar's app DMs event updates with an empty `text`: the words are in attachments or blocks.
    #[tokio::test]
    async fn app_messages_are_readable_and_to_read() {
        let search = r#"{"ok": true, "messages": {"matches": [
          {"ts": "1700000300.0", "text": "", "username": "Google Calendar", "bot_id": "B1", "channel": {"id": "D7", "is_im": true},
           "attachments": [{"fallback": "Event updated", "pretext": "Event updated", "title": "Design review", "text": "Tomorrow 10:00"}]},
          {"ts": "1700000200.0", "text": "", "username": "Jira Cloud", "channel": {"id": "D8", "is_im": true},
           "blocks": [{"type": "section", "text": {"type": "mrkdwn", "text": "*PAY-12* moved to Done"}},
                      {"type": "context", "elements": [{"type": "mrkdwn", "text": "by Erin"}, {"type": "image", "image_url": "x"}]}]},
          {"ts": "1700000100.0", "username": "Mystery", "channel": {"id": "D9", "is_im": true}, "attachments": "odd"},
          {"ts": "1700000000.0", "text": "can you look?", "username": "carol", "channel": {"id": "D1", "is_im": true}}
        ]}}"#;
        let http = StubHttp::paths(&[
            ("/auth.test", r#"{"ok": true, "user_id": "UME", "user": "alice", "team": "Acme"}"#),
            ("/search.messages", search),
        ]);
        let items: Vec<InboxItem> = SlackPlugin::new(&config(&[("token", "xoxp")]), Arc::new(http))
            .unwrap()
            .fetch()
            .await
            .unwrap()
            .items
            .into_iter()
            .map(remora_core::VerbClassifier::classify)
            .collect();
        let by = |name: &str| items.iter().find(|i| i.author.as_ref().is_some_and(|a| a.name == name)).unwrap();
        let calendar = by("Google Calendar");
        assert_eq!(
            (calendar.title.as_str(), calendar.preview.as_deref()),
            ("Event updated", Some("Design review Tomorrow 10:00"))
        );
        assert_eq!(calendar.bundle, InboxBundle::read());
        assert!(!calendar.needs_action);
        let jira = by("Jira Cloud");
        assert_eq!((jira.title.as_str(), jira.preview.as_deref()), ("PAY-12 moved to Done", Some("by Erin")));
        assert_eq!(jira.bundle, InboxBundle::read());
        let empty = by("Mystery");
        assert_eq!((empty.title.as_str(), &empty.bundle), ("(no text)", &InboxBundle::read()));
        let person = by("carol");
        assert_eq!(person.bundle, InboxBundle::reply());
        assert!(person.needs_action);
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
        assert_eq!(
            json["oauth_config"]["scopes"]["user"],
            serde_json::json!(["search:read", "channels:history", "groups:history"])
        );
    }

    /// Answers Slack by method, and the searches by their query: your own thread messages, or nothing.
    struct SlackStub {
        mine: &'static str,
        replies: &'static str,
        asked: std::sync::Mutex<Vec<String>>,
    }

    #[async_trait]
    impl HttpClient for SlackStub {
        async fn send(&self, request: Request) -> Result<crate::Response, PluginError> {
            let url = reqwest::Url::parse(&request.url).unwrap();
            let value = |name: &str| {
                url.query_pairs().find(|(k, _)| k == name).map(|(_, v)| v.into_owned()).unwrap_or_default()
            };
            let body = match url.path().rsplit('/').next().unwrap_or_default() {
                "auth.test" => {
                    r#"{"ok": true, "user_id": "UME", "user": "alice", "team": "Acme", "team_id": "T1"}"#.to_string()
                }
                "conversations.replies" => {
                    self.asked.lock().unwrap().push(format!(
                        "{} {} {}",
                        value("channel"),
                        value("ts"),
                        value("oldest")
                    ));
                    self.replies.to_string()
                }
                _ if value("query").contains("is:thread") => self.mine.to_string(),
                _ => r#"{"ok": true, "messages": {"matches": []}}"#.to_string(),
            };
            Ok(crate::Response { status: 200, body })
        }
    }

    /// A reply in a thread you wrote in, without a mention, reaches the inbox; one that mentions you is
    /// already a mention, and your own or older replies don't count. Same cases as macOS.
    #[tokio::test]
    async fn replies_in_your_threads() {
        let mine = r#"{"ok": true, "messages": {"matches": [
          {"ts": "1700000200.0", "text": "I can take it", "channel": {"id": "C1", "name": "design"},
           "permalink": "https://acme.slack.com/archives/C1/p1700000200?thread_ts=1700000100.0"},
          {"ts": "1700000050.0", "text": "older one", "channel": {"id": "C1", "name": "design"},
           "permalink": "https://acme.slack.com/archives/C1/p1700000050?thread_ts=1700000100.0"},
          {"ts": "1700000300.0", "text": "in a DM", "channel": {"id": "D1", "is_im": true}}
        ]}}"#;
        let replies = r#"{"ok": true, "messages": [
          {"ts": "1700000100.0", "text": "Who can review the mockups?", "user": "U2"},
          {"ts": "1700000200.0", "text": "I can take it", "user": "UME"},
          {"ts": "1700000400.0", "text": "Great, *thanks*! Tomorrow then", "user": "U2", "user_profile": {"display_name": "erin"}},
          {"ts": "1700000500.0", "text": "<@UME> also this", "user": "U3"}
        ]}"#;
        let http = Arc::new(SlackStub { mine, replies, asked: Default::default() });
        let items = SlackPlugin::new(&config(&[("token", "xoxp")]), http.clone()).unwrap().fetch().await.unwrap().items;
        let thread = items.iter().find(|i| i.context.starts_with("Thread")).unwrap();
        assert_eq!(thread.title, "Great, thanks! Tomorrow then");
        assert_eq!(thread.author.as_ref().unwrap().name, "erin");
        assert_eq!(thread.context, "Thread in #design");
        assert!(thread.id.ends_with("/C1/thread/1700000100.0"));
        assert!(thread.bundle == InboxBundle::mentions() && thread.needs_action);
        assert_eq!(
            *http.asked.lock().unwrap(),
            ["C1 1700000100.0 1700000200.0"],
            "one call, from your last message, no DM"
        );
    }

    #[tokio::test]
    async fn threads_without_the_history_scope_say_so() {
        let mine = r#"{"ok": true, "messages": {"matches": [{"ts": "1.0", "text": "x", "channel": {"id": "C1", "name": "a"}}]}}"#;
        let http = Arc::new(SlackStub {
            mine,
            replies: r#"{"ok": false, "error": "missing_scope"}"#,
            asked: Default::default(),
        });
        let snapshot = SlackPlugin::new(&config(&[("token", "xoxp")]), http).unwrap().fetch().await.unwrap();
        assert!(snapshot.remarks.len() == 1 && snapshot.remarks[0].contains("channels:history"));
        assert!(!snapshot.items.iter().any(|i| i.context.starts_with("Thread")));
    }
}

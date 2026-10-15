use crate::linear::{due_badge, due_date};
use crate::{
    decode, url_with_query, AccountCache, HttpClient, PluginConfig, PluginError, Request, SourcePlugin, SourceSnapshot,
    IDENTITY_LIFETIME,
};
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use regex::Regex;
use remora_core::{Badge, ConfigField, Egress, InboxBundle, InboxItem, Person, PluginManifest, Priority, Tone};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock};

pub fn manifest() -> PluginManifest {
    PluginManifest {
        id: "jira".into(),
        name: "Jira".into(),
        summary: "Issues assigned to you, and comments that mention you. Jira Cloud or Data Center.".into(),
        fields: vec![
            ConfigField {
                key: "host".into(),
                label: "Host".into(),
                placeholder: "https://your-company.atlassian.net".into(),
                default_value: String::new(),
                is_secret: false,
                is_optional: false,
                help: None,
                managed_key: None,
                is_toggle: false,
            },
            ConfigField {
                key: "email".into(),
                label: "E-mail".into(),
                placeholder: "you@company.com".into(),
                default_value: String::new(),
                is_secret: false,
                is_optional: true,
                help: Some("Jira Cloud only: the e-mail you sign in with. Leave empty for Data Center.".into()),
                managed_key: None,
                is_toggle: false,
            },
            ConfigField::token("API token", "Jira Cloud: an API token. Data Center: a personal access token."),
            ConfigField::toggle(
                "confluence",
                "Mentions in Confluence",
                "Jira Cloud only: pages and comments that mention you in Confluence, with the same token.",
            ),
        ],
        setup_steps: vec![
            "Set your Jira host first, for example your-company.atlassian.net.".into(),
            "Jira Cloud: click “Create a token”, create an API token named Remora, and enter your e-mail too.".into(),
            "Data Center: in Jira, open your profile → Personal Access Tokens, create one, and leave the e-mail empty."
                .into(),
            "Paste the token below.".into(),
        ],
        setup_label: "Create a token".into(),
        setup_url: Some("https://id.atlassian.com/manage-profile/security/api-tokens".into()),
        egress: Egress {
            hosts: vec!["atlassian.net".into()],
            description: "Reads issues assigned to you and recent comments on issues you watch, from your Jira host. \
                If turned on, the Confluence pages and comments that mention you, on the same site."
                .into(),
            external_ai: false,
        },
        logo: Some("jira".into()),
        sign_in: None,
    }
}

const ASSIGNED_JQL: &str = "assignee = currentUser() AND statusCategory != Done ORDER BY updated DESC";
/// Issues you watch (Jira adds you to those you create, are assigned or comment on) with recent activity: their
/// comments are read for mentions of you.
const WATCHED_JQL: &str = "watcher = currentUser() AND updated >= -7d ORDER BY updated DESC";
/// Confluence pages and comments that mention you, changed in the last week.
const MENTION_CQL: &str = r#"mention = currentUser() AND lastmodified >= now("-7d") ORDER BY lastmodified DESC"#;

/// Issues assigned to you, and recent comments that mention you, on Jira Cloud or Data Center. On Cloud, if turned
/// on, the Confluence pages and comments that mention you too, read on the same site with the same token.
pub struct JiraPlugin {
    account_id: String,
    host: String,
    authorization: String,
    /// Cloud signs in with the e-mail and an API token, on `api/3`; Data Center with a personal access token.
    cloud: bool,
    /// Confluence lives on the same Cloud site, under `/wiki`; Data Center's is a separate server.
    confluence: bool,
    http: Arc<dyn HttpClient>,
    cache: Arc<AccountCache>,
}

impl JiraPlugin {
    pub fn new(config: &PluginConfig, http: Arc<dyn HttpClient>) -> Result<Self, PluginError> {
        let host = config.url("host")?;
        let token = config.required("token")?;
        let email = config.get("email");
        let cloud = !email.is_empty();
        let on_atlassian =
            reqwest::Url::parse(&host).ok().and_then(|u| u.host_str().map(|h| h.ends_with(".atlassian.net")));
        if on_atlassian == Some(true) && !cloud {
            return Err(PluginError::MissingField("email".into()));
        }
        let authorization = if cloud {
            format!("Basic {}", base64(format!("{email}:{token}").as_bytes()))
        } else {
            format!("Bearer {token}")
        };
        Ok(JiraPlugin {
            account_id: config.account_id.clone(),
            host,
            authorization,
            cloud,
            confluence: cloud && config.get("confluence") == "true",
            http,
            cache: config.cache.clone(),
        })
    }

    async fn get<T: DeserializeOwned>(&self, path: &str, query: &[(&str, &str)]) -> Result<T, PluginError> {
        let version = if self.cloud { 3 } else { 2 };
        let url = url_with_query(&self.host, &format!("rest/api/{version}/{path}"), query)?;
        let request =
            Request::get(url).header("Authorization", self.authorization.clone()).header("Accept", "application/json");
        decode(self.http.as_ref(), request).await
    }

    /// Cloud's search is `search/jql` (the older `search` is gone there); Data Center keeps `api/2/search`.
    async fn search(&self, jql: &str, fields: &str, max: &str) -> Result<SearchPage, PluginError> {
        let path = if self.cloud { "search/jql" } else { "search" };
        self.get(path, &[("jql", jql), ("fields", fields), ("maxResults", max)]).await
    }

    /// Confluence's search, on the same site: CQL finds what mentions you, pages and comments alike.
    async fn confluence_mentions(&self) -> Result<ConfluencePage, PluginError> {
        let query =
            [("cql", MENTION_CQL), ("limit", "25"), ("expand", "content.history.lastUpdated,content.container")];
        let url = url_with_query(&self.host, "wiki/rest/api/search", &query)?;
        let request =
            Request::get(url).header("Authorization", self.authorization.clone()).header("Accept", "application/json");
        decode(self.http.as_ref(), request).await
    }

    pub fn snapshot(
        me: &Myself,
        assigned: &SearchPage,
        watched: &SearchPage,
        confluence: &ConfluencePage,
        host: &str,
        account_id: &str,
        now: DateTime<Utc>,
    ) -> SourceSnapshot {
        let mut items: Vec<InboxItem> = assigned.issues.iter().map(|i| i.item(host, account_id)).collect();
        let week_ago = now - Duration::days(7);
        for issue in &watched.issues {
            let comments = issue.fields.comment.as_ref().map(|c| c.comments.as_slice()).unwrap_or_default();
            items.extend(
                comments
                    .iter()
                    .filter(|c| {
                        jira_date(c.created.as_deref()).is_some_and(|d| d > week_ago)
                            && !me.is_author(c)
                            && c.body.mentions(me)
                    })
                    .map(|c| c.item(issue, host, account_id)),
            );
        }
        // Not the pages you changed last: you have seen those.
        items.extend(
            confluence
                .results
                .iter()
                .filter(|r| {
                    me.account_id.is_none()
                        || r.last_editor().and_then(|b| b.account_id.as_ref()) != me.account_id.as_ref()
                })
                .filter_map(|r| r.item(host, account_id)),
        );
        SourceSnapshot {
            identity: me.display_name.clone(),
            items,
            remarks: if assigned.is_cut() { vec![crate::truncated("Jira")] } else { vec![] },
        }
    }
}

#[async_trait]
impl SourcePlugin for JiraPlugin {
    async fn fetch(&self) -> Result<SourceSnapshot, PluginError> {
        let me: Myself = self.cache.value("myself", IDENTITY_LIFETIME, || self.get("myself", &[])).await?;
        let (assigned, watched) = futures::join!(
            self.search(ASSIGNED_JQL, "summary,priority,duedate,status,updated", "50"),
            self.search(WATCHED_JQL, "summary,comment", "25"),
        );
        // Mentions are a bonus: if that search fails, assigned issues still come through, and a remark says so.
        let (watched, failed) = match watched {
            Ok(page) => (page, None),
            Err(error) => match crate::rate_limit_of(&error) {
                Some(limit) => return Err(limit),
                None => (SearchPage::default(), Some(crate::partial("Mentions and comments", &error))),
            },
        };
        let mut pages = ConfluencePage::default();
        let mut confluence_failed = None;
        if self.confluence {
            match self.confluence_mentions().await {
                Ok(page) => pages = page,
                Err(error) => match crate::rate_limit_of(&error) {
                    Some(limit) => return Err(limit),
                    None => confluence_failed = Some(crate::partial("Confluence mentions", &error)),
                },
            }
        }
        let mut snapshot = Self::snapshot(&me, &assigned?, &watched, &pages, &self.host, &self.account_id, Utc::now());
        snapshot.remarks.extend(failed);
        snapshot.remarks.extend(confluence_failed);
        Ok(snapshot)
    }
}

/// "2026-10-12T09:14:03.123+0200", Jira's timestamps: ISO 8601 without the colon in the offset.
pub fn jira_date(raw: Option<&str>) -> Option<DateTime<Utc>> {
    let raw = raw?;
    DateTime::parse_from_rfc3339(raw)
        .or_else(|_| DateTime::parse_from_str(raw, "%Y-%m-%dT%H:%M:%S%.f%z"))
        .ok()
        .map(|d| d.with_timezone(&Utc))
}

/// Standard base64, for the Basic header: one use, not worth a dependency.
pub(crate) fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Myself {
    /// Cloud's id; mentions carry it.
    account_id: Option<String>,
    /// Data Center's user name; mentions are `[~name]`.
    name: Option<String>,
    key: Option<String>,
    display_name: String,
}

impl Myself {
    fn is_author(&self, comment: &Comment) -> bool {
        let author = comment.author.as_ref();
        (self.account_id.is_some() && author.and_then(|a| a.account_id.as_ref()) == self.account_id.as_ref())
            || (self.name.is_some() && author.and_then(|a| a.name.as_ref()) == self.name.as_ref())
    }
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchPage {
    #[serde(default)]
    issues: Vec<Issue>,
    /// Cloud: set when there is more.
    next_page_token: Option<String>,
    /// Data Center: how many match in all.
    total: Option<usize>,
}

impl SearchPage {
    fn is_cut(&self) -> bool {
        self.next_page_token.is_some() || self.total.is_some_and(|t| t > self.issues.len())
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct User {
    account_id: Option<String>,
    name: Option<String>,
    display_name: Option<String>,
    avatar_urls: Option<HashMap<String, String>>,
}

#[derive(Deserialize)]
struct Named {
    name: String,
}

#[derive(Deserialize)]
struct Comments {
    comments: Vec<Comment>,
}

#[derive(Deserialize)]
struct Fields {
    summary: Option<String>,
    priority: Option<Named>,
    duedate: Option<String>,
    status: Option<Named>,
    updated: Option<String>,
    comment: Option<Comments>,
}

#[derive(Deserialize)]
struct Issue {
    id: String,
    key: String,
    fields: Fields,
}

impl Issue {
    fn url(&self, host: &str) -> String {
        format!("{}/browse/{}", host.trim_end_matches('/'), self.key)
    }

    fn summary(&self) -> String {
        remora_core::text::readable(self.fields.summary.as_deref().unwrap_or(&self.key))
    }

    /// Jira's default priorities by name (Highest … Lowest), and the older Blocker … Trivial.
    fn item_priority(&self) -> Priority {
        match self.fields.priority.as_ref().map(|p| p.name.to_lowercase()).as_deref() {
            Some("highest" | "blocker" | "critical") => Priority::Urgent,
            Some("high" | "major") => Priority::High,
            Some("low" | "lowest" | "minor" | "trivial") => Priority::Low,
            _ => Priority::Normal,
        }
    }

    fn item(&self, host: &str, account_id: &str) -> InboxItem {
        let priority = self.item_priority();
        let mut badges = vec![];
        match priority {
            Priority::Urgent => {
                badges.push(Badge::new("priority.urgent", "Urgent", Tone::Negative).notifying("Urgent issue"))
            }
            Priority::High => badges.push(Badge::new("priority.high", "High", Tone::Warning)),
            _ => {}
        }
        let due = self.fields.duedate.as_deref().and_then(due_date);
        if let Some(due) = due {
            badges.push(due_badge(due));
        }
        if let Some(status) = &self.fields.status {
            badges.push(Badge::new("status", &status.name, Tone::Neutral));
        }
        InboxItem {
            id: format!("{account_id}/{}", self.id),
            account_id: account_id.into(),
            plugin_id: "jira".into(),
            bundle: InboxBundle::tasks(),
            title: self.summary(),
            context: self.key.clone(),
            preview: None,
            url: Some(self.url(host)),
            app_url: None,
            author: None,
            participants: vec![],
            badges,
            date: jira_date(self.fields.updated.as_deref()).unwrap_or_else(Utc::now),
            needs_action: priority != Priority::Low,
            priority: Some(priority),
            due,
            expires: None,
            changes: None,
            suggested_people: None,
            recurrence: None,
            reactions: None,
        }
    }
}

#[derive(Deserialize)]
struct Comment {
    id: String,
    author: Option<User>,
    body: Body,
    created: Option<String>,
}

impl Comment {
    fn item(&self, issue: &Issue, host: &str, account_id: &str) -> InboxItem {
        let text = self.body.text();
        InboxItem {
            id: format!("{account_id}/comment/{}", self.id),
            account_id: account_id.into(),
            plugin_id: "jira".into(),
            bundle: InboxBundle::mentions(),
            title: issue.summary(),
            context: issue.key.clone(),
            preview: (!text.is_empty()).then(|| remora_core::text::readable(&text)),
            url: Some(format!("{}?focusedCommentId={}", issue.url(host), self.id)),
            app_url: None,
            author: self.author.as_ref().and_then(|a| {
                a.display_name.as_ref().map(|name| Person {
                    name: name.clone(),
                    avatar_url: a.avatar_urls.as_ref().and_then(|u| u.get("48x48").cloned()),
                    tone: None,
                })
            }),
            participants: vec![],
            badges: vec![],
            date: jira_date(self.created.as_deref()).unwrap_or_else(Utc::now),
            needs_action: true,
            priority: None,
            due: None,
            expires: None,
            changes: None,
            suggested_people: None,
            recurrence: None,
            reactions: None,
        }
    }
}

/// A comment's body: Atlassian Document Format on Cloud (`api/3`), wiki markup on Data Center (`api/2`).
#[derive(Deserialize)]
#[serde(untagged)]
enum Body {
    Wiki(String),
    Document(Node),
}

#[derive(Default, Deserialize)]
struct Attributes {
    id: Option<String>,
    text: Option<String>,
}

#[derive(Deserialize)]
struct Node {
    #[serde(rename = "type")]
    kind: Option<String>,
    text: Option<String>,
    attrs: Option<Attributes>,
    #[serde(default)]
    content: Vec<Node>,
}

impl Node {
    fn mention_ids(&self, ids: &mut Vec<String>) {
        if self.kind.as_deref() == Some("mention") {
            ids.extend(self.attrs.as_ref().and_then(|a| a.id.clone()));
        }
        self.content.iter().for_each(|n| n.mention_ids(ids));
    }

    fn plain_text(&self) -> String {
        if self.kind.as_deref() == Some("mention") {
            return self.attrs.as_ref().and_then(|a| a.text.clone()).unwrap_or_default();
        }
        if let Some(text) = &self.text {
            return text.clone();
        }
        let inner: String = self.content.iter().map(Node::plain_text).collect();
        if self.kind.as_deref() == Some("paragraph") {
            inner + "\n"
        } else {
            inner
        }
    }
}

/// `[~alice]` or `[~accountid:…]`, a mention in wiki markup.
static WIKI_MENTION: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[~(?:accountid:)?([^\]]+)\]").unwrap());

impl Body {
    fn mentions(&self, me: &Myself) -> bool {
        match self {
            Body::Document(node) => {
                let mut ids = vec![];
                node.mention_ids(&mut ids);
                me.account_id.as_ref().is_some_and(|id| ids.contains(id))
            }
            Body::Wiki(text) => {
                let names =
                    [me.account_id.as_ref().map(|id| format!("accountid:{id}")), me.name.clone(), me.key.clone()];
                names.iter().flatten().any(|name| text.contains(&format!("[~{name}]")))
            }
        }
    }

    fn text(&self) -> String {
        match self {
            Body::Document(node) => node.plain_text().trim().to_string(),
            Body::Wiki(text) => WIKI_MENTION.replace_all(text, "@$1").trim().to_string(),
        }
    }
}

/// Confluence's search (`wiki/rest/api/search`): pages and comments, each with its content.
#[derive(Default, Deserialize)]
pub struct ConfluencePage {
    #[serde(default)]
    results: Vec<ConfluenceResult>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConfluenceResult {
    content: Option<Content>,
    excerpt: Option<String>,
    last_modified: Option<String>,
    result_global_container: Option<Titled>,
}

#[derive(Deserialize)]
struct Titled {
    title: Option<String>,
}

#[derive(Deserialize)]
struct Content {
    id: String,
    #[serde(rename = "type")]
    kind: Option<String>,
    title: Option<String>,
    history: Option<History>,
    /// A comment's page.
    container: Option<Titled>,
    #[serde(rename = "_links")]
    links: Option<Links>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct History {
    last_updated: Option<Version>,
}

#[derive(Deserialize)]
struct Version {
    by: Option<Editor>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Editor {
    account_id: Option<String>,
    display_name: Option<String>,
    profile_picture: Option<Picture>,
}

#[derive(Deserialize)]
struct Picture {
    path: Option<String>,
}

#[derive(Deserialize)]
struct Links {
    webui: Option<String>,
}

impl ConfluenceResult {
    fn last_editor(&self) -> Option<&Editor> {
        self.content.as_ref()?.history.as_ref()?.last_updated.as_ref()?.by.as_ref()
    }

    /// A mention, the page's title first: the verb classifier reads it with the excerpt.
    fn item(&self, host: &str, account_id: &str) -> Option<InboxItem> {
        let content = self.content.as_ref()?;
        let link = content.links.as_ref()?.webui.as_ref()?;
        let site = host.trim_end_matches('/');
        let page_title = if content.kind.as_deref() == Some("comment") {
            content.container.as_ref().and_then(|c| c.title.as_deref())
        } else {
            None
        };
        let title = page_title.or(content.title.as_deref()).unwrap_or_default();
        let excerpt = confluence_excerpt(self.excerpt.as_deref().unwrap_or_default());
        let by = self.last_editor();
        Some(InboxItem {
            id: format!("{account_id}/confluence/{}", content.id),
            account_id: account_id.into(),
            plugin_id: "jira".into(),
            bundle: InboxBundle::mentions(),
            title: remora_core::text::readable(title),
            context: self.result_global_container.as_ref().and_then(|c| c.title.clone()).unwrap_or("Confluence".into()),
            preview: (!excerpt.is_empty()).then_some(excerpt),
            url: Some(format!("{site}/wiki{link}")),
            app_url: None,
            author: by.and_then(|b| {
                b.display_name.as_ref().map(|name| Person {
                    name: name.clone(),
                    // A profile picture's path is on the site: `/wiki/aa-avatar/…`.
                    avatar_url: b.profile_picture.as_ref().and_then(|p| p.path.as_ref()).map(|path| {
                        if path.starts_with('/') {
                            format!("{site}{path}")
                        } else {
                            path.clone()
                        }
                    }),
                    tone: None,
                })
            }),
            participants: vec![],
            badges: vec![],
            date: jira_date(self.last_modified.as_deref()).unwrap_or_else(Utc::now),
            needs_action: true,
            priority: None,
            due: None,
            expires: None,
            changes: None,
            suggested_people: None,
            recurrence: None,
            reactions: None,
        })
    }
}

/// A search excerpt as text: without its `@@@hl@@@` highlight marks and HTML.
fn confluence_excerpt(raw: &str) -> String {
    crate::microsoft::plain(&raw.replace("@@@hl@@@", "").replace("@@@endhl@@@", ""))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stub::config;
    use crate::Response;

    /// Answers Jira by path (who you are) or by the search's JQL, and checks the Authorization header.
    struct JiraStub {
        myself: String,
        assigned: String,
        watched: String,
        authorization: String,
    }

    /// Confluence's search for `mention = currentUser()`: a page, a comment on it, and a page you changed last.
    const CONFLUENCE: &str = r#"{"results": [
          {"content": {"id": "98304", "type": "page", "title": "Q4 launch plan",
            "history": {"lastUpdated": {"by": {"accountId": "erin", "displayName": "Erin", "profilePicture": {"path": "/wiki/aa-avatar/erin"}}}},
            "_links": {"webui": "/spaces/ENG/pages/98304/Q4+launch+plan"}},
           "excerpt": "@@@hl@@@@Alice@@@endhl@@@ can you review this page?", "lastModified": "2026-10-12T09:14:03.000Z",
           "resultGlobalContainer": {"title": "Engineering"}},
          {"content": {"id": "98400", "type": "comment", "title": "Re: Q4 launch plan", "container": {"title": "Q4 launch plan"},
            "history": {"lastUpdated": {"by": {"accountId": "frank", "displayName": "Frank"}}},
            "_links": {"webui": "/spaces/ENG/pages/98304/Q4+launch+plan?focusedCommentId=98400#comment-98400"}},
           "excerpt": "FYI @Alice, the dates moved &amp; nothing to do", "lastModified": "2026-10-12T10:00:00.000Z",
           "resultGlobalContainer": {"title": "Engineering"}},
          {"content": {"id": "98500", "type": "page", "title": "My notes",
            "history": {"lastUpdated": {"by": {"accountId": "alice-id", "displayName": "Alice"}}},
            "_links": {"webui": "/spaces/~alice/pages/98500"}},
           "excerpt": "@Alice to do", "lastModified": "2026-10-12T11:00:00.000Z", "resultGlobalContainer": {"title": "Alice"}}
        ]}"#;

    #[async_trait]
    impl HttpClient for JiraStub {
        async fn send(&self, request: Request) -> Result<Response, PluginError> {
            let auth = request.headers.iter().find(|(k, _)| k == "Authorization").map(|(_, v)| v.clone());
            assert_eq!(auth.as_deref(), Some(self.authorization.as_str()));
            let url = reqwest::Url::parse(&request.url).unwrap();
            let jql = url.query_pairs().find(|(k, _)| k == "jql").map(|(_, v)| v.into_owned()).unwrap_or_default();
            let body: &str = if url.path().ends_with("/wiki/rest/api/search") {
                CONFLUENCE
            } else if url.path().ends_with("/myself") {
                &self.myself
            } else if jql.starts_with("watcher") {
                &self.watched
            } else {
                &self.assigned
            };
            Ok(Response { status: 200, body: body.to_string(), etag: None, reset: None })
        }
    }

    fn jira(date: DateTime<Utc>) -> String {
        date.with_timezone(&chrono::FixedOffset::east_opt(2 * 3600).unwrap())
            .format("%Y-%m-%dT%H:%M:%S%.3f%z")
            .to_string()
    }

    fn assigned(recent: &str) -> String {
        format!(
            r#"{{"issues": [
              {{"id": "10001", "key": "OPS-7", "fields": {{"summary": "Rotate the TLS certificates", "priority": {{"name": "Highest"}},
               "duedate": "2020-01-01", "status": {{"name": "In Progress"}}, "updated": "{recent}"}}}},
              {{"id": "10002", "key": "OPS-8", "fields": {{"summary": "Tidy the runbook", "priority": {{"name": "Low"}},
               "status": {{"name": "To Do"}}, "updated": "{recent}"}}}}
            ], "isLast": true}}"#
        )
    }

    fn watched(recent: &str, old: &str) -> String {
        format!(
            r#"{{"issues": [{{"id": "10003", "key": "OPS-9", "fields": {{"summary": "Pager rota", "comment": {{"comments": [
              {{"id": "501", "author": {{"accountId": "erin", "displayName": "Erin", "avatarUrls": {{"48x48": "https://secure.gravatar.com/avatar/e"}}}},
               "created": "{recent}", "body": {{"type": "doc", "content": [{{"type": "paragraph", "content": [
                 {{"type": "mention", "attrs": {{"id": "alice-id", "text": "@Alice"}}}}, {{"type": "text", "text": " can you take Friday?"}}]}}]}}}},
              {{"id": "502", "author": {{"accountId": "erin", "displayName": "Erin"}}, "created": "{old}",
               "body": {{"type": "doc", "content": [{{"type": "mention", "attrs": {{"id": "alice-id"}}}}]}}}},
              {{"id": "503", "author": {{"accountId": "erin", "displayName": "Erin"}}, "created": "{recent}",
               "body": {{"type": "doc", "content": [{{"type": "text", "text": "No mention here"}}]}}}},
              {{"id": "504", "author": {{"accountId": "alice-id", "displayName": "Alice"}}, "created": "{recent}",
               "body": {{"type": "doc", "content": [{{"type": "mention", "attrs": {{"id": "alice-id"}}}}]}}}}
            ]}}}}}}]}}"#
        )
    }

    fn cloud(assigned_body: Option<String>) -> JiraPlugin {
        cloud_with(assigned_body, "")
    }

    fn cloud_with(assigned_body: Option<String>, confluence: &str) -> JiraPlugin {
        let now = Utc::now();
        let (recent, old) = (jira(now - Duration::hours(1)), jira(now - Duration::days(10)));
        let stub = JiraStub {
            myself: r#"{"accountId": "alice-id", "displayName": "Alice"}"#.into(),
            assigned: assigned_body.unwrap_or_else(|| assigned(&recent)),
            watched: watched(&recent, &old),
            authorization: format!("Basic {}", base64(b"alice@acme.com:t0ken")),
        };
        JiraPlugin::new(
            &config(&[
                ("host", "acme.atlassian.net"),
                ("email", "alice@acme.com"),
                ("token", "t0ken"),
                ("confluence", confluence),
            ]),
            Arc::new(stub),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn assigned_issues_are_tasks_with_priority_and_due_date() {
        let snapshot = cloud(None).fetch().await.unwrap();
        assert_eq!(snapshot.identity, "Alice");
        let tasks: Vec<_> = snapshot.items.iter().filter(|i| i.bundle == InboxBundle::tasks()).collect();
        assert_eq!(tasks.iter().map(|i| i.context.as_str()).collect::<Vec<_>>(), ["OPS-7", "OPS-8"]);
        assert_eq!(tasks[0].priority, Some(Priority::Urgent));
        assert!(tasks[0].needs_action && tasks[0].badges.iter().any(|b| b.id == "overdue"));
        assert_eq!(tasks[0].url.as_deref(), Some("https://acme.atlassian.net/browse/OPS-7"));
        assert!(
            tasks[1].priority == Some(Priority::Low) && !tasks[1].needs_action,
            "low priority is shown, not counted"
        );
    }

    /// Recent comments from someone else that mention you; not old ones, not yours, not those without a mention.
    #[tokio::test]
    async fn comments_that_mention_you_are_mentions() {
        let snapshot = cloud(None).fetch().await.unwrap();
        let mentions: Vec<_> = snapshot.items.iter().filter(|i| i.bundle == InboxBundle::mentions()).collect();
        assert_eq!(mentions.len(), 1);
        assert_eq!((mentions[0].title.as_str(), mentions[0].context.as_str()), ("Pager rota", "OPS-9"));
        assert_eq!(mentions[0].preview.as_deref(), Some("@Alice can you take Friday?"));
        assert_eq!(mentions[0].author.as_ref().map(|a| a.name.as_str()), Some("Erin"));
        assert_eq!(mentions[0].url.as_deref(), Some("https://acme.atlassian.net/browse/OPS-9?focusedCommentId=501"));
    }

    /// Turned on: Confluence pages and comments that mention you, not those you changed last, then sorted by verb.
    #[tokio::test]
    async fn confluence_mentions_are_mentions_when_turned_on() {
        let off = cloud(None).fetch().await.unwrap().items;
        assert!(!off.iter().any(|i| i.id.contains("/confluence/")), "off by default");
        let items = cloud_with(None, "true").fetch().await.unwrap().items;
        let pages: Vec<_> = items.iter().filter(|i| i.id.contains("/confluence/")).collect();
        assert_eq!(pages.iter().map(|i| i.title.as_str()).collect::<Vec<_>>(), ["Q4 launch plan", "Q4 launch plan"]);
        let page = pages[0];
        assert!(page.bundle == InboxBundle::mentions() && page.context == "Engineering");
        assert_eq!(page.preview.as_deref(), Some("@Alice can you review this page?"));
        let author = page.author.as_ref().unwrap();
        assert_eq!(author.name, "Erin");
        assert_eq!(author.avatar_url.as_deref(), Some("https://acme.atlassian.net/wiki/aa-avatar/erin"));
        assert_eq!(page.url.as_deref(), Some("https://acme.atlassian.net/wiki/spaces/ENG/pages/98304/Q4+launch+plan"));
        assert_eq!(page.date, jira_date(Some("2026-10-12T09:14:03.000Z")).unwrap());
        let comment = pages[1];
        assert_eq!(comment.preview.as_deref(), Some("FYI @Alice, the dates moved & nothing to do"));
        assert_eq!(
            comment.url.as_deref(),
            Some("https://acme.atlassian.net/wiki/spaces/ENG/pages/98304/Q4+launch+plan?focusedCommentId=98400#comment-98400")
        );
        use remora_core::VerbClassifier;
        assert_eq!(VerbClassifier::classify(page.clone()).bundle, InboxBundle::reply());
        assert_eq!(VerbClassifier::classify(comment.clone()).bundle, InboxBundle::read());
    }

    #[tokio::test]
    async fn says_when_issues_are_cut() {
        let recent = jira(Utc::now());
        let more = assigned(&recent).replace(r#""isLast": true"#, r#""nextPageToken": "abc""#);
        assert_eq!(cloud(Some(more)).fetch().await.unwrap().remarks, [crate::truncated("Jira")]);
        assert!(cloud(None).fetch().await.unwrap().remarks.is_empty());
    }

    /// Data Center: a personal access token as Bearer, `api/2`, wiki-markup comments with `[~name]`.
    #[tokio::test]
    async fn data_center_uses_a_bearer_token_and_wiki_mentions() {
        let recent = jira(Utc::now() - Duration::hours(1));
        let watched = format!(
            r#"{{"issues": [{{"id": "1", "key": "IT-3", "fields": {{"summary": "VPN", "comment": {{"comments": [
              {{"id": "9", "author": {{"name": "erin", "displayName": "Erin"}}, "created": "{recent}", "body": "[~alice] please check"}}
            ]}}}}}}], "total": 1}}"#
        );
        let stub = JiraStub {
            myself: r#"{"name": "alice", "key": "JIRAUSER1", "displayName": "Alice"}"#.into(),
            assigned: r#"{"issues": [], "total": 0}"#.into(),
            watched,
            authorization: "Bearer pat".into(),
        };
        // Data Center's Confluence is another server: the option is for Cloud.
        let plugin = JiraPlugin::new(
            &config(&[("host", "jira.acme.io"), ("token", "pat"), ("confluence", "true")]),
            Arc::new(stub),
        )
        .unwrap();
        let items = plugin.fetch().await.unwrap().items;
        assert_eq!(items.iter().map(|i| i.context.as_str()).collect::<Vec<_>>(), ["IT-3"]);
        assert_eq!(items[0].preview.as_deref(), Some("@alice please check"));
    }

    /// Only the declared hosts, plus the account's own: `acme.atlassian.net` falls under `atlassian.net`.
    #[tokio::test]
    async fn uses_only_its_declared_hosts() {
        let now = Utc::now();
        let (recent, old) = (jira(now - Duration::hours(1)), jira(now - Duration::days(10)));
        let stub = JiraStub {
            myself: r#"{"accountId": "alice-id", "displayName": "Alice"}"#.into(),
            assigned: assigned(&recent),
            watched: watched(&recent, &old),
            authorization: format!("Basic {}", base64(b"alice@acme.com:t0ken")),
        };
        let guarded = crate::GuardedHttpClient::new(Arc::new(stub), manifest().egress.hosts);
        let plugin = JiraPlugin::new(
            &config(&[
                ("host", "acme.atlassian.net"),
                ("email", "alice@acme.com"),
                ("token", "t0ken"),
                ("confluence", "true"),
            ]),
            Arc::new(guarded),
        )
        .unwrap();
        let snapshot = plugin.fetch().await.unwrap();
        assert!(snapshot.items.len() == 5 && snapshot.remarks.is_empty(), "Confluence is on the same site");
    }

    #[test]
    fn jira_cloud_needs_the_email() {
        let plugin = JiraPlugin::new(
            &config(&[("host", "acme.atlassian.net"), ("token", "t")]),
            Arc::new(crate::stub::StubHttp::paths(&[])),
        );
        assert_eq!(plugin.err(), Some(PluginError::MissingField("email".into())));
    }

    #[test]
    fn reads_jira_timestamps_and_encodes_base64() {
        assert_eq!(
            jira_date(Some("2026-10-12T09:14:03.123+0200")),
            DateTime::parse_from_rfc3339("2026-10-12T07:14:03.123Z").ok().map(|d| d.with_timezone(&Utc))
        );
        assert!(jira_date(Some("2026-10-12T09:14:03Z")).is_some());
        assert_eq!(jira_date(None), None);
        assert_eq!(base64(b"alice@acme.com:t0ken"), "YWxpY2VAYWNtZS5jb206dDBrZW4=");
        assert_eq!((base64(b"a"), base64(b"ab"), base64(b"abc")), ("YQ==".into(), "YWI=".into(), "YWJj".into()));
    }
}

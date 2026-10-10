use crate::linear::{due_badge, due_date};
use crate::{decode, url_with_query, HttpClient, PluginConfig, PluginError, Request, SourcePlugin, SourceSnapshot};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures::future::try_join_all;
use remora_core::{Badge, ConfigField, Egress, InboxBundle, InboxItem, Person, PluginManifest, Tone};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

fn field(key: &str, label: &str, placeholder: &str, default_value: &str, is_optional: bool, help: Option<&str>) -> ConfigField {
    ConfigField {
        key: key.into(), label: label.into(), placeholder: placeholder.into(), default_value: default_value.into(),
        is_secret: false, is_optional, help: help.map(Into::into),
    }
}

pub fn manifest() -> PluginManifest {
    PluginManifest {
        id: "notion".into(),
        name: "Notion".into(),
        summary: "Open tasks assigned to you in the databases you choose.".into(),
        fields: vec![
            ConfigField::token("Personal access token", "A personal access token, or the secret of an internal connection."),
            field("databases", "Task databases", "Database links, comma separated", "", false, Some("Open the database as a full page, then ••• → Copy link.")),
            field("assignee", "Assignee property", "", "Assignee", false, Some("The people property that says who a task is for.")),
            field("doneValues", "Done statuses", "", "Done, Complete, Archived", false, None),
            field("email", "Your Notion email", "you@company.com", "", true, Some("Only needed with an internal connection, to find your tasks.")),
        ],
        setup_steps: vec![
            "Click “Create a token”, then “New token”. Name it Remora, keep the “Notion API” capability, pick an expiration and create it.".into(),
            "Copy the token (Notion shows it only once) and paste it below.".into(),
            "Paste the links of your task databases. No sharing needed: the token sees what you see.".into(),
            "No “New token” button? Your workspace limits tokens to owners: ask one, or use an internal connection’s secret.".into(),
        ],
        setup_label: "Create a token".into(),
        setup_url: Some("https://www.notion.so/developers/tokens".into()),
        egress: Egress {
            hosts: vec!["api.notion.com".into()],
            description: "Reads open tasks from the Notion databases you choose.".into(),
            external_ai: false,
        },
        logo: Some("notion".into()),
    }
}

/// Without knowing who you are, the query would return everyone's tasks: better say so. Same text on macOS.
pub const UNKNOWN_USER: &str = "Remora can’t tell which Notion user you are: add your Notion email to this account.";
/// An internal connection's email that matches no one in the workspace. Same text on macOS.
pub const NO_USER: &str = "No Notion user with the email %@.";

const API: &str = "https://api.notion.com/v1";

/// Open tasks assigned to you in the Notion databases you pick.
/// Notion's public API has no notifications endpoint, so tasks are the actionable signal.
pub struct NotionPlugin {
    account_id: String,
    token: String,
    databases: Vec<String>,
    email: String,
    assignee: String,
    done_values: HashSet<String>,
    http: Arc<dyn HttpClient>,
}

impl NotionPlugin {
    pub fn new(config: &PluginConfig, http: Arc<dyn HttpClient>) -> Result<Self, PluginError> {
        let token = config.required("token")?;
        let databases = config.required("databases")?.split(',').filter(|s| !s.is_empty()).map(Self::database_id).collect();
        let assignee = Some(config.get("assignee")).filter(|s| !s.is_empty()).unwrap_or_else(|| "Assignee".into());
        let done_values = config.get("doneValues").split(',').filter(|s| !s.is_empty()).map(|s| s.trim().to_lowercase()).collect();
        Ok(NotionPlugin { account_id: config.account_id.clone(), token, databases, email: config.get("email").to_lowercase(), assignee, done_values, http })
    }

    /// Accepts a raw ID or a notion.so link and returns the 32-character ID.
    pub fn database_id(raw: &str) -> String {
        let trimmed = raw.trim();
        let candidate = trimmed.split('?').next().unwrap_or(trimmed);
        let hex: String = candidate.chars().filter(|c| *c != '-').collect();
        let tail: String = hex.chars().rev().take_while(char::is_ascii_hexdigit).collect::<Vec<_>>().into_iter().rev().collect();
        if tail.len() >= 32 { tail[tail.len() - 32..].to_string() } else { trimmed.to_string() }
    }

    async fn send<T: DeserializeOwned>(&self, request: Request) -> Result<T, PluginError> {
        let request = request.header("Authorization", format!("Bearer {}", self.token)).header("Notion-Version", "2025-09-03");
        decode(self.http.as_ref(), request).await
    }

    /// The database's tasks for you, and whether it has more than one page of them.
    async fn tasks(&self, database_id: &str, user: &str) -> Result<(Vec<InboxItem>, bool), PluginError> {
        let database: Database = self.send(Request::get(format!("{API}/databases/{database_id}"))).await?;
        let query = Query {
            filter: Some(Filter { property: self.assignee.clone(), people: Contains { contains: user.into() } }),
            sorts: vec![Sort { timestamp: "last_edited_time".into(), direction: "descending".into() }],
            page_size: 50,
        };
        let mut pages: Vec<Results<Page>> = vec![];
        for chunk in database.data_sources.chunks(2) {
            let requests = chunk.iter().map(|source| async {
                self.send::<Results<Page>>(Request::post_json(format!("{API}/data_sources/{}/query", source.id), &query)?).await
            });
            pages.extend(try_join_all(requests).await?);
        }
        let name = database.name();
        let more = pages.iter().any(|p| p.next_cursor.is_some());
        let items = pages.iter().flat_map(|p| &p.results).filter(|p| !p.is_done(&self.done_values)).map(|p| p.item(&self.account_id, &name)).collect();
        Ok((items, more))
    }

    /// A personal access token is the user itself; an internal connection needs the email to find them.
    async fn current_user(&self) -> Result<Option<(String, String)>, PluginError> {
        let me: User = self.send(Request::get(format!("{API}/users/me"))).await?;
        if me.kind.as_deref() == Some("person") {
            let label = me.name.clone().or_else(|| me.person.as_ref().and_then(|p| p.email.clone())).unwrap_or_else(|| "Notion".into());
            return Ok(Some((me.id, label)));
        }
        if let Some(owner) = me.bot.and_then(|b| b.owner).and_then(|o| o.user).filter(|u| u.kind.as_deref() == Some("person")) {
            return Ok(Some((owner.id, owner.name.unwrap_or_else(|| "Notion".into()))));
        }
        if self.email.is_empty() {
            return Ok(None);
        }
        Ok(Some((self.user_id(&self.email).await?, self.email.clone())))
    }

    async fn user_id(&self, email: &str) -> Result<String, PluginError> {
        let mut cursor: Option<String> = None;
        loop {
            let mut query = vec![("page_size", "100")];
            if let Some(cursor) = cursor.as_deref() {
                query.push(("start_cursor", cursor));
            }
            let page: Results<User> = self.send(Request::get(url_with_query(API, "users", &query))).await?;
            if let Some(user) = page.results.iter().find(|u| u.person.as_ref().and_then(|p| p.email.as_deref()).map(str::to_lowercase).as_deref() == Some(email)) {
                return Ok(user.id.clone());
            }
            match page.next_cursor {
                Some(next) => cursor = Some(next),
                None => return Err(PluginError::Api(NO_USER.replace("%@", email))),
            }
        }
    }
}

#[async_trait]
impl SourcePlugin for NotionPlugin {
    async fn fetch(&self) -> Result<SourceSnapshot, PluginError> {
        let Some((me, label)) = self.current_user().await? else {
            return Err(PluginError::Api(UNKNOWN_USER.into()));
        };
        let mut found = vec![];
        for chunk in self.databases.chunks(4) {
            found.extend(try_join_all(chunk.iter().map(|id| self.tasks(id, &me))).await?);
        }
        let cut = found.iter().any(|(_, more)| *more);
        let items = found.into_iter().flat_map(|(items, _)| items).collect();
        Ok(SourceSnapshot { identity: label, items, remarks: if cut { vec![crate::truncated("Notion")] } else { vec![] } })
    }
}

#[derive(Serialize)]
struct Query {
    filter: Option<Filter>,
    sorts: Vec<Sort>,
    page_size: u32,
}

#[derive(Serialize)]
struct Sort {
    timestamp: String,
    direction: String,
}

#[derive(Serialize)]
struct Filter {
    property: String,
    people: Contains,
}

#[derive(Serialize)]
struct Contains {
    contains: String,
}

#[derive(Deserialize)]
struct Results<T> {
    results: Vec<T>,
    next_cursor: Option<String>,
}

#[derive(Deserialize)]
struct User {
    id: String,
    #[serde(rename = "type")]
    kind: Option<String>,
    name: Option<String>,
    person: Option<Email>,
    bot: Option<Bot>,
}

#[derive(Deserialize)]
struct Email {
    email: Option<String>,
}

#[derive(Deserialize)]
struct Bot {
    owner: Option<Owner>,
}

#[derive(Deserialize)]
struct Owner {
    user: Option<Member>,
}

#[derive(Deserialize)]
struct Member {
    id: String,
    #[serde(rename = "type")]
    kind: Option<String>,
    name: Option<String>,
}

#[derive(Deserialize)]
struct RichText {
    plain_text: String,
}

#[derive(Deserialize)]
struct Source {
    id: String,
    name: Option<String>,
}

#[derive(Deserialize)]
struct Database {
    title: Option<Vec<RichText>>,
    data_sources: Vec<Source>,
}

impl Database {
    fn name(&self) -> String {
        let title: String = self.title.iter().flatten().map(|t| t.plain_text.as_str()).collect();
        Some(title).filter(|t| !t.is_empty()).or_else(|| self.data_sources.first().and_then(|s| s.name.clone())).unwrap_or_else(|| "Notion".into())
    }
}

#[derive(Deserialize)]
struct Named {
    name: String,
}

#[derive(Deserialize)]
struct DateValue {
    start: Option<String>,
}

#[derive(Deserialize)]
struct Property {
    #[serde(rename = "type")]
    kind: Option<String>,
    title: Option<Vec<RichText>>,
    status: Option<Named>,
    select: Option<Named>,
    checkbox: Option<bool>,
    date: Option<DateValue>,
}

#[derive(Deserialize)]
struct Page {
    id: String,
    url: Option<String>,
    last_edited_time: DateTime<Utc>,
    properties: HashMap<String, Property>,
}

impl Page {
    fn title(&self) -> String {
        let title = self.properties.values().find_map(|p| p.title.as_ref()).map(|t| t.iter().map(|r| r.plain_text.as_str()).collect::<String>());
        title.filter(|t| !t.is_empty()).unwrap_or_else(|| "Untitled".into())
    }

    /// The status property, or a select named "Status": not any select, which could be a priority.
    fn status(&self) -> Option<&str> {
        let by_type = self.properties.values().find(|p| p.kind.as_deref() == Some("status")).and_then(|p| p.status.as_ref());
        let by_name = || self.properties.iter().find(|(k, _)| k.to_lowercase() == "status").and_then(|(_, p)| p.select.as_ref());
        by_type.or_else(by_name).map(|n| n.name.as_str())
    }

    /// Done when its status is one of the done values, or a checkbox named like one ("Done", "Complete") is
    /// ticked. Any other checkbox ("Urgent", "Blocked") says nothing about being done.
    fn is_done(&self, done_values: &HashSet<String>) -> bool {
        if self.status().is_some_and(|s| done_values.contains(&s.to_lowercase())) {
            return true;
        }
        let named_done = |name: &str| done_values.contains(name) || ["done", "complete", "completed"].contains(&name);
        self.properties.iter().any(|(k, p)| named_done(&k.to_lowercase()) && p.checkbox == Some(true))
    }

    fn due(&self) -> Option<DateTime<Utc>> {
        self.properties.values().filter_map(|p| p.date.as_ref()?.start.as_deref().filter(|s| !s.is_empty()).and_then(due_date)).min()
    }

    fn item(&self, account_id: &str, database: &str) -> InboxItem {
        let mut badges = vec![];
        if let Some(status) = self.status() {
            badges.push(Badge::new("status", status, Tone::Neutral));
        }
        let due = self.due();
        if let Some(due) = due {
            badges.push(due_badge(due));
        }
        InboxItem {
            id: format!("{account_id}/{}", self.id),
            account_id: account_id.into(),
            plugin_id: "notion".into(),
            bundle: InboxBundle::tasks(),
            title: remora_core::text::readable(&self.title()),
            context: database.into(),
            preview: None,
            url: self.url.clone(),
            // The macOS app opens Notion items in the browser too: no notion:// link.
            app_url: None,
            author: Some(Person::named(database)),
            participants: vec![],
            badges,
            date: self.last_edited_time,
            needs_action: true,
            priority: None,
            due,
            expires: None,
            changes: None,
            suggested_people: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stub::*;

    const ID: &str = "0123456789abcdef0123456789abcdef";

    #[test]
    fn extracts_database_id_from_links() {
        assert_eq!(NotionPlugin::database_id(&format!("https://www.notion.so/acme/Tasks-{ID}?v=42")), ID);
        assert_eq!(NotionPlugin::database_id(" 01234567-89ab-cdef-0123-456789abcdef "), ID);
        assert_eq!(NotionPlugin::database_id(" not-an-id "), "not-an-id", "kept as typed: Notion says what's wrong");
    }

    fn page(id: &str, status: &str) -> String {
        format!(
            r#"{{"id": "{id}", "url": "https://notion.so/{id}", "last_edited_time": "2026-10-08T10:00:00.000Z",
             "properties": {{
               "Name": {{"type": "title", "title": [{{"plain_text": "Task {id}"}}]}},
               "Status": {{"type": "status", "status": {{"name": "{status}"}}}},
               "Due": {{"type": "date", "date": {{"start": "2020-01-01"}}}}
             }}}}"#
        )
    }

    #[tokio::test]
    async fn lists_open_tasks_only() {
        let query = format!(r#"{{"results": [{}, {}]}}"#, page("a", "In progress"), page("b", "Done"));
        let http = StubHttp::paths(&[
            ("/users/me", r#"{"id": "u1", "type": "person", "name": "Alice", "person": {"email": "alice@acme.example"}}"#),
            ("/databases/0123456789abcdef0123456789abcdef", r#"{"title": [{"plain_text": "Team tasks"}], "data_sources": [{"id": "ds1"}]}"#),
            ("/data_sources/ds1/query", &query),
        ]);
        let plugin = NotionPlugin::new(&config(&[("token", "secret"), ("databases", ID), ("doneValues", "Done")]), Arc::new(http)).unwrap();
        let snapshot = plugin.fetch().await.unwrap();
        assert_eq!(snapshot.identity, "Alice");
        assert_eq!(snapshot.items.iter().map(|i| i.title.as_str()).collect::<Vec<_>>(), ["Task a"]);
        let item = &snapshot.items[0];
        assert_eq!(item.context, "Team tasks");
        assert!(item.has_badge("overdue") && item.has_badge("status"));
        assert_eq!((item.bundle.clone(), item.needs_action), (InboxBundle::tasks(), true));
        assert!(snapshot.remarks.is_empty());
    }

    /// The query asks for 50 of your tasks, newest first, through the assignee property.
    #[tokio::test]
    async fn queries_your_tasks_by_the_assignee_property() {
        struct Capture(std::sync::Mutex<Vec<String>>);
        #[async_trait]
        impl HttpClient for Capture {
            async fn send(&self, request: Request) -> Result<crate::Response, PluginError> {
                self.0.lock().unwrap().extend(request.body.clone());
                let body = if request.url.ends_with("/users/me") {
                    r#"{"id": "u1", "type": "person", "name": "Alice"}"#
                } else if request.url.contains("/databases/") {
                    r#"{"title": [], "data_sources": [{"id": "ds1", "name": "Sprint"}]}"#
                } else {
                    r#"{"results": [], "next_cursor": "c2"}"#
                };
                Ok(crate::Response { status: 200, body: body.into() })
            }
        }
        let http = Arc::new(Capture(std::sync::Mutex::new(vec![])));
        let plugin = NotionPlugin::new(&config(&[("token", "secret"), ("databases", ID), ("assignee", "Owner")]), http.clone()).unwrap();
        let snapshot = plugin.fetch().await.unwrap();
        assert_eq!(snapshot.remarks, [crate::truncated("Notion")], "a second page means more tasks than shown");
        let sent: serde_json::Value = serde_json::from_str(&http.0.lock().unwrap()[0]).unwrap();
        assert_eq!(sent["page_size"], 50);
        assert_eq!(sent["filter"], serde_json::json!({"property": "Owner", "people": {"contains": "u1"}}));
        assert_eq!(sent["sorts"][0]["timestamp"], "last_edited_time");
    }

    /// A priority select isn't the status, and an "Urgent" checkbox doesn't mean done; a "Done" checkbox does.
    #[test]
    fn reads_the_status_and_done_checkbox_by_type_and_name() {
        let json = |extra: &str| {
            format!(
                r#"{{"id": "p", "last_edited_time": "2026-10-08T10:00:00.000Z", "properties": {{
                  "Name": {{"type": "title", "title": [{{"plain_text": "Ship it"}}]}},
                  "Priority": {{"type": "select", "select": {{"name": "Done"}}}},
                  "Urgent": {{"type": "checkbox", "checkbox": true}}{extra}
                }}}}"#
            )
        };
        let decode = |extra: &str| serde_json::from_str::<Page>(&json(extra)).unwrap();
        let done = |values: &[&str]| values.iter().map(|v| v.to_string()).collect::<HashSet<_>>();
        let open = decode("");
        assert_eq!(open.status(), None);
        assert!(!open.is_done(&done(&["done"])));
        let select_status = decode(r#", "Status": {"type": "select", "select": {"name": "Done"}}"#);
        assert_eq!(select_status.status(), Some("Done"));
        assert!(select_status.is_done(&done(&["done"])));
        let checked = decode(r#", "Done": {"type": "checkbox", "checkbox": true}"#);
        assert!(checked.is_done(&done(&["archived"])));
    }

    /// An internal connection without an email would list everyone's tasks.
    #[tokio::test]
    async fn refuses_to_list_everyones_tasks() {
        let http = StubHttp::paths(&[("/users/me", r#"{"id": "bot1", "type": "bot", "bot": {"owner": {"workspace": true}}}"#)]);
        let plugin = NotionPlugin::new(&config(&[("token", "secret"), ("databases", ID)]), Arc::new(http)).unwrap();
        assert_eq!(plugin.fetch().await.err(), Some(PluginError::Api(UNKNOWN_USER.into())));
    }

    /// An internal connection finds you by email, across pages of users.
    #[tokio::test]
    async fn finds_you_by_email_with_an_internal_connection() {
        let me = r#"{"id": "bot1", "type": "bot", "bot": {"owner": {"workspace": true}}}"#;
        let users = r#"{"results": [{"id": "u7", "type": "person", "person": {"email": "Alice@Acme.example"}}]}"#;
        let http = StubHttp::paths(&[("/users/me", me), ("/users", users), ("/databases/0123456789abcdef0123456789abcdef", r#"{"data_sources": []}"#)]);
        let plugin = NotionPlugin::new(&config(&[("token", "secret"), ("databases", ID), ("email", "alice@acme.example")]), Arc::new(http)).unwrap();
        assert_eq!(plugin.fetch().await.unwrap().identity, "alice@acme.example");

        let http = StubHttp::paths(&[("/users/me", me), ("/users", r#"{"results": []}"#)]);
        let plugin = NotionPlugin::new(&config(&[("token", "secret"), ("databases", ID), ("email", "bob@acme.example")]), Arc::new(http)).unwrap();
        assert_eq!(plugin.fetch().await.err(), Some(PluginError::Api("No Notion user with the email bob@acme.example.".into())));
    }

    /// A token Notion no longer accepts asks for reconnecting; the databases are required.
    #[tokio::test]
    async fn a_rejected_token_is_reconnected() {
        struct Rejecting;
        #[async_trait]
        impl HttpClient for Rejecting {
            async fn send(&self, _: Request) -> Result<crate::Response, PluginError> {
                Ok(crate::Response { status: 401, body: r#"{"object": "error", "code": "unauthorized"}"#.into() })
            }
        }
        let plugin = NotionPlugin::new(&config(&[("token", "secret"), ("databases", ID)]), Arc::new(Rejecting)).unwrap();
        assert_eq!(plugin.fetch().await.err(), Some(PluginError::Unauthorized));
        assert_eq!(NotionPlugin::new(&config(&[("token", "secret")]), Arc::new(Rejecting)).err(), Some(PluginError::MissingField("databases".into())));
    }
}

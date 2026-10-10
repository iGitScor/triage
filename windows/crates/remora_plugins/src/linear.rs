use crate::{decode, HttpClient, PluginConfig, PluginError, Request, SourcePlugin, SourceSnapshot};
use async_trait::async_trait;
use chrono::{DateTime, Duration, Local, NaiveDate, TimeZone, Utc};
use remora_core::{Badge, ConfigField, Egress, InboxBundle, InboxItem, Person, PluginManifest, Priority, Tone};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::sync::Arc;

pub fn manifest() -> PluginManifest {
    PluginManifest {
        id: "linear".into(),
        name: "Linear".into(),
        summary: "Issues assigned to you, and the mentions and comments waiting in your Linear inbox.".into(),
        fields: vec![ConfigField::token("Personal API key", "Linear → Settings → Security & access → Personal API keys.")],
        setup_steps: vec![
            "Click “Create an API key”: Linear opens its Security & access settings.".into(),
            "Under Personal API keys, create a key named Remora (read access is enough) and paste it below.".into(),
        ],
        setup_label: "Create an API key".into(),
        setup_url: Some("https://linear.app/settings/account/security".into()),
        egress: Egress {
            hosts: vec!["api.linear.app".into(), "public.linear.app".into()],
            description: "Reads issues assigned to you and your Linear inbox.".into(),
            external_ai: false,
        },
        logo: Some("linear".into()),
    }
}

/// Open issues assigned to you, plus unread mentions and comments from Linear's inbox.
pub struct LinearPlugin {
    account_id: String,
    token: String,
    http: Arc<dyn HttpClient>,
}

/// Notifications that ask something of you; assignments arrive as issues, the rest stays out.
const ACTIONABLE_TYPES: [&str; 3] = ["mention", "comment", "reply"];

const ISSUES_QUERY: &str = r#"query {
  viewer {
    name
    assignedIssues(first: 50, orderBy: updatedAt, filter: { state: { type: { nin: ["completed", "canceled"] } } }) {
      nodes { id identifier title url priority dueDate updatedAt state { name type } }
      pageInfo { hasNextPage }
    }
  }
}"#;

const NOTIFICATIONS_QUERY: &str = r#"query {
  notifications(first: 50) {
    nodes { id type title url readAt createdAt actor { name avatarUrl } }
  }
}"#;

impl LinearPlugin {
    pub fn new(config: &PluginConfig, http: Arc<dyn HttpClient>) -> Result<Self, PluginError> {
        Ok(LinearPlugin { account_id: config.account_id.clone(), token: config.required("token")?, http })
    }

    async fn query<T: DeserializeOwned>(&self, text: &str) -> Result<T, PluginError> {
        let request = Request::post_json("https://api.linear.app/graphql", &serde_json::json!({ "query": text }))?
            .header("Authorization", self.token.clone());
        let response: GraphQl<T> = decode(self.http.as_ref(), request).await?;
        response.data.ok_or_else(|| {
            let message = response.errors.and_then(|e| e.into_iter().next()).map(|e| e.message).unwrap_or_else(|| "the request failed.".into());
            // "Authentication required, not authenticated": a key Linear no longer accepts.
            if message.to_lowercase().contains("authenticat") {
                PluginError::Unauthorized
            } else {
                PluginError::Api(format!("Linear: {message}"))
            }
        })
    }

    /// https://linear.app/acme/issue/ENG-42/slug → linear://acme/issue/ENG-42/slug.
    pub fn app_url(url: Option<&str>) -> Option<String> {
        let url = reqwest::Url::parse(url?).ok()?;
        (url.host_str() == Some("linear.app")).then(|| format!("linear:/{}", url.path()))
    }
}

#[async_trait]
impl SourcePlugin for LinearPlugin {
    async fn fetch(&self) -> Result<SourceSnapshot, PluginError> {
        let issues: IssuesData = self.query(ISSUES_QUERY).await?;
        // Notifications are a bonus: if their shape differs, issues still come through.
        let notifications = self.query::<NotificationsData>(NOTIFICATIONS_QUERY).await.map(|d| d.notifications.nodes).unwrap_or_default();
        let now = Utc::now();
        let mut items: Vec<InboxItem> = issues.viewer.assigned_issues.nodes.iter().map(|i| i.item(&self.account_id)).collect();
        items.extend(
            notifications
                .iter()
                .filter(|n| n.read_at.is_none() && n.created_at > now - Duration::days(7) && n.is_actionable())
                .map(|n| n.item(&self.account_id)),
        );
        let cut = issues.viewer.assigned_issues.page_info.as_ref().is_some_and(|p| p.has_next_page);
        Ok(SourceSnapshot { identity: issues.viewer.name, items, remarks: if cut { vec![crate::truncated("Linear")] } else { vec![] } })
    }
}

#[derive(Deserialize)]
struct GraphQl<T> {
    data: Option<T>,
    errors: Option<Vec<Message>>,
}

#[derive(Deserialize)]
struct Message {
    message: String,
}

#[derive(Deserialize)]
struct Nodes<T> {
    nodes: Vec<T>,
    #[serde(rename = "pageInfo", default)]
    page_info: Option<PageInfo>,
}

#[derive(Deserialize)]
struct PageInfo {
    #[serde(rename = "hasNextPage")]
    has_next_page: bool,
}

#[derive(Deserialize)]
struct IssuesData {
    viewer: Viewer,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Viewer {
    name: String,
    assigned_issues: Nodes<Issue>,
}

#[derive(Deserialize)]
struct State {
    name: String,
    #[serde(rename = "type")]
    kind: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Issue {
    id: String,
    identifier: String,
    title: String,
    url: Option<String>,
    priority: Option<u8>,
    due_date: Option<String>,
    updated_at: DateTime<Utc>,
    state: Option<State>,
}

impl Issue {
    /// Linear's 1 urgent … 4 low (0 none). Backlog counts as low unless urgent or high.
    fn item_priority(&self) -> Priority {
        let mapped = match self.priority {
            Some(1) => Priority::Urgent,
            Some(2) => Priority::High,
            Some(4) => Priority::Low,
            _ => Priority::Normal,
        };
        if self.state.as_ref().and_then(|s| s.kind.as_deref()) == Some("backlog") && mapped < Priority::High {
            Priority::Low
        } else {
            mapped
        }
    }

    fn item(&self, account_id: &str) -> InboxItem {
        let mut badges = vec![];
        match self.priority {
            Some(1) => badges.push(Badge::new("priority.urgent", "Urgent", Tone::Negative).notifying("Urgent issue")),
            Some(2) => badges.push(Badge::new("priority.high", "High", Tone::Warning)),
            _ => {}
        }
        let due = self.due_date.as_deref().and_then(due_date);
        if let Some(due) = due {
            badges.push(due_badge(due));
        }
        if let Some(state) = &self.state {
            badges.push(Badge::new("status", &state.name, Tone::Neutral));
        }
        let priority = self.item_priority();
        InboxItem {
            id: format!("{account_id}/{}", self.id),
            account_id: account_id.into(),
            plugin_id: "linear".into(),
            bundle: InboxBundle::tasks(),
            title: remora_core::text::readable(&self.title),
            context: self.identifier.clone(),
            preview: None,
            url: self.url.clone(),
            app_url: LinearPlugin::app_url(self.url.as_deref()),
            author: None,
            participants: vec![],
            badges,
            date: self.updated_at,
            needs_action: priority != Priority::Low,
            priority: Some(priority),
            due,
            expires: None,
            changes: None,
            suggested_people: None,
        }
    }
}

/// "2026-10-09" (a day, at local midnight) or a full timestamp. Notion dates too.
pub(crate) fn due_date(raw: &str) -> Option<DateTime<Utc>> {
    if let Ok(date) = DateTime::parse_from_rfc3339(raw) {
        return Some(date.with_timezone(&Utc));
    }
    let day = NaiveDate::parse_from_str(raw, "%Y-%m-%d").ok()?;
    Local.from_local_datetime(&day.and_hms_opt(0, 0, 0)?).earliest().map(|d| d.with_timezone(&Utc))
}

/// Overdue and due-today notify once; later dates are a quiet chip. Shared with Notion, as on macOS.
pub(crate) fn due_badge(due: DateTime<Utc>) -> Badge {
    let today = Local::now().date_naive();
    let day = due.with_timezone(&Local).date_naive();
    if day < today {
        Badge::new("overdue", "Overdue", Tone::Negative).notifying("Task overdue")
    } else if day == today {
        Badge::new("due.today", "Due today", Tone::Warning).notifying("Due today")
    } else {
        Badge::new("due", &day.format("%Y-%m-%d").to_string(), Tone::Neutral)
    }
}

#[derive(Deserialize)]
struct NotificationsData {
    notifications: Nodes<Notification>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Actor {
    name: String,
    avatar_url: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Notification {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    title: Option<String>,
    url: Option<String>,
    read_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    actor: Option<Actor>,
}

impl Notification {
    fn is_actionable(&self) -> bool {
        let kind = self.kind.to_lowercase();
        ACTIONABLE_TYPES.iter().any(|t| kind.contains(t))
    }

    fn item(&self, account_id: &str) -> InboxItem {
        InboxItem {
            id: format!("{account_id}/notification/{}", self.id),
            account_id: account_id.into(),
            plugin_id: "linear".into(),
            bundle: InboxBundle::mentions(),
            title: self.title.as_deref().map_or_else(|| "New comment".into(), remora_core::text::readable),
            context: "Linear".into(),
            preview: None,
            url: self.url.clone(),
            app_url: LinearPlugin::app_url(self.url.as_deref()),
            author: self.actor.as_ref().map(|a| Person { name: a.name.clone(), avatar_url: a.avatar_url.clone(), tone: None }),
            participants: vec![],
            badges: vec![],
            date: self.created_at,
            needs_action: true,
            priority: None,
            due: None,
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

    fn issues(recent: &str) -> String {
        format!(
            r#"{{"data": {{"viewer": {{"name": "Alice", "assignedIssues": {{"nodes": [
              {{"id": "i1", "identifier": "ENG-42", "title": "Fix CSV export", "url": "https://linear.app/x/issue/ENG-42",
               "priority": 1, "dueDate": "2020-01-01", "updatedAt": "{recent}", "state": {{"name": "In Progress", "type": "started"}}}},
              {{"id": "i2", "identifier": "ENG-43", "title": "Polish onboarding", "priority": 4, "updatedAt": "{recent}"}},
              {{"id": "i3", "identifier": "ENG-44", "title": "Upgrade axios", "priority": 3, "updatedAt": "{recent}", "state": {{"name": "Backlog", "type": "backlog"}}}},
              {{"id": "i4", "identifier": "ENG-45", "title": "Patch CVE", "priority": 2, "updatedAt": "{recent}", "state": {{"name": "Backlog", "type": "backlog"}}}}
            ]}}}}}}}}"#
        )
    }

    #[tokio::test]
    async fn issues_and_actionable_notifications() {
        let now = Utc::now();
        let recent = (now - Duration::hours(1)).to_rfc3339();
        let old = (now - Duration::days(10)).to_rfc3339();
        let notifications = format!(
            r#"{{"data": {{"notifications": {{"nodes": [
              {{"id": "n1", "type": "issueCommentMention", "title": "Erin mentioned you in ENG-40", "url": "https://linear.app/x", "readAt": null, "createdAt": "{recent}", "actor": {{"name": "Erin"}}}},
              {{"id": "n2", "type": "issueAssignedToYou", "title": "Assigned", "readAt": null, "createdAt": "{recent}"}},
              {{"id": "n3", "type": "issueNewComment", "title": "Frank commented", "readAt": "{recent}", "createdAt": "{recent}"}},
              {{"id": "n4", "type": "issueMention", "title": "Old mention", "readAt": null, "createdAt": "{old}"}}
            ]}}}}}}"#
        );
        let http = StubHttp::bodies(&[("notifications", &notifications), ("assignedIssues", &issues(&recent))]);
        let snapshot = LinearPlugin::new(&config(&[("token", "lin_api_key")]), Arc::new(http)).unwrap().fetch().await.unwrap();

        assert_eq!(snapshot.identity, "Alice");
        let tasks: Vec<&InboxItem> = snapshot.items.iter().filter(|i| i.bundle == InboxBundle::tasks()).collect();
        assert_eq!(tasks.iter().map(|i| i.context.as_str()).collect::<Vec<_>>(), ["ENG-42", "ENG-43", "ENG-44", "ENG-45"]);
        assert_eq!(tasks.iter().map(|i| i.priority).collect::<Vec<_>>(), [Some(Priority::Urgent), Some(Priority::Low), Some(Priority::Low), Some(Priority::High)]);
        assert_eq!(tasks.iter().map(|i| i.needs_action).collect::<Vec<_>>(), [true, false, false, true]);
        assert!(tasks[0].has_badge("priority.urgent") && tasks[0].has_badge("overdue") && tasks[0].has_badge("status"));
        assert_eq!(tasks[0].app_url.as_deref(), Some("linear://x/issue/ENG-42"));
        let mentions: Vec<&InboxItem> = snapshot.items.iter().filter(|i| i.bundle == InboxBundle::mentions()).collect();
        assert_eq!(mentions.iter().map(|i| i.title.as_str()).collect::<Vec<_>>(), ["Erin mentioned you in ENG-40"]);
    }

    #[tokio::test]
    async fn issues_survive_a_notifications_schema_change_and_bad_keys_are_reported() {
        let recent = Utc::now().to_rfc3339();
        let broken = r#"{"errors": [{"message": "Cannot query field \"title\" on type \"Notification\"."}]}"#;
        let http = StubHttp::bodies(&[("notifications", broken), ("assignedIssues", &issues(&recent))]);
        let snapshot = LinearPlugin::new(&config(&[("token", "k")]), Arc::new(http)).unwrap().fetch().await.unwrap();
        assert_eq!(snapshot.items.len(), 4);

        let rejected = r#"{"errors": [{"message": "Authentication required"}]}"#;
        let http = StubHttp::bodies(&[("query", rejected)]);
        let result = LinearPlugin::new(&config(&[("token", "k")]), Arc::new(http)).unwrap().fetch().await;
        assert_eq!(result.err(), Some(PluginError::Unauthorized), "a rejected key is reconnected");
    }
}

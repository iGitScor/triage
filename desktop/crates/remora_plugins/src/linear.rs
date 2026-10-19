use crate::{decode, HttpClient, PluginConfig, PluginError, Request, SourcePlugin, SourceSnapshot};
use async_trait::async_trait;
use chrono::{DateTime, Duration, Local, NaiveDate, TimeZone, Utc};
use remora_core::{badge, in_review, link_finder};
use remora_core::{
    Badge, ConfigField, Egress, InboxBundle, InboxItem, Person, PluginManifest, Priority, TaskStage, Tone,
};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::collections::HashSet;
use std::sync::Arc;

pub fn manifest() -> PluginManifest {
    PluginManifest {
        id: "linear".into(),
        name: "Linear".into(),
        summary: "Issues assigned to you, and the mentions and comments waiting in your Linear inbox.".into(),
        fields: vec![ConfigField::token(
            "Personal API key",
            "Linear → Settings → Security & access → Personal API keys.",
        )],
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
        sign_in: None,
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
      nodes {
        id identifier title url priority dueDate createdAt updatedAt state { name type }
        cycle { startsAt endsAt }
        inverseRelations(first: 10) { nodes { type issue { identifier state { type } } } }
        comments(last: 5) { nodes { resolvedAt user { isMe } botActor { id } } }
        attachments(first: 10) { nodes { url metadata } }
      }
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
            let message = response
                .errors
                .and_then(|e| e.into_iter().next())
                .map(|e| e.message)
                .unwrap_or_else(|| "the request failed.".into());
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
        // Notifications are a bonus: if they fail, issues still come through, and a remark says so.
        let (notifications, failed) = match self.query::<NotificationsData>(NOTIFICATIONS_QUERY).await {
            Ok(data) => (data.notifications.nodes, None),
            Err(error) => match crate::rate_limit_of(&error) {
                Some(limit) => return Err(limit),
                None => (vec![], Some(crate::partial("Mentions and comments", &error))),
            },
        };
        let mut snapshot = Self::snapshot(issues.viewer, &notifications, &self.account_id, Utc::now());
        snapshot.remarks.extend(failed);
        Ok(snapshot)
    }
}

impl LinearPlugin {
    fn snapshot(
        viewer: Viewer,
        notifications: &[Notification],
        account_id: &str,
        now: DateTime<Utc>,
    ) -> SourceSnapshot {
        let unread: Vec<&Notification> = notifications
            .iter()
            .filter(|n| n.read_at.is_none() && n.created_at > now - Duration::days(7) && n.is_actionable())
            .collect();
        // An issue someone wrote to you about goes first in its priority: its comment's link is the issue's.
        let discussed: HashSet<String> =
            unread.iter().filter_map(|n| n.url.as_deref().map(link_finder::address)).collect();
        let mut items: Vec<InboxItem> = viewer
            .assigned_issues
            .nodes
            .iter()
            .map(|i| {
                let discussed = i.url.as_deref().is_some_and(|url| discussed.contains(&link_finder::address(url)));
                i.item(account_id, now, discussed)
            })
            .collect();
        items.extend(unread.iter().map(|n| n.item(account_id)));
        let cut = viewer.assigned_issues.page_info.as_ref().is_some_and(|p| p.has_next_page);
        SourceSnapshot {
            identity: viewer.name,
            items,
            remarks: cut.then(|| crate::truncated("Linear")).into_iter().collect(),
        }
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
    #[serde(default)]
    attachments: Option<Nodes<Attachment>>,
    #[serde(default)]
    created_at: Option<DateTime<Utc>>,
    #[serde(default)]
    cycle: Option<Cycle>,
    /// Relations pointing at it: "blocks" from another issue blocks this one.
    #[serde(default)]
    inverse_relations: Option<Nodes<Relation>>,
    /// The latest comments, oldest first.
    #[serde(default)]
    comments: Option<Nodes<IssueComment>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct IssueComment {
    resolved_at: Option<DateTime<Utc>>,
    user: Option<CommentAuthor>,
    /// Set when an integration wrote it (GitHub, Slack): not a person waiting on you.
    bot_actor: Option<serde::de::IgnoredAny>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommentAuthor {
    is_me: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Cycle {
    starts_at: Option<DateTime<Utc>>,
    ends_at: Option<DateTime<Utc>>,
}

#[derive(Deserialize)]
struct Relation {
    #[serde(rename = "type")]
    kind: Option<String>,
    issue: Option<RelatedIssue>,
}

#[derive(Deserialize)]
struct RelatedIssue {
    identifier: String,
    /// Only its type is asked for.
    state: Option<RelatedState>,
}

#[derive(Deserialize)]
struct RelatedState {
    #[serde(rename = "type")]
    kind: Option<String>,
}

/// A link on an issue. Its metadata is the integration's own: read leniently, so an unexpected shape never fails the
/// fetch.
#[derive(Deserialize)]
struct Attachment {
    #[serde(default, deserialize_with = "lenient_string")]
    url: Option<String>,
    #[serde(default)]
    metadata: serde_json::Value,
}

impl Attachment {
    /// A pull request's state as the integration last saw it ("open", "merged", "closed"…), when it says.
    fn status(&self) -> Option<&str> {
        self.metadata.get("status").and_then(|s| s.as_str())
    }

    /// Open unless the integration says it was merged or closed.
    fn is_open(&self) -> bool {
        !["merged", "closed"].contains(&self.status().unwrap_or("").to_lowercase().as_str())
    }
}

/// A string, or None for anything else.
fn lenient_string<'de, D: serde::Deserializer<'de>>(decoder: D) -> Result<Option<String>, D::Error> {
    Ok(serde_json::Value::deserialize(decoder)?.as_str().map(String::from))
}

impl Issue {
    fn state_kind(&self) -> Option<&str> {
        self.state.as_ref().and_then(|s| s.kind.as_deref())
    }

    /// The issues still open that block it.
    fn blockers(&self) -> Vec<&str> {
        let relations = self.inverse_relations.as_ref().map_or(&[][..], |r| &r.nodes);
        relations
            .iter()
            .filter(|r| r.kind.as_deref() == Some("blocks"))
            .filter_map(|r| r.issue.as_ref())
            .filter(|other| {
                let kind = other.state.as_ref().and_then(|s| s.kind.as_deref()).unwrap_or("");
                !["completed", "canceled"].contains(&kind)
            })
            .map(|other| other.identifier.as_str())
            .collect()
    }

    /// The latest comment written by a person is someone else's, and its thread isn't resolved: they wait on you,
    /// even when Linear's notification was read long ago.
    fn awaits_your_answer(&self) -> bool {
        let comments = self.comments.as_ref().map_or(&[][..], |c| &c.nodes);
        let Some(last) = comments.iter().rev().find(|c| c.bot_actor.is_none() && c.user.is_some()) else {
            return false;
        };
        last.user.as_ref().and_then(|u| u.is_me) == Some(false) && last.resolved_at.is_none()
    }

    /// Started, then put aside ("Paused", "On hold"): it waits like an issue not started.
    fn paused(&self) -> bool {
        self.state_kind() == Some("started") && TaskStage::paused(self.state.as_ref().map(|s| s.name.as_str()))
    }

    fn in_current_cycle(&self, now: DateTime<Utc>) -> bool {
        match self.cycle.as_ref() {
            Some(Cycle { starts_at: Some(start), ends_at: Some(end) }) => *start <= now && now < *end,
            _ => false,
        }
    }

    /// Where it stands, from its status type. A comment waiting for you comes first, a blocker after the rest; a
    /// status Linear doesn't type stands with the rest.
    fn stage(&self, now: DateTime<Utc>, discussed: bool) -> Option<TaskStage> {
        if discussed || self.awaits_your_answer() {
            return Some(TaskStage::Discussed);
        }
        if !self.blockers().is_empty() {
            return Some(TaskStage::Blocked);
        }
        if self.paused() {
            return Some(TaskStage::Todo);
        }
        match self.state_kind() {
            Some("started") => Some(TaskStage::Started),
            Some("triage") => Some(TaskStage::Triage),
            // A duplicate is another issue's: last, still shown.
            Some("backlog" | "duplicate") => Some(TaskStage::Backlog),
            _ if self.in_current_cycle(now) => Some(TaskStage::Planned),
            Some("unstarted") => Some(TaskStage::Todo),
            _ => None,
        }
    }

    /// Its pull and merge requests (Linear's GitHub and GitLab integrations, or a link pasted by hand).
    fn changes(&self) -> Vec<&Attachment> {
        let attachments = self.attachments.as_ref().map_or(&[][..], |a| &a.nodes);
        attachments.iter().filter(|a| a.url.as_deref().is_some_and(in_review::is_change)).collect()
    }

    /// Up for review: a pull request still open, or a review status ("In Review"). Not from the backlog or triage,
    /// which nobody has started.
    fn in_review(&self) -> bool {
        let kind = self.state.as_ref().and_then(|s| s.kind.as_deref());
        if kind == Some("backlog") || kind == Some("triage") {
            return false;
        }
        self.changes().iter().any(|c| c.is_open()) || in_review::status(self.state.as_ref().map(|s| s.name.as_str()))
    }

    /// Linear's 1 urgent … 4 low (0 none). Backlog and duplicates count as low unless urgent or high.
    fn item_priority(&self) -> Priority {
        let mapped = match self.priority {
            Some(1) => Priority::Urgent,
            Some(2) => Priority::High,
            Some(4) => Priority::Low,
            _ => Priority::Normal,
        };
        if matches!(self.state_kind(), Some("backlog" | "duplicate")) && mapped < Priority::High {
            Priority::Low
        } else {
            mapped
        }
    }

    /// `discussed`: a mention or comment on it still unread in your Linear inbox.
    fn item(&self, account_id: &str, now: DateTime<Utc>, discussed: bool) -> InboxItem {
        let mut badges = vec![];
        match self.priority {
            Some(1) => {
                badges.push(Badge::new(badge::PRIORITY_URGENT, "Urgent", Tone::Negative).notifying("Urgent issue"))
            }
            Some(2) => badges.push(Badge::new(badge::PRIORITY_HIGH, "High", Tone::Warning)),
            _ => {}
        }
        let due = self.due_date.as_deref().and_then(due_date);
        if let Some(due) = due {
            badges.push(due_badge(due));
        }
        if let Some(state) = &self.state {
            badges.push(Badge::new(badge::STATUS, &state.name, Tone::Neutral));
        }
        let stage = self.stage(now, discussed);
        // Someone wrote to you: a low or backlog issue is counted again.
        let discussed = stage == Some(TaskStage::Discussed);
        let priority = if discussed { self.item_priority().max(Priority::Normal) } else { self.item_priority() };
        let unstarted = self.state_kind() != Some("started") || self.paused();
        let long_open = self.created_at.is_some_and(|created| now - created > TaskStage::LONG_OPEN);
        badges.extend(TaskStage::badges(
            discussed,
            !self.blockers().is_empty(),
            stage == Some(TaskStage::Planned),
            priority >= Priority::High && unstarted && long_open,
            false,
        ));
        let item = InboxItem {
            id: InboxItem::make_id(account_id, &self.id),
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
            recurrence: None,
            reactions: None,
            reason: None,
            related: None,
            stage,
        };
        if self.in_review() {
            let changes = self.changes().iter().filter_map(|c| c.url.clone()).collect();
            item.waiting_on_review(changes)
        } else if stage == Some(TaskStage::Blocked) {
            item.blocked_by_another()
        } else {
            item
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
        Badge::new(badge::OVERDUE, "Overdue", Tone::Negative).notifying("Task overdue")
    } else if day == today {
        Badge::new(badge::DUE_TODAY, "Due today", Tone::Warning).notifying("Due today")
    } else {
        Badge::new(badge::DUE, &day.format("%Y-%m-%d").to_string(), Tone::Neutral)
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
            id: InboxItem::make_id(account_id, format!("notification/{}", self.id)),
            account_id: account_id.into(),
            plugin_id: "linear".into(),
            bundle: InboxBundle::mentions(),
            title: self.title.as_deref().map_or_else(|| "New comment".into(), remora_core::text::readable),
            context: "Linear".into(),
            preview: None,
            url: self.url.clone(),
            app_url: LinearPlugin::app_url(self.url.as_deref()),
            author: self.actor.as_ref().map(|a| Person {
                name: a.name.clone(),
                avatar_url: a.avatar_url.clone(),
                review: None,
                bot: false,
            }),
            participants: vec![],
            badges: vec![],
            date: self.created_at,
            needs_action: true,
            priority: None,
            due: None,
            expires: None,
            changes: None,
            suggested_people: None,
            recurrence: None,
            reactions: None,
            reason: None,
            related: None,
            stage: None,
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
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

    /// The fetch `registry`'s egress test runs through the guard.
    pub(crate) fn egress_fixture() -> EgressFixture {
        let issues = issues(&(Utc::now() - Duration::hours(1)).to_rfc3339());
        let none = r#"{"data": {"notifications": {"nodes": []}}}"#;
        EgressFixture {
            settings: vec![("token", "lin_api_key")],
            http: Arc::new(StubHttp::bodies(&[("notifications", none), ("assignedIssues", &issues)])),
        }
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
        let snapshot =
            LinearPlugin::new(&config(&[("token", "lin_api_key")]), Arc::new(http)).unwrap().fetch().await.unwrap();

        assert_eq!(snapshot.identity, "Alice");
        let tasks: Vec<&InboxItem> = snapshot.items.iter().filter(|i| i.bundle == InboxBundle::tasks()).collect();
        assert_eq!(
            tasks.iter().map(|i| i.context.as_str()).collect::<Vec<_>>(),
            ["ENG-42", "ENG-43", "ENG-44", "ENG-45"]
        );
        assert_eq!(
            tasks.iter().map(|i| i.priority).collect::<Vec<_>>(),
            [Some(Priority::Urgent), Some(Priority::Low), Some(Priority::Low), Some(Priority::High)]
        );
        assert_eq!(tasks.iter().map(|i| i.needs_action).collect::<Vec<_>>(), [true, false, false, true]);
        assert!(tasks[0].has_badge("priority.urgent") && tasks[0].has_badge("overdue") && tasks[0].has_badge("status"));
        assert_eq!(tasks[0].app_url.as_deref(), Some("linear://x/issue/ENG-42"));
        let mentions: Vec<&InboxItem> = snapshot.items.iter().filter(|i| i.bundle == InboxBundle::mentions()).collect();
        assert_eq!(mentions.iter().map(|i| i.title.as_str()).collect::<Vec<_>>(), ["Erin mentioned you in ENG-40"]);
    }

    /// An open pull request, or a review status, puts an issue in Awaiting, tied to its pull requests; a merged one,
    /// or a review status still in the backlog, doesn't. Metadata of an odd shape is no failure. As macOS.
    #[tokio::test]
    async fn issues_up_for_review_wait_on_the_reviewers() {
        let recent = Utc::now().to_rfc3339();
        let started = r#"{"name": "In Progress", "type": "started"}"#;
        let review = r#"{"name": "In Review", "type": "started"}"#;
        let backlog_review = r#"{"name": "In Review", "type": "backlog"}"#;
        let open = r#"{"url": "https://github.com/acme/app/pull/9", "metadata": {"status": "open"}}"#;
        let merged = r#"{"url": "https://github.com/acme/app/pull/8", "metadata": {"status": "merged"}}"#;
        let odd = r#"{"url": 12, "metadata": {"status": {"name": "open"}}}"#;
        let issue = |id: &str, state: &str, attachments: &str| {
            format!(
                r#"{{"id": "{id}", "identifier": "ENG-{id}", "title": "Issue {id}", "updatedAt": "{recent}",
                  "state": {state}, "attachments": {{"nodes": [{attachments}]}}}}"#
            )
        };
        let nodes = [
            issue("1", started, open),
            issue("2", started, merged),
            issue("3", review, ""),
            issue("4", backlog_review, open),
            issue("5", started, &format!("{odd}, {}", r#"{"metadata": [1, 2]}"#)),
        ]
        .join(",");
        let body =
            format!(r#"{{"data": {{"viewer": {{"name": "Alice", "assignedIssues": {{"nodes": [{nodes}]}}}}}}}}"#);
        let none = r#"{"data": {"notifications": {"nodes": []}}}"#;
        let http = StubHttp::bodies(&[("notifications", none), ("assignedIssues", &body)]);
        let snapshot = LinearPlugin::new(&config(&[("token", "k")]), Arc::new(http)).unwrap().fetch().await.unwrap();

        let bundles: Vec<_> = snapshot.items.iter().map(|i| (i.context.as_str(), i.bundle.clone())).collect();
        assert_eq!(
            bundles,
            [
                ("ENG-1", InboxBundle::awaiting()),
                ("ENG-2", InboxBundle::tasks()),
                ("ENG-3", InboxBundle::awaiting()),
                ("ENG-4", InboxBundle::tasks()),
                ("ENG-5", InboxBundle::tasks()),
            ]
        );
        assert_eq!(snapshot.items[0].related, Some(vec!["https://github.com/acme/app/pull/9".to_string()]));
        assert!(!snapshot.items[0].needs_action);
        assert_eq!(snapshot.items[2].related, None, "no pull request to tie it to");
    }

    fn fixed_now() -> DateTime<Utc> {
        Utc.timestamp_opt(1_800_000_000, 0).unwrap()
    }

    fn linear(extra: &str, priority: u8) -> Issue {
        let json = format!(
            r#"{{"id": "i1", "identifier": "ENG-42", "title": "Fix", "updatedAt": "2027-01-15T08:00:00Z",
               "url": "https://linear.app/acme/issue/ENG-42/fix", "priority": {priority}, {extra}}}"#
        );
        serde_json::from_str(&json).unwrap()
    }

    /// Where an issue stands, from its status type and its cycle; a status Linear doesn't type isn't guessed. As
    /// macOS.
    #[test]
    fn linear_stages_from_the_status_type_and_the_cycle() {
        let now = fixed_now();
        let cycle = r#""cycle": {"startsAt": "2027-01-10T00:00:00Z", "endsAt": "2027-01-24T00:00:00Z"}"#;
        let stage = |extra: &str| linear(extra, 3).stage(now, false);
        assert_eq!(stage(r#""state": {"name": "In Progress", "type": "started"}"#), Some(TaskStage::Started));
        let planned = linear(&format!(r#""state": {{"name": "Todo", "type": "unstarted"}}, {cycle}"#), 3);
        assert_eq!(planned.stage(now, false), Some(TaskStage::Planned));
        assert!(planned.item("a", now, false).has_badge(badge::CYCLE));
        assert_eq!(stage(r#""state": {"name": "Todo", "type": "unstarted"}"#), Some(TaskStage::Todo));
        assert_eq!(stage(r#""state": {"name": "Triage", "type": "triage"}"#), Some(TaskStage::Triage));
        assert_eq!(stage(r#""state": {"name": "Custom"}"#), None, "no guess");
    }

    /// Blocked by an issue still open: last in its priority, and said so. Not by a completed one. As macOS.
    #[test]
    fn a_linear_issue_blocked_by_an_open_one_goes_down() {
        let now = fixed_now();
        let relations = |kind: &str| {
            linear(
                &format!(
                    r#""state": {{"name": "Todo", "type": "unstarted"}}, "inverseRelations": {{"nodes": [
                      {{"type": "blocks", "issue": {{"identifier": "ENG-40", "state": {{"type": "{kind}"}}}}}},
                      {{"type": "related", "issue": {{"identifier": "ENG-41"}}}}]}}"#
                ),
                3,
            )
        };
        let blocked = relations("started");
        assert_eq!(blocked.blockers(), ["ENG-40"]);
        let item = blocked.item("a", now, false);
        assert!(item.stage == Some(TaskStage::Blocked) && item.has_badge(badge::BLOCKED));
        assert!(item.bundle == InboxBundle::blocked() && !item.needs_action, "it waits on ENG-40, in Waiting");
        assert!(relations("completed").blockers().is_empty());
    }

    /// A comment waiting for you lifts the issue first in its priority, and a low one is counted again. As macOS.
    #[test]
    fn a_linear_issue_someone_wrote_about_comes_first() {
        let viewer = Viewer {
            name: "Alice".into(),
            assigned_issues: Nodes {
                nodes: vec![linear(r#""state": {"name": "Backlog", "type": "backlog"}"#, 4)],
                page_info: None,
            },
        };
        let comment: Notification = serde_json::from_str(
            r#"{"id": "n1", "type": "issueNewComment", "url": "https://linear.app/acme/issue/ENG-42/old#comment-1",
                "createdAt": "2027-01-15T07:00:00Z"}"#,
        )
        .unwrap();
        let items = LinearPlugin::snapshot(viewer, &[comment], "a", fixed_now()).items;
        let issue = items.iter().find(|i| i.context == "ENG-42").unwrap();
        assert!(issue.stage == Some(TaskStage::Discussed) && issue.has_badge(badge::DISCUSSED));
        assert!(issue.priority == Some(Priority::Normal) && issue.needs_action);
    }

    /// A comment from someone else, never answered nor resolved, lifts the issue even when Linear's notification was
    /// read months ago; your answer, a resolved thread or a bot's comment don't. A duplicate, otherwise last. As macOS.
    #[test]
    fn a_linear_comment_left_unanswered_comes_first() {
        let issue = |comments: &str| {
            linear(
                &format!(
                    r#""state": {{"name": "Duplicate", "type": "duplicate"}}, "comments": {{"nodes": [{comments}]}}"#
                ),
                3,
            )
            .item("a", fixed_now(), false)
        };
        let theirs = r#"{"resolvedAt": null, "user": {"isMe": false}, "botActor": null}"#;
        let mine = r#"{"resolvedAt": null, "user": {"isMe": true}, "botActor": null}"#;
        let bot = r#"{"resolvedAt": null, "user": null, "botActor": {"id": "github"}}"#;
        let unanswered = issue(theirs);
        assert!(
            unanswered.stage == Some(TaskStage::Discussed)
                && unanswered.has_badge(badge::DISCUSSED)
                && unanswered.needs_action
        );
        assert_eq!(
            issue(&format!("{theirs}, {bot}")).stage,
            Some(TaskStage::Discussed),
            "a bot after them doesn't answer"
        );
        assert_eq!(issue(&format!("{theirs}, {mine}")).stage, Some(TaskStage::Backlog));
        assert_eq!(
            issue(r#"{"resolvedAt": "2027-01-01T00:00:00Z", "user": {"isMe": false}}"#).stage,
            Some(TaskStage::Backlog)
        );
        assert_eq!(issue("").priority, Some(Priority::Low), "a duplicate counts as low");
    }

    /// Started, then paused: ranked as not started, and old enough to say so when it matters. As macOS.
    #[test]
    fn a_paused_issue_is_not_work_in_progress() {
        let now = fixed_now();
        let paused =
            linear(r#""state": {"name": "Paused", "type": "started"}, "createdAt": "2026-12-01T00:00:00Z""#, 2);
        assert_eq!(paused.stage(now, false), Some(TaskStage::Todo));
        assert!(paused.item("a", now, false).has_badge(badge::AGE));
        assert!(TaskStage::paused(Some("On hold")) && TaskStage::paused(Some("En pause")));
        assert!(!TaskStage::paused(Some("In Progress")));
    }

    /// Urgent or high, not started, open for weeks: said on the row, without making Done come back. As macOS.
    #[test]
    fn an_urgent_issue_left_for_weeks_says_so() {
        let now = fixed_now();
        let old = linear(r#""state": {"name": "Todo", "type": "unstarted"}, "createdAt": "2026-12-01T00:00:00Z""#, 2);
        let item = old.item("a", now, false);
        assert!(item.has_badge(badge::AGE));
        let mut aged = item.clone();
        aged.badges.retain(|b| b.id != badge::AGE);
        assert_eq!(aged.fingerprint(), item.fingerprint());
        let started =
            linear(r#""state": {"name": "In Progress", "type": "started"}, "createdAt": "2026-12-01T00:00:00Z""#, 2);
        assert!(!started.item("a", now, false).has_badge(badge::AGE));
    }

    #[tokio::test]
    async fn issues_survive_a_notifications_schema_change_and_bad_keys_are_reported() {
        let recent = Utc::now().to_rfc3339();
        let broken = r#"{"errors": [{"message": "Cannot query field \"title\" on type \"Notification\"."}]}"#;
        let http = StubHttp::bodies(&[("notifications", broken), ("assignedIssues", &issues(&recent))]);
        let snapshot = LinearPlugin::new(&config(&[("token", "k")]), Arc::new(http)).unwrap().fetch().await.unwrap();
        assert_eq!(snapshot.items.len(), 4);
        assert_eq!(snapshot.remarks.iter().map(|r| r.code.as_str()).collect::<Vec<_>>(), ["partial"], "and says so");

        let rejected = r#"{"errors": [{"message": "Authentication required"}]}"#;
        let http = StubHttp::bodies(&[("query", rejected)]);
        let result = LinearPlugin::new(&config(&[("token", "k")]), Arc::new(http)).unwrap().fetch().await;
        assert_eq!(result.err(), Some(PluginError::Unauthorized), "a rejected key is reconnected");
    }
}

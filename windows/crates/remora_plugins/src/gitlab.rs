use crate::code_review::{Checks, CodeReview};
use crate::{decode, url_with_query, HttpClient, PluginConfig, PluginError, Request, SourcePlugin, SourceSnapshot};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures::future::join_all;
use remora_core::{ConfigField, Egress, InboxBundle, InboxItem, Person, PluginManifest, Tone};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::collections::HashSet;
use std::sync::Arc;

pub fn manifest() -> PluginManifest {
    PluginManifest {
        id: "gitlab".into(),
        name: "GitLab".into(),
        summary: "Merge requests you opened and those you review. Works with self-hosted GitLab.".into(),
        fields: vec![ConfigField::host("https://gitlab.com"), ConfigField::token("Personal access token", "Token with the read_api scope.")],
        setup_steps: vec![
            "If you use self-hosted GitLab, set the host first.".into(),
            "Click “Create a token”: GitLab opens with the read_api scope selected.".into(),
            "Click “Create personal access token” and paste it below.".into(),
        ],
        setup_label: "Create a token".into(),
        setup_url: Some("{host}/-/user_settings/personal_access_tokens?name=Remora&scopes=read_api".into()),
        egress: Egress {
            hosts: vec!["gitlab.com".into()],
            description: "Reads your merge requests, approvals and pipelines from your GitLab host.".into(),
            external_ai: false,
        },
        logo: Some("gitlab".into()),
    }
}

/// Merge requests you opened and those where you are a reviewer, on gitlab.com or self-hosted.
pub struct GitLabPlugin {
    account_id: String,
    host: String,
    token: String,
    http: Arc<dyn HttpClient>,
}

impl GitLabPlugin {
    pub fn new(config: &PluginConfig, http: Arc<dyn HttpClient>) -> Result<Self, PluginError> {
        Ok(GitLabPlugin { account_id: config.account_id.clone(), host: config.url("host")?, token: config.required("token")?, http })
    }

    async fn get<T: DeserializeOwned>(&self, path: &str, query: &[(&str, &str)]) -> Result<T, PluginError> {
        let url = url_with_query(&self.host, &format!("api/v4/{path}"), query);
        decode(self.http.as_ref(), Request::get(url).header("PRIVATE-TOKEN", self.token.clone())).await
    }

    async fn item(&self, mr: MergeRequest, authored: bool) -> Result<InboxItem, PluginError> {
        let base = format!("projects/{}/merge_requests/{}", mr.project_id, mr.iid);
        let approvals_path = format!("{base}/approvals");
        let (approvals, detail) = futures::join!(self.get::<Approvals>(&approvals_path, &[]), self.get::<Detail>(&base, &[]));
        Ok(Self::map(mr, approvals?, detail?, authored, &self.account_id, &self.host))
    }

    fn map(mr: MergeRequest, approvals: Approvals, detail: Detail, authored: bool, account_id: &str, host: &str) -> InboxItem {
        let approvers: HashSet<String> = approvals.approved_by.iter().flatten().map(|a| a.user.username.clone()).collect();
        let is_draft = mr.draft.or(mr.work_in_progress).unwrap_or(false);
        let checks = match detail.head_pipeline.as_ref().map(|p| p.status.as_str()) {
            Some("success") => Checks::Passing,
            Some("failed") => Checks::Failing,
            Some("running" | "pending" | "created" | "preparing" | "waiting_for_resource") => Checks::Running,
            _ => Checks::None,
        };
        let review = CodeReview {
            is_draft,
            is_approved: !approvers.is_empty() && approvals.approved != Some(false),
            changes_requested: mr.detailed_merge_status.as_deref() == Some("requested_changes"),
            has_conflicts: mr.has_conflicts.unwrap_or(false),
            checks,
            comments: mr.user_notes_count.unwrap_or(0),
            additions: None,
            deletions: None,
        };
        let avatar = |raw: &Option<String>| raw.as_ref().filter(|s| !s.is_empty()).map(|s| if s.starts_with('/') { format!("{host}{s}") } else { s.clone() });
        let mut participants: Vec<Person> = mr
            .reviewers
            .iter()
            .flatten()
            .map(|r| Person { name: r.username.clone(), avatar_url: avatar(&r.avatar_url), tone: approvers.contains(&r.username).then_some(Tone::Accent) })
            .collect();
        for approver in approvals.approved_by.iter().flatten() {
            if !participants.iter().any(|p| p.name == approver.user.username) {
                participants.push(Person { name: approver.user.username.clone(), avatar_url: avatar(&approver.user.avatar_url), tone: Some(Tone::Accent) });
            }
        }
        let project = mr.references.as_ref().and_then(|r| r.full.split('!').next().map(str::to_string)).unwrap_or_else(|| format!("project {}", mr.project_id));
        InboxItem {
            id: format!("{account_id}/{}", mr.id),
            account_id: account_id.into(),
            plugin_id: "gitlab".into(),
            bundle: if authored { InboxBundle::authored() } else { InboxBundle::reviews() },
            title: mr.title,
            context: format!("{project} !{}", mr.iid),
            preview: None,
            url: Some(mr.web_url),
            app_url: None,
            author: Some(Person { name: mr.author.username.clone(), avatar_url: avatar(&mr.author.avatar_url), tone: None }),
            participants,
            badges: review.badges(authored),
            date: mr.updated_at,
            needs_action: review.needs_action(authored),
            priority: None,
            due: None,
        }
    }
}

#[async_trait]
impl SourcePlugin for GitLabPlugin {
    async fn fetch(&self) -> Result<SourceSnapshot, PluginError> {
        let user: User = self.get("user", &[]).await?;
        let authored_query = [("scope", "created_by_me"), ("state", "opened"), ("per_page", "50")];
        let reviewing_query = [("scope", "all"), ("state", "opened"), ("per_page", "50"), ("reviewer_username", user.username.as_str())];
        let (authored, reviewing) = futures::join!(
            self.get::<Vec<MergeRequest>>("merge_requests", &authored_query),
            self.get::<Vec<MergeRequest>>("merge_requests", &reviewing_query)
        );
        let all: Vec<(MergeRequest, bool)> = authored?.into_iter().map(|mr| (mr, true)).chain(reviewing?.into_iter().map(|mr| (mr, false))).collect();
        let mut items = Vec::with_capacity(all.len());
        let mut pending = all.into_iter().peekable();
        while pending.peek().is_some() {
            let chunk: Vec<_> = pending.by_ref().take(8).collect();
            for item in join_all(chunk.into_iter().map(|(mr, authored)| self.item(mr, authored))).await {
                items.push(item?);
            }
        }
        Ok(SourceSnapshot { identity: user.username, items })
    }
}

#[derive(Deserialize)]
struct User {
    username: String,
    avatar_url: Option<String>,
}

#[derive(Deserialize)]
struct References {
    full: String,
}

#[derive(Deserialize)]
struct MergeRequest {
    id: u64,
    iid: u64,
    project_id: u64,
    title: String,
    web_url: String,
    draft: Option<bool>,
    work_in_progress: Option<bool>,
    updated_at: DateTime<Utc>,
    author: User,
    reviewers: Option<Vec<User>>,
    user_notes_count: Option<u32>,
    has_conflicts: Option<bool>,
    references: Option<References>,
    detailed_merge_status: Option<String>,
}

#[derive(Deserialize)]
struct Approver {
    user: User,
}

#[derive(Deserialize)]
struct Approvals {
    approved: Option<bool>,
    approved_by: Option<Vec<Approver>>,
}

#[derive(Deserialize)]
struct Pipeline {
    status: String,
}

#[derive(Deserialize)]
struct Detail {
    head_pipeline: Option<Pipeline>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stub::*;

    const MR: &str = r#"{"id": 101, "iid": 12, "project_id": 5, "title": "Speed up CI", "web_url": "https://gitlab.acme.io/g/p/-/merge_requests/12",
     "draft": false, "updated_at": "2026-10-08T10:00:00.000Z", "author": {"username": "alice", "avatar_url": "/uploads/a.png"},
     "reviewers": [{"username": "erin"}, {"username": "frank"}], "user_notes_count": 2, "has_conflicts": false,
     "references": {"full": "g/p!12"}, "detailed_merge_status": "mergeable"}"#;

    #[tokio::test]
    async fn maps_merge_requests_with_approvals_and_pipeline() {
        let list = format!("[{MR}]");
        let http = StubHttp::paths(&[
            ("/api/v4/user", r#"{"username": "alice"}"#),
            ("/api/v4/merge_requests", &list),
            ("/merge_requests/12/approvals", r#"{"approved": true, "approved_by": [{"user": {"username": "erin"}}]}"#),
            ("/projects/5/merge_requests/12", r#"{"head_pipeline": {"status": "success"}}"#),
        ]);
        let plugin = GitLabPlugin::new(&config(&[("host", "gitlab.acme.io"), ("token", "t")]), Arc::new(http)).unwrap();
        let snapshot = plugin.fetch().await.unwrap();

        assert_eq!(snapshot.identity, "alice");
        assert_eq!(snapshot.items.iter().map(|i| i.bundle.clone()).collect::<Vec<_>>(), [InboxBundle::authored(), InboxBundle::reviews()]);
        let item = &snapshot.items[0];
        assert_eq!(item.context, "g/p !12");
        assert!(item.has_badge("approved") && item.has_badge("checks.passing") && item.has_badge("comments"));
        assert_eq!(item.participants.iter().find(|p| p.name == "erin").and_then(|p| p.tone), Some(Tone::Accent));
        assert_eq!(item.participants.iter().find(|p| p.name == "frank").and_then(|p| p.tone), None);
        assert_eq!(item.author.as_ref().and_then(|a| a.avatar_url.clone()).as_deref(), Some("https://gitlab.acme.io/uploads/a.png"));
    }
}

use crate::code_review::{Checks, CodeReview};
use crate::{decode, url_with_query, HttpClient, PluginConfig, PluginError, Request, SourcePlugin, SourceSnapshot};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures::future::join_all;
use remora_core::{ChangeSet, ChangedFile, ConfigField, Egress, InboxBundle, InboxItem, Person, PluginManifest, Tone};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
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

    /// The REST way, one merge request at a time. Best-effort: a call that fails leaves that part out instead of
    /// failing the whole account.
    async fn item(&self, mr: MergeRequest, authored: bool) -> InboxItem {
        let base = format!("projects/{}/merge_requests/{}", mr.project_id, mr.iid);
        let approvals_path = format!("{base}/approvals");
        let diffs_path = format!("{base}/diffs");
        // Review prep: file paths only. The diff text in this response is never decoded.
        let files = async {
            if authored { None } else { self.get::<Vec<DiffFile>>(&diffs_path, &[("per_page", "50")]).await.ok() }
        };
        let (approvals, detail, files) = futures::join!(self.get::<Approvals>(&approvals_path, &[]), self.get::<Detail>(&base, &[]), files);
        let approvals = approvals.unwrap_or(Approvals { approved: None, approved_by: None });
        Self::map(mr, approvals, detail.unwrap_or(Detail { head_pipeline: None }), files, authored, &self.account_id, &self.host)
    }

    /// Approvals, pipeline and changed files of every open merge request you wrote or review, in one GraphQL
    /// request instead of two or three REST calls each. Keyed by the REST id. Same query as the macOS app.
    async fn graphql_details(&self) -> Result<HashMap<u64, graphql::Details>, PluginError> {
        const QUERY: &str = "query { currentUser { \
            authored: authoredMergeRequests(state: opened, first: 50) { nodes { ...status } } \
            reviewing: reviewRequestedMergeRequests(state: opened, first: 50) { nodes { ...status diffStats { path additions deletions } } } } } \
            fragment status on MergeRequest { id approved approvedBy { nodes { username avatarUrl } } headPipeline { status } }";
        let url = format!("{}/api/graphql", self.host.trim_end_matches('/'));
        let request = Request::post_json(url, &serde_json::json!({ "query": QUERY }))?.header("PRIVATE-TOKEN", self.token.clone());
        let response: graphql::Response = decode(self.http.as_ref(), request).await?;
        Ok(response.details())
    }

    fn map(mr: MergeRequest, approvals: Approvals, detail: Detail, files: Option<Vec<DiffFile>>, authored: bool, account_id: &str, host: &str) -> InboxItem {
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
            title: remora_core::text::readable(&mr.title),
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
            expires: None,
            changes: files.filter(|f| !f.is_empty()).map(|files| {
                let files = files.into_iter().map(|f| ChangedFile { path: f.new_path.or(f.old_path).unwrap_or_else(|| "?".into()), additions: f.additions, deletions: f.deletions });
                ChangeSet::new(files.collect(), None)
            }),
            suggested_people: None,
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
        let (authored, reviewing) = (authored?, reviewing?);
        // A full page means there may be more: GitLab's count header isn't always sent.
        let remarks = if authored.len() >= 50 || reviewing.len() >= 50 { vec![crate::truncated("GitLab")] } else { vec![] };
        let all: Vec<(MergeRequest, bool)> = authored.into_iter().map(|mr| (mr, true)).chain(reviewing.into_iter().map(|mr| (mr, false))).collect();
        // One GraphQL request for every merge request's details; an instance where it fails gets the REST calls.
        let details = if all.is_empty() { HashMap::new() } else { self.graphql_details().await.unwrap_or_default() };
        let mut items = Vec::with_capacity(all.len());
        let mut rest = vec![];
        for (mr, authored) in all {
            match details.get(&mr.id).cloned() {
                Some(found) => {
                    let files = if authored { None } else { found.files };
                    items.push(Self::map(mr, found.approvals, found.detail, files, authored, &self.account_id, &self.host));
                }
                None => rest.push((mr, authored)),
            }
        }
        let mut pending = rest.into_iter().peekable();
        while pending.peek().is_some() {
            let chunk: Vec<_> = pending.by_ref().take(4).collect();
            items.extend(join_all(chunk.into_iter().map(|(mr, authored)| self.item(mr, authored))).await);
        }
        Ok(SourceSnapshot { identity: user.username, items, remarks })
    }
}

#[derive(Deserialize, Clone)]
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

#[derive(Deserialize, Clone)]
struct Approver {
    user: User,
}

#[derive(Deserialize, Clone)]
struct Approvals {
    approved: Option<bool>,
    approved_by: Option<Vec<Approver>>,
}

#[derive(Deserialize, Clone)]
struct Pipeline {
    status: String,
}

#[derive(Deserialize, Clone)]
struct Detail {
    head_pipeline: Option<Pipeline>,
}

/// One file of `/diffs`: only the paths are decoded, never the `diff` text. GraphQL adds the line counts.
#[derive(Deserialize, Clone)]
struct DiffFile {
    new_path: Option<String>,
    old_path: Option<String>,
    additions: Option<u32>,
    deletions: Option<u32>,
}

/// The GraphQL answer, turned into the REST shapes above so both ways build the same items.
mod graphql {
    use super::{Approvals, Approver, Detail, DiffFile, Pipeline, User};
    use serde::Deserialize;
    use std::collections::HashMap;

    #[derive(Deserialize)]
    pub struct Response {
        data: Option<Data>,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Data {
        current_user: Option<CurrentUser>,
    }

    #[derive(Deserialize)]
    struct CurrentUser {
        authored: Option<Connection>,
        reviewing: Option<Connection>,
    }

    #[derive(Deserialize)]
    struct Connection {
        nodes: Option<Vec<Option<Node>>>,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Node {
        id: String,
        approved: Option<bool>,
        approved_by: Option<People>,
        head_pipeline: Option<GqlPipeline>,
        diff_stats: Option<Vec<Stat>>,
    }

    #[derive(Deserialize)]
    struct Stat {
        path: String,
        additions: Option<u32>,
        deletions: Option<u32>,
    }

    #[derive(Clone)]
    pub struct Details {
        pub approvals: Approvals,
        pub detail: Detail,
        pub files: Option<Vec<DiffFile>>,
    }

    #[derive(Deserialize)]
    struct People {
        nodes: Option<Vec<Option<GqlUser>>>,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct GqlUser {
        username: String,
        avatar_url: Option<String>,
    }

    #[derive(Deserialize)]
    struct GqlPipeline {
        status: String,
    }

    impl Response {
        pub fn details(self) -> HashMap<u64, Details> {
            let user = self.data.and_then(|d| d.current_user);
            let lists = user.map(|u| [u.authored, u.reviewing]).into_iter().flatten().flatten();
            let mut found = HashMap::new();
            for node in lists.flat_map(|c| c.nodes.unwrap_or_default()).flatten() {
                // "gid://gitlab/MergeRequest/101" → 101, the id REST uses.
                let Some(id) = node.id.rsplit('/').next().and_then(|n| n.parse::<u64>().ok()) else { continue };
                let approved_by = node.approved_by.and_then(|p| p.nodes).unwrap_or_default().into_iter().flatten()
                    .map(|u| Approver { user: User { username: u.username, avatar_url: u.avatar_url } })
                    .collect();
                let approvals = Approvals { approved: node.approved, approved_by: Some(approved_by) };
                let detail = Detail { head_pipeline: node.head_pipeline.map(|p| Pipeline { status: p.status.to_lowercase() }) };
                let files = node.diff_stats.map(|stats| {
                    stats.into_iter().map(|s| DiffFile { new_path: Some(s.path), old_path: None, additions: s.additions, deletions: s.deletions }).collect()
                });
                // The same merge request in both lists: keep the one with files.
                if found.get(&id).is_none_or(|d: &Details| d.files.is_none()) || files.is_some() {
                    found.insert(id, Details { approvals, detail, files });
                }
            }
            found
        }
    }
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
            ("/merge_requests/12/diffs", r#"[{"new_path": "db/migrate/add_avatars.rb", "old_path": "db/migrate/add_avatars.rb", "diff": "+ secret code"}]"#),
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
        assert_eq!(item.changes, None, "your own merge requests don't need a review prep");
        let review = &snapshot.items[1];
        let changes = review.changes.as_ref().unwrap();
        assert_eq!(changes.files, [ChangedFile { path: "db/migrate/add_avatars.rb".into(), additions: None, deletions: None }]);
        assert_eq!(changes.file_count, 1);
        let stored = serde_json::to_string(review).unwrap();
        assert!(!stored.contains("secret code"), "diff text is never kept");
    }

    /// One GraphQL request instead of two REST calls per merge request.
    #[tokio::test]
    async fn details_come_from_one_graphql_request() {
        let list = format!("[{MR}]");
        let graphql = r#"{"data": {"currentUser": {
          "authored": {"nodes": [{"id": "gid://gitlab/MergeRequest/101", "approved": true,
            "approvedBy": {"nodes": [{"username": "erin", "avatarUrl": null}]}, "headPipeline": {"status": "FAILED"}}]},
          "reviewing": {"nodes": [{"id": "gid://gitlab/MergeRequest/101", "approved": true,
            "approvedBy": {"nodes": [{"username": "erin"}]}, "headPipeline": {"status": "FAILED"},
            "diffStats": [{"path": "app/models/user.rb", "additions": 12, "deletions": 3}]}]}}}}"#;
        let stub = StubHttp::paths(&[("/api/v4/user", r#"{"username": "alice"}"#), ("/api/v4/merge_requests", &list), ("/api/graphql", graphql)]);
        let http = Arc::new(Recording::new(stub));
        let plugin = GitLabPlugin::new(&config(&[("host", "gitlab.acme.io"), ("token", "t")]), http.clone()).unwrap();
        let snapshot = plugin.fetch().await.unwrap();

        let paths = http.paths();
        assert!(paths.iter().all(|p| !p.contains("/projects/")), "no per-merge-request call: {paths:?}");
        assert_eq!(paths.len(), 4, "user, the two lists, one GraphQL request");
        let item = &snapshot.items[0];
        assert!(item.has_badge("approved") && item.has_badge("checks.failing"));
        assert_eq!(item.participants.iter().find(|p| p.name == "erin").and_then(|p| p.tone), Some(Tone::Accent));
        assert_eq!(item.changes, None, "your own merge requests don't need a review prep");
        let review = &snapshot.items[1];
        assert_eq!(review.bundle, InboxBundle::reviews());
        assert_eq!(
            review.changes.as_ref().map(|c| c.files.clone()),
            Some(vec![ChangedFile { path: "app/models/user.rb".into(), additions: Some(12), deletions: Some(3) }])
        );
    }
}

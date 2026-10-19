use crate::code_review::{Checks, CodeReview};
use crate::{
    decode, maintained_repositories, partial, rate_limit_of, url_with_query, AccountCache, HttpClient, PluginConfig,
    PluginError, Request, SourcePlugin, SourceSnapshot, IDENTITY_LIFETIME,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures::future::join_all;
use remora_core::ReviewState;
use remora_core::{
    badge, Badge, ChangeSet, ChangedFile, ConfigField, Egress, InboxBundle, InboxItem, Message, Person, PluginManifest,
    Tone,
};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// One page, asked for by every list request: a full page of merge requests means the list was cut.
const PAGE_SIZE: usize = 50;

pub fn manifest() -> PluginManifest {
    PluginManifest {
        id: "gitlab".into(),
        name: "GitLab".into(),
        summary: "Merge requests you opened and those you review, and your To-Dos. Works with self-hosted GitLab."
            .into(),
        fields: vec![
            ConfigField::host("https://gitlab.com"),
            ConfigField::token("Personal access token", "Token with the read_api scope."),
            ConfigField::text(
                "repositories",
                "Projects I maintain",
                "acme/web, acme/tools/cli",
                "Optional. When the last pipeline of a project's default branch failed, it goes to To fix.",
            ),
        ],
        setup_steps: vec![
            "If you use self-hosted GitLab, set the host first.".into(),
            "Click “Create a token”: GitLab opens with the read_api scope selected.".into(),
            "Click “Create personal access token” and paste it below.".into(),
        ],
        setup_label: "Create a token".into(),
        setup_url: Some("{host}/-/user_settings/personal_access_tokens?name=Remora&scopes=read_api".into()),
        egress: Egress {
            hosts: vec!["gitlab.com".into()],
            description: "Reads your merge requests, approvals, pipelines and To-Dos from your GitLab host, and the \
                          default branch's pipelines of the projects you maintain."
                .into(),
            external_ai: false,
        },
        logo: Some("gitlab".into()),
        sign_in: None,
    }
}

/// Merge requests you opened and those where you are a reviewer, on gitlab.com or self-hosted; for the projects you
/// maintain, a failed pipeline on the default branch.
pub struct GitLabPlugin {
    account_id: String,
    host: String,
    token: String,
    projects: Vec<String>,
    http: Arc<dyn HttpClient>,
    cache: Arc<AccountCache>,
}

impl GitLabPlugin {
    pub fn new(config: &PluginConfig, http: Arc<dyn HttpClient>) -> Result<Self, PluginError> {
        Ok(GitLabPlugin {
            account_id: config.account_id.clone(),
            host: config.url("host")?,
            token: config.required("token")?,
            projects: maintained_repositories(config.values.get("repositories").map_or("", String::as_str), true),
            http,
            cache: config.cache.clone(),
        })
    }

    async fn get<T: DeserializeOwned>(&self, path: &str, query: &[(&str, &str)]) -> Result<T, PluginError> {
        let url = url_with_query(&self.host, &format!("api/v4/{path}"), query)?;
        decode(self.http.as_ref(), Request::get(url).header("PRIVATE-TOKEN", self.token.clone())).await
    }

    /// A project you maintain whose default branch's last finished pipeline failed: to fix. Best effort: a project
    /// that can't be read is a remark, except a rate limit, which fails the fetch.
    async fn maintained(&self, project: &str) -> Result<(Option<InboxItem>, Option<Message>), PluginError> {
        let failing = async {
            let info: Project = self.get(&format!("projects/{}", project.replace('/', "%2F")), &[]).await?;
            let Some(branch) = info.default_branch else { return Ok(None) };
            let base = format!("projects/{}", info.id);
            let pipelines: Vec<BranchPipeline> = self
                .get(
                    &format!("{base}/pipelines"),
                    &[("ref", branch.as_str()), ("scope", "finished"), ("per_page", "1")],
                )
                .await?;
            let Some(pipeline) = pipelines.into_iter().next().filter(|p| p.status == "failed") else { return Ok(None) };
            // The commit's title says what broke it; without it, the pipeline still shows.
            let commit = self.get::<Commit>(&format!("{base}/repository/commits/{}", pipeline.sha), &[]).await.ok();
            Ok::<_, PluginError>(Some(pipeline.item(&self.account_id, project, &branch, commit)))
        };
        match failing.await {
            Ok(item) => Ok((item, None)),
            Err(error) => match rate_limit_of(&error) {
                Some(limit) => Err(limit),
                None => Ok((None, Some(partial("Projects you maintain", &error)))),
            },
        }
    }

    /// The REST way, one merge request at a time. Best-effort: a call that fails leaves that part out instead of
    /// failing the whole account, and is returned so the fetch can say so.
    async fn item(&self, mr: MergeRequest, authored: bool) -> (InboxItem, Option<PluginError>) {
        let base = format!("projects/{}/merge_requests/{}", mr.project_id, mr.iid);
        let approvals_path = format!("{base}/approvals");
        let diffs_path = format!("{base}/diffs");
        let per_page = PAGE_SIZE.to_string();
        // Review prep: file paths only. The diff text in this response is never decoded.
        let files = async {
            if authored {
                None
            } else {
                Some(self.get::<Vec<DiffFile>>(&diffs_path, &[("per_page", per_page.as_str())]).await)
            }
        };
        let (approvals, detail, files) =
            futures::join!(self.get::<Approvals>(&approvals_path, &[]), self.get::<Detail>(&base, &[]), files);
        let failed = [approvals.as_ref().err(), detail.as_ref().err(), files.as_ref().and_then(|f| f.as_ref().err())]
            .into_iter()
            .flatten()
            .next()
            .cloned();
        let approvals = approvals.unwrap_or(Approvals { approved: None, approved_by: None });
        let item = Self::map(
            mr,
            approvals,
            detail.unwrap_or(Detail { head_pipeline: None }),
            files.and_then(Result::ok),
            authored,
            &self.account_id,
            &self.host,
        );
        (item, failed)
    }

    /// Approvals, pipeline and changed files of every open merge request you wrote or review, in one GraphQL
    /// request instead of two or three REST calls each. Keyed by the REST id. Same query as the macOS app.
    async fn graphql_details(&self) -> Result<HashMap<u64, graphql::Details>, PluginError> {
        let query = format!(
            "query {{ currentUser {{ \
            authored: authoredMergeRequests(state: opened, first: {PAGE_SIZE}) {{ nodes {{ ...status }} }} \
            reviewing: reviewRequestedMergeRequests(state: opened, first: {PAGE_SIZE}) {{ nodes {{ ...status diffStats {{ path additions deletions }} }} }} }} }} \
            fragment status on MergeRequest {{ id approved approvedBy {{ nodes {{ username avatarUrl }} }} headPipeline {{ status }} }}"
        );
        let url = format!("{}/api/graphql", self.host.trim_end_matches('/'));
        let request = Request::post_json(url, &serde_json::json!({ "query": query }))?
            .header("PRIVATE-TOKEN", self.token.clone());
        let response: graphql::Response = decode(self.http.as_ref(), request).await?;
        Ok(response.details())
    }

    fn map(
        mr: MergeRequest,
        approvals: Approvals,
        detail: Detail,
        files: Option<Vec<DiffFile>>,
        authored: bool,
        account_id: &str,
        host: &str,
    ) -> InboxItem {
        let approvers: HashSet<String> =
            approvals.approved_by.iter().flatten().map(|a| a.user.username.clone()).collect();
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
        let avatar = |raw: &Option<String>| {
            raw.as_ref().filter(|s| !s.is_empty()).map(|s| {
                if s.starts_with('/') {
                    format!("{host}{s}")
                } else {
                    s.clone()
                }
            })
        };
        let mut participants: Vec<Person> = mr
            .reviewers
            .iter()
            .flatten()
            .map(|r| Person {
                name: r.username.clone(),
                avatar_url: avatar(&r.avatar_url),
                review: approvers.contains(&r.username).then_some(ReviewState::Approved),
                bot: false,
            })
            .collect();
        for approver in approvals.approved_by.iter().flatten() {
            if !participants.iter().any(|p| p.name == approver.user.username) {
                participants.push(Person {
                    name: approver.user.username.clone(),
                    avatar_url: avatar(&approver.user.avatar_url),
                    review: Some(ReviewState::Approved),
                    bot: false,
                });
            }
        }
        let project = mr
            .references
            .as_ref()
            .and_then(|r| r.full.split('!').next().map(str::to_string))
            .unwrap_or_else(|| format!("project {}", mr.project_id));
        InboxItem {
            id: InboxItem::make_id(account_id, mr.id),
            account_id: account_id.into(),
            plugin_id: "gitlab".into(),
            bundle: if authored { InboxBundle::authored() } else { InboxBundle::reviews() },
            title: remora_core::text::readable(&mr.title),
            context: format!("{project} !{}", mr.iid),
            preview: None,
            url: Some(mr.web_url),
            app_url: None,
            author: Some(Person {
                name: mr.author.username.clone(),
                avatar_url: avatar(&mr.author.avatar_url),
                review: None,
                bot: false,
            }),
            participants,
            badges: review.badges(authored),
            date: mr.updated_at,
            needs_action: review.needs_action(authored),
            priority: review.priority(authored),
            due: None,
            expires: None,
            changes: files.filter(|f| !f.is_empty()).map(|files| {
                let files = files.into_iter().map(|f| ChangedFile {
                    path: f.new_path.or(f.old_path).unwrap_or_else(|| "?".into()),
                    additions: f.additions,
                    deletions: f.deletions,
                });
                ChangeSet::new(files.collect(), None)
            }),
            suggested_people: None,
            recurrence: None,
            reactions: None,
            reason: None,
            related: None,
            stage: None,
            reach: None,
            followed_up: None,
        }
    }
}

#[async_trait]
impl SourcePlugin for GitLabPlugin {
    async fn fetch(&self) -> Result<SourceSnapshot, PluginError> {
        let user: User = self.cache.value("user", IDENTITY_LIFETIME, || self.get("user", &[])).await?;
        let per_page = PAGE_SIZE.to_string();
        let authored_query = [("scope", "created_by_me"), ("state", "opened"), ("per_page", per_page.as_str())];
        let reviewing_query = [
            ("scope", "all"),
            ("state", "opened"),
            ("per_page", per_page.as_str()),
            ("reviewer_username", user.username.as_str()),
        ];
        let todos_query = [("state", "pending"), ("per_page", per_page.as_str())];
        let (authored, reviewing, todos) = futures::join!(
            self.get::<Vec<MergeRequest>>("merge_requests", &authored_query),
            self.get::<Vec<MergeRequest>>("merge_requests", &reviewing_query),
            self.get::<Vec<Todo>>("todos", &todos_query)
        );
        let (authored, reviewing) = (authored?, reviewing?);
        // A full page means there may be more: GitLab's count header isn't always sent.
        let mut remarks = if authored.len() >= PAGE_SIZE || reviewing.len() >= PAGE_SIZE {
            vec![crate::truncated("GitLab")]
        } else {
            vec![]
        };
        // A merge request of yours where you are also a reviewer comes once, as yours: the same id twice would share
        // its Done, snooze and pin between two rows. Same on macOS.
        let mine: HashSet<u64> = authored.iter().map(|mr| mr.id).collect();
        let all: Vec<(MergeRequest, bool)> = authored
            .into_iter()
            .map(|mr| (mr, true))
            .chain(reviewing.into_iter().filter(|mr| !mine.contains(&mr.id)).map(|mr| (mr, false)))
            .collect();
        // One GraphQL request for every merge request's details; an instance where it fails gets the REST calls.
        let details = if all.is_empty() {
            HashMap::new()
        } else {
            match self.graphql_details().await {
                Ok(details) => details,
                Err(PluginError::RateLimited(at)) => return Err(PluginError::RateLimited(at)),
                Err(_) => HashMap::new(),
            }
        };
        let mut items = Vec::with_capacity(all.len());
        // To-Dos are a bonus, as on the Mac: their failure is a remark, and merge requests still come through.
        match todos {
            Ok(todos) => items.extend(todos.iter().filter_map(|t| t.item(&self.account_id))),
            Err(error) => match crate::rate_limit_of(&error) {
                Some(limit) => return Err(limit),
                None => remarks.push(crate::partial("To-Dos", &error)),
            },
        }
        let mut rest = vec![];
        for (mr, authored) in all {
            match details.get(&mr.id).cloned() {
                Some(found) => {
                    let files = if authored { None } else { found.files };
                    items.push(Self::map(
                        mr,
                        found.approvals,
                        found.detail,
                        files,
                        authored,
                        &self.account_id,
                        &self.host,
                    ));
                }
                None => rest.push((mr, authored)),
            }
        }
        let mut pending = rest.into_iter().peekable();
        let mut failed = None;
        while pending.peek().is_some() {
            let chunk: Vec<_> = pending.by_ref().take(4).collect();
            for (item, error) in join_all(chunk.into_iter().map(|(mr, authored)| self.item(mr, authored))).await {
                items.push(item);
                failed = failed.or(error);
            }
        }
        if let Some(error) = failed {
            if let Some(limit) = crate::rate_limit_of(&error) {
                return Err(limit);
            }
            // Without approvals, an approved merge request would wait as if it weren't: say so.
            remarks.push(crate::partial("Approvals, pipelines or changed files", &error));
        }
        for project in &self.projects {
            let (item, remark) = self.maintained(project).await?;
            items.extend(item);
            if let Some(remark) = remark.filter(|r| !remarks.contains(r)) {
                remarks.push(remark);
            }
        }
        Ok(SourceSnapshot { identity: user.username, items, remarks })
    }
}

#[derive(Deserialize)]
struct Project {
    id: u64,
    default_branch: Option<String>,
}

#[derive(Deserialize)]
struct BranchPipeline {
    id: u64,
    status: String,
    sha: String,
    web_url: String,
    updated_at: DateTime<Utc>,
}

#[derive(Deserialize)]
struct Commit {
    title: String,
}

impl BranchPipeline {
    /// The default branch is red: to fix, said by the checks badge (and notified once). Same as GitHub.
    fn item(self, account_id: &str, project: &str, branch: &str, commit: Option<Commit>) -> InboxItem {
        InboxItem {
            id: InboxItem::make_id(account_id, format!("ci/{project}/{}", self.id)),
            account_id: account_id.into(),
            plugin_id: "gitlab".into(),
            bundle: InboxBundle::fix(),
            title: commit.map_or_else(|| format!("Pipeline #{}", self.id), |c| remora_core::text::readable(&c.title)),
            context: format!("{project} · {branch}"),
            preview: None,
            url: Some(self.web_url),
            app_url: None,
            author: None,
            participants: vec![],
            badges: vec![Badge::new(badge::CHECKS_FAILING, "Checks failed", Tone::Negative).notifying("Checks failed")],
            date: self.updated_at,
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
            reach: None,
            followed_up: None,
        }
    }
}

/// A GitLab To-Do: an issue assigned to you, a mention, something you marked. Merge requests are left out, already
/// shown with their approvals and pipelines.
#[derive(Deserialize)]
struct Todo {
    id: u64,
    action_name: String,
    target_type: String,
    target: Option<TodoTarget>,
    target_url: Option<String>,
    body: Option<String>,
    author: Option<TodoAuthor>,
    project: Option<TodoProject>,
    updated_at: Option<DateTime<Utc>>,
    created_at: Option<DateTime<Utc>>,
}

#[derive(Deserialize)]
struct TodoTarget {
    title: Option<String>,
    iid: Option<u64>,
}

#[derive(Deserialize)]
struct TodoAuthor {
    name: Option<String>,
    username: Option<String>,
    avatar_url: Option<String>,
}

#[derive(Deserialize)]
struct TodoProject {
    path_with_namespace: Option<String>,
}

impl Todo {
    fn item(&self, account_id: &str) -> Option<InboxItem> {
        if self.target_type == "MergeRequest" {
            return None;
        }
        let mention = matches!(self.action_name.as_str(), "mentioned" | "directly_addressed");
        let target = self.target.as_ref();
        let title = target.and_then(|t| t.title.clone()).or_else(|| self.body.clone()).unwrap_or_default();
        let project = self.project.as_ref().and_then(|p| p.path_with_namespace.clone()).unwrap_or_default();
        let context = match target.and_then(|t| t.iid) {
            Some(iid) if self.target_type == "Issue" => format!("{project} #{iid}"),
            _ => project,
        };
        let preview = if mention { self.body.as_deref().map(remora_core::text::readable) } else { None };
        Some(InboxItem {
            id: InboxItem::make_id(account_id, format!("todo/{}", self.id)),
            account_id: account_id.into(),
            plugin_id: "gitlab".into(),
            bundle: if mention { InboxBundle::mentions() } else { InboxBundle::tasks() },
            title: remora_core::text::readable(&title),
            context,
            preview: preview.filter(|p| !p.is_empty() && *p != title),
            url: self.target_url.clone(),
            app_url: None,
            author: self.author.as_ref().and_then(|a| {
                let name = a.username.clone().or_else(|| a.name.clone())?;
                Some(Person { name, avatar_url: a.avatar_url.clone(), review: None, bot: false })
            }),
            participants: vec![],
            badges: vec![],
            date: self.updated_at.or(self.created_at).unwrap_or_else(Utc::now),
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
            reach: None,
            followed_up: None,
        })
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
                let approved_by = node
                    .approved_by
                    .and_then(|p| p.nodes)
                    .unwrap_or_default()
                    .into_iter()
                    .flatten()
                    .map(|u| Approver { user: User { username: u.username, avatar_url: u.avatar_url } })
                    .collect();
                let approvals = Approvals { approved: node.approved, approved_by: Some(approved_by) };
                let detail =
                    Detail { head_pipeline: node.head_pipeline.map(|p| Pipeline { status: p.status.to_lowercase() }) };
                let files = node.diff_stats.map(|stats| {
                    stats
                        .into_iter()
                        .map(|s| DiffFile {
                            new_path: Some(s.path),
                            old_path: None,
                            additions: s.additions,
                            deletions: s.deletions,
                        })
                        .collect()
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
pub(crate) mod tests {
    use super::*;
    use crate::stub::*;

    const MR: &str = r#"{"id": 101, "iid": 12, "project_id": 5, "title": "Speed up CI", "web_url": "https://gitlab.acme.io/g/p/-/merge_requests/12",
     "draft": false, "updated_at": "2026-10-08T10:00:00.000Z", "author": {"username": "alice", "avatar_url": "/uploads/a.png"},
     "reviewers": [{"username": "erin"}, {"username": "frank"}], "user_notes_count": 2, "has_conflicts": false,
     "references": {"full": "g/p!12"}, "detailed_merge_status": "mergeable"}"#;

    /// The merge request to review: `MR` under another id, so its details come from the same routes.
    fn review() -> String {
        MR.replace(r#""id": 101"#, r#""id": 102"#)
    }

    #[tokio::test]
    async fn maps_merge_requests_with_approvals_and_pipeline() {
        let (mine, others) = (format!("[{MR}]"), format!("[{}]", review()));
        let http = StubHttp::paths(&[
            ("/api/v4/user", r#"{"username": "alice"}"#),
            ("/api/v4/merge_requests?scope=created_by_me", &mine),
            ("/api/v4/merge_requests?reviewer_username=", &others),
            ("/merge_requests/12/approvals", r#"{"approved": true, "approved_by": [{"user": {"username": "erin"}}]}"#),
            ("/projects/5/merge_requests/12", r#"{"head_pipeline": {"status": "success"}}"#),
            (
                "/merge_requests/12/diffs",
                r#"[{"new_path": "db/migrate/add_avatars.rb", "old_path": "db/migrate/add_avatars.rb", "diff": "+ secret code"}]"#,
            ),
        ]);
        let plugin = GitLabPlugin::new(&config(&[("host", "gitlab.acme.io"), ("token", "t")]), Arc::new(http)).unwrap();
        let snapshot = plugin.fetch().await.unwrap();

        assert_eq!(snapshot.identity, "alice");
        assert_eq!(
            snapshot.items.iter().map(|i| i.bundle.clone()).collect::<Vec<_>>(),
            [InboxBundle::authored(), InboxBundle::reviews()]
        );
        let item = &snapshot.items[0];
        assert_eq!(item.context, "g/p !12");
        assert!(item.has_badge("approved") && item.has_badge("checks.passing") && item.has_badge("comments"));
        assert_eq!(
            item.participants.iter().find(|p| p.name == "erin").and_then(|p| p.review),
            Some(ReviewState::Approved)
        );
        assert_eq!(item.participants.iter().find(|p| p.name == "frank").and_then(|p| p.review), None);
        assert_eq!(
            item.author.as_ref().and_then(|a| a.avatar_url.clone()).as_deref(),
            Some("https://gitlab.acme.io/uploads/a.png")
        );
        assert_eq!(item.changes, None, "your own merge requests don't need a review prep");
        let review = &snapshot.items[1];
        let changes = review.changes.as_ref().unwrap();
        assert_eq!(
            changes.files,
            [ChangedFile { path: "db/migrate/add_avatars.rb".into(), additions: None, deletions: None }]
        );
        assert_eq!(changes.file_count, 1);
        let stored = serde_json::to_string(review).unwrap();
        assert!(!stored.contains("secret code"), "diff text is never kept");
    }

    /// Approvals that can't be read are said, not passed over; a rate limit there fails the fetch, so the
    /// backoff waits it out and the last items stay.
    #[tokio::test]
    async fn failed_details_are_said_and_a_rate_limit_is_waited_out() {
        struct Limited(StubHttp);
        #[async_trait::async_trait]
        impl crate::HttpClient for Limited {
            async fn send(&self, request: crate::Request) -> Result<crate::Response, PluginError> {
                if request.url.contains("/approvals") {
                    return Ok(crate::Response { status: 429, body: String::new(), etag: None, reset: None });
                }
                self.0.send(request).await
            }
        }
        let (mine, others) = (format!("[{MR}]"), format!("[{}]", review()));
        let routes = [
            ("/api/v4/user", r#"{"username": "alice"}"#),
            ("/api/v4/merge_requests?scope=created_by_me", mine.as_str()),
            ("/api/v4/merge_requests?reviewer_username=", others.as_str()),
            ("/todos", "[]"),
        ];
        let plugin = |http: Arc<dyn crate::HttpClient>| {
            GitLabPlugin::new(&config(&[("host", "gitlab.acme.io"), ("token", "t")]), http).unwrap()
        };
        // No approvals, pipeline or diffs routes: each answers 404.
        let snapshot = plugin(Arc::new(StubHttp::paths(&routes))).fetch().await.unwrap();
        assert_eq!(snapshot.items.len(), 2, "the merge requests still come");
        assert_eq!(snapshot.remarks.len(), 1);
        assert_eq!(snapshot.remarks[0].code, "partial");
        assert_eq!(snapshot.remarks[0].args[0], "Approvals, pipelines or changed files");

        let limited = plugin(Arc::new(Limited(StubHttp::paths(&routes)))).fetch().await;
        assert_eq!(limited.err(), Some(PluginError::RateLimited(None)));
    }

    /// Who the token belongs to is asked once, not at every refresh.
    #[tokio::test]
    async fn the_user_is_asked_once_across_refreshes() {
        let stub = StubHttp::paths(&[("/api/v4/user", r#"{"username": "alice"}"#), ("/api/v4/merge_requests", "[]")]);
        let http = Arc::new(Recording::new(stub));
        let config = config(&[("host", "gitlab.acme.io"), ("token", "t")]);
        for _ in 0..2 {
            GitLabPlugin::new(&config, http.clone()).unwrap().fetch().await.unwrap();
        }
        assert_eq!(http.paths().iter().filter(|p| p.ends_with("/user")).count(), 1);
    }

    /// A merge request of yours where you are also a reviewer comes once, as yours. Same on macOS.
    #[tokio::test]
    async fn your_merge_request_you_also_review_comes_once() {
        let list = format!("[{MR}]");
        let http = StubHttp::paths(&[("/api/v4/user", r#"{"username": "alice"}"#), ("/api/v4/merge_requests", &list)]);
        let plugin = GitLabPlugin::new(&config(&[("host", "gitlab.acme.io"), ("token", "t")]), Arc::new(http)).unwrap();
        let snapshot = plugin.fetch().await.unwrap();
        assert_eq!(snapshot.items.iter().map(|i| i.bundle.clone()).collect::<Vec<_>>(), [InboxBundle::authored()]);
    }

    /// The user, the lists, the To-Dos and one GraphQL request for the details.
    fn graphql_stub() -> StubHttp {
        let (mine, others) = (format!("[{MR}]"), format!("[{}]", review()));
        let graphql = r#"{"data": {"currentUser": {
          "authored": {"nodes": [{"id": "gid://gitlab/MergeRequest/101", "approved": true,
            "approvedBy": {"nodes": [{"username": "erin", "avatarUrl": null}]}, "headPipeline": {"status": "FAILED"}}]},
          "reviewing": {"nodes": [{"id": "gid://gitlab/MergeRequest/102", "approved": true,
            "approvedBy": {"nodes": [{"username": "erin"}]}, "headPipeline": {"status": "FAILED"},
            "diffStats": [{"path": "app/models/user.rb", "additions": 12, "deletions": 3}]}]}}}}"#;
        StubHttp::paths(&[
            ("/api/v4/user", r#"{"username": "alice"}"#),
            ("/api/v4/merge_requests?scope=created_by_me", &mine),
            ("/api/v4/merge_requests?reviewer_username=", &others),
            ("/api/graphql", graphql),
            ("/api/v4/todos", "[]"),
        ])
    }

    /// The fetch `registry`'s egress test runs through the guard: GitLab.com, a declared host.
    pub(crate) fn egress_fixture() -> EgressFixture {
        EgressFixture { settings: vec![("host", "gitlab.com"), ("token", "t")], http: Arc::new(graphql_stub()) }
    }

    /// One GraphQL request instead of two REST calls per merge request.
    #[tokio::test]
    async fn details_come_from_one_graphql_request() {
        let http = Arc::new(Recording::new(graphql_stub()));
        let plugin = GitLabPlugin::new(&config(&[("host", "gitlab.acme.io"), ("token", "t")]), http.clone()).unwrap();
        let snapshot = plugin.fetch().await.unwrap();

        let paths = http.paths();
        assert!(paths.iter().all(|p| !p.contains("/projects/")), "no per-merge-request call: {paths:?}");
        assert_eq!(paths.len(), 5, "user, the two lists, the To-Dos, one GraphQL request");
        let item = &snapshot.items[0];
        assert!(item.has_badge("approved") && item.has_badge("checks.failing"));
        assert_eq!(
            item.participants.iter().find(|p| p.name == "erin").and_then(|p| p.review),
            Some(ReviewState::Approved)
        );
        assert_eq!(item.changes, None, "your own merge requests don't need a review prep");
        let review = &snapshot.items[1];
        assert_eq!(review.bundle, InboxBundle::reviews());
        assert_eq!(
            review.changes.as_ref().map(|c| c.files.clone()),
            Some(vec![ChangedFile { path: "app/models/user.rb".into(), additions: Some(12), deletions: Some(3) }])
        );
    }

    /// To-Dos other than merge requests, already listed: assigned issues to do, mentions to answer.
    #[tokio::test]
    async fn to_dos_bring_issues_and_mentions_but_not_merge_requests_again() {
        let todos = r#"[
          {"id": 1, "action_name": "assigned", "target_type": "Issue",
           "target": {"title": "Export *CSV*", "iid": 42}, "target_url": "https://gitlab.acme.io/acme/app/-/issues/42",
           "author": {"name": "Erin", "username": "erin", "avatar_url": null}, "project": {"path_with_namespace": "acme/app"},
           "updated_at": "2026-10-12T09:00:00Z"},
          {"id": 2, "action_name": "mentioned", "target_type": "Issue", "target": {"title": "Login loop", "iid": 7},
           "body": "@alice can you look?", "target_url": "https://gitlab.acme.io/acme/app/-/issues/7#note_1",
           "author": {"username": "frank"}, "project": {"path_with_namespace": "acme/app"}, "created_at": "2026-10-12T10:00:00Z"},
          {"id": 3, "action_name": "review_requested", "target_type": "MergeRequest", "target": {"title": "Fix", "iid": 3}}
        ]"#;
        let stub = StubHttp::paths(&[
            ("/api/v4/user", r#"{"username": "alice"}"#),
            ("/api/v4/merge_requests", "[]"),
            ("/api/v4/todos", todos),
        ]);
        let snapshot = GitLabPlugin::new(&config(&[("host", "gitlab.acme.io"), ("token", "t")]), Arc::new(stub))
            .unwrap()
            .fetch()
            .await
            .unwrap();
        assert_eq!(snapshot.items.len(), 2, "the merge request is already listed as such");
        let issue = &snapshot.items[0];
        assert_eq!(
            (issue.bundle.clone(), issue.title.as_str(), issue.context.as_str()),
            (InboxBundle::tasks(), "Export CSV", "acme/app #42")
        );
        let mention = &snapshot.items[1];
        assert_eq!(mention.bundle, InboxBundle::mentions());
        assert_eq!(mention.preview.as_deref(), Some("@alice can you look?"));
        assert_eq!(mention.author.as_ref().map(|a| a.name.as_str()), Some("frank"));
        assert!(snapshot.remarks.is_empty());
    }

    /// A project whose default branch's last pipeline failed is to fix; a passing one is nothing; one that can't be
    /// read is a remark. Same cases as macOS.
    #[tokio::test]
    async fn a_failed_pipeline_on_the_default_branch_is_to_fix() {
        let fetch = |pipeline: &'static str| async move {
            let http = StubHttp::paths(&[
                ("/api/v4/user", r#"{"username": "alice"}"#),
                ("/api/v4/merge_requests", "[]"),
                ("/api/v4/todos", "[]"),
                ("/api/v4/projects/acme%2Ftools%2Fcli", r#"{"id": 7, "default_branch": "main"}"#),
                ("/api/v4/projects/7/pipelines?ref=main", pipeline),
                ("/api/v4/projects/7/repository/commits/abc", r#"{"title": "Bump *ruby*"}"#),
            ]);
            let config =
                config(&[("host", "gitlab.acme.io"), ("token", "t"), ("repositories", "acme/tools/cli, acme/gone")]);
            GitLabPlugin::new(&config, Arc::new(http)).unwrap().fetch().await.unwrap()
        };
        let red = fetch(r#"[{"id": 9, "status": "failed", "sha": "abc", "web_url": "https://gitlab.acme.io/acme/tools/cli/-/pipelines/9", "updated_at": "2026-10-08T12:00:00Z"}]"#).await;
        assert_eq!(red.items.len(), 1);
        let item = &red.items[0];
        assert_eq!((item.title.as_str(), item.context.as_str()), ("Bump ruby", "acme/tools/cli · main"));
        assert_eq!(item.bundle, InboxBundle::fix());
        assert!(item.has_badge(badge::CHECKS_FAILING) && item.needs_action);
        assert_eq!(red.remarks.len(), 1, "acme/gone can't be read");
        assert_eq!(red.remarks[0].args[0], "Projects you maintain");

        let green = fetch(
            r#"[{"id": 9, "status": "success", "sha": "abc", "web_url": "u", "updated_at": "2026-10-08T12:00:00Z"}]"#,
        )
        .await;
        assert!(green.items.is_empty());
    }
}

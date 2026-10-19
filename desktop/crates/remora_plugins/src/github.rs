use crate::code_review::{Checks, CodeReview};
use crate::{
    decode, decode_with_reset, maintained_repositories, partial, rate_limit_of, url_with_query, HttpClient,
    PluginConfig, PluginError, Request, SourcePlugin, SourceSnapshot,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use remora_core::ReviewState;
use remora_core::{
    badge, Badge, ChangeSet, ChangedFile, ConfigField, Egress, InboxBundle, InboxItem, Message as Remark, Person,
    PluginManifest, Priority, Tone,
};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;

pub fn manifest() -> PluginManifest {
    PluginManifest {
        id: "github".into(),
        name: "GitHub".into(),
        summary: "Pull requests you opened, review requests and issues assigned to you. Works with GitHub Enterprise."
            .into(),
        fields: vec![
            ConfigField::host("https://github.com"),
            ConfigField::token(
                "Personal access token",
                "A fine-grained token with read-only Pull requests and Issues, or a classic token with repo and read:org.",
            ),
            ConfigField::text(
                "repositories",
                "Repositories I maintain",
                "acme/web, acme/api",
                "Optional. A failing default branch and open critical or high security alerts go to To fix. The token \
                 also needs read-only Actions, Dependabot alerts and Code scanning alerts.",
            ),
        ],
        setup_steps: vec![
            // Read-only and limited to what Remora reads, rather than the whole `repo` scope.
            "Click “Create a token”: GitHub opens a fine-grained token named Remora, with read-only Pull requests and Issues.".into(),
            "Pick the owner (you, or your organization) and All repositories, set an expiration, generate it and paste it below.".into(),
            "Repositories of several organizations: connect GitHub once per organization, or use a classic token with repo and read:org.".into(),
            "To see the checks and security alerts of repositories you maintain, also give the token read-only Actions, Dependabot alerts and Code scanning alerts.".into(),
        ],
        setup_label: "Create a token".into(),
        setup_url: Some("{host}/settings/personal-access-tokens/new?name=Remora&description=Remora%20reads%20your%20pull%20requests%2C%20reviews%20and%20issues&pull_requests=read&issues=read&metadata=read".into()),
        egress: Egress {
            hosts: vec!["api.github.com".into(), "github.com".into(), "avatars.githubusercontent.com".into()],
            description: "Reads your pull requests, review requests and assigned issues from GitHub (or your GitHub \
                          Enterprise host), and the checks and security alerts of the repositories you maintain."
                .into(),
            external_ai: false,
        },
        logo: Some("github".into()),
        sign_in: None,
    }
}

/// Pull requests you opened and those awaiting your review, through a single GraphQL query; for the repositories you
/// maintain, a failing default branch and open critical or high security alerts, through the REST API.
pub struct GitHubPlugin {
    account_id: String,
    endpoint: String,
    /// The REST API: api.github.com, or the Enterprise host's /api/v3.
    rest: String,
    token: String,
    repositories: Vec<String>,
    http: Arc<dyn HttpClient>,
}

/// The security alerts listed per repository, most recent first.
const ALERTS_PER_REPOSITORY: usize = 10;

impl GitHubPlugin {
    pub fn new(config: &PluginConfig, http: Arc<dyn HttpClient>) -> Result<Self, PluginError> {
        let host = config.url("host")?;
        let endpoint = if host.ends_with("://github.com") {
            "https://api.github.com/graphql".to_string()
        } else {
            format!("{host}/api/graphql")
        };
        let rest = if host.ends_with("://github.com") {
            "https://api.github.com".to_string()
        } else {
            format!("{host}/api/v3")
        };
        let repositories = maintained_repositories(config.values.get("repositories").map_or("", String::as_str), false);
        Ok(GitHubPlugin {
            account_id: config.account_id.clone(),
            endpoint,
            rest,
            token: config.required("token")?,
            repositories,
            http,
        })
    }

    /// `reset`: when GitHub's rate limit resets, from the answer's headers.
    pub fn snapshot(
        response: GraphQlResponse,
        account_id: &str,
        reset: Option<i64>,
    ) -> Result<SourceSnapshot, PluginError> {
        // GraphQL points used up: a 200 whose error says so.
        if response.errors.as_ref().is_some_and(|e| e.iter().any(|e| e.kind.as_deref() == Some("RATE_LIMITED"))) {
            return Err(PluginError::RateLimited(reset));
        }
        let Some(data) = response.data else {
            let message = response
                .errors
                .and_then(|e| e.into_iter().next())
                .map(|e| e.message)
                .unwrap_or_else(|| "GitHub returned no data.".into());
            // A token GitHub no longer accepts: the user reconnects.
            if message.eq_ignore_ascii_case("Bad credentials") {
                return Err(PluginError::Unauthorized);
            }
            return Err(PluginError::Api(message));
        };
        let cut =
            [&data.authored, &data.reviewing].iter().any(|s| s.issue_count.is_some_and(|n| n as usize > s.nodes.len()))
                || data.assigned.issue_count.is_some_and(|n| n as usize > data.assigned.nodes.len());
        let assigned = data.assigned.nodes.into_iter().flatten().map(|issue| issue.item(account_id));
        let authored =
            data.authored.nodes.into_iter().flatten().map(|pr| pr.item(account_id, InboxBundle::authored(), true));
        let reviewing =
            data.reviewing.nodes.into_iter().flatten().map(|pr| pr.item(account_id, InboxBundle::reviews(), false));
        let remarks = if cut { vec![crate::truncated("GitHub")] } else { vec![] };
        Ok(SourceSnapshot {
            identity: data.viewer.login,
            items: authored.chain(reviewing).chain(assigned).collect(),
            remarks,
        })
    }

    const QUERY: &'static str = r#"
    query($authored: String!, $reviewing: String!, $assigned: String!) {
      viewer { login }
      authored: search(query: $authored, type: ISSUE, first: 50) { issueCount nodes { ...PR } }
      reviewing: search(query: $reviewing, type: ISSUE, first: 50) { issueCount nodes { ...PR } }
      assigned: search(query: $assigned, type: ISSUE, first: 50) { issueCount nodes { ...Issue } }
    }
    fragment Issue on Issue {
      id number title url updatedAt
      repository { nameWithOwner }
      author { __typename login avatarUrl }
      closedByPullRequestsReferences(first: 5) { nodes { url } }
    }
    fragment PR on PullRequest {
      id number title url isDraft updatedAt additions deletions mergeable reviewDecision
      repository { nameWithOwner }
      author { __typename login avatarUrl }
      comments { totalCount }
      latestOpinionatedReviews(first: 20) { nodes { state author { __typename login avatarUrl } } }
      reviewRequests(first: 20) {
        nodes { requestedReviewer { ... on User { login avatarUrl } ... on Team { name avatarUrl } } }
      }
      commits(last: 1) { nodes { commit { statusCheckRollup { state } } } }
      suggestedReviewers { reviewer { login avatarUrl } }
      changedFiles
      files(first: 50) { nodes { path additions deletions } }
    }
    "#;
}

#[async_trait]
impl SourcePlugin for GitHubPlugin {
    async fn fetch(&self) -> Result<SourceSnapshot, PluginError> {
        let variables = HashMap::from([
            ("authored", "is:pr is:open archived:false author:@me sort:updated-desc"),
            ("reviewing", "is:pr is:open archived:false review-requested:@me sort:updated-desc"),
            ("assigned", "is:issue is:open archived:false assignee:@me sort:updated-desc"),
        ]);
        let body = serde_json::json!({ "query": Self::QUERY, "variables": variables });
        let request =
            Request::post_json(&self.endpoint, &body)?.header("Authorization", format!("bearer {}", self.token));
        let (response, reset) = decode_with_reset::<GraphQlResponse>(self.http.as_ref(), request).await?;
        let mut snapshot = Self::snapshot(response, &self.account_id, reset)?;
        for repository in &self.repositories {
            let (items, remarks) = self.maintained(repository).await?;
            snapshot.items.extend(items);
            for remark in remarks {
                if !snapshot.remarks.contains(&remark) {
                    snapshot.remarks.push(remark);
                }
            }
        }
        Ok(snapshot)
    }
}

impl GitHubPlugin {
    async fn get<T: serde::de::DeserializeOwned>(
        &self,
        repository: &str,
        path: &str,
        query: &[(&str, &str)],
    ) -> Result<T, PluginError> {
        let path = if path.is_empty() { format!("repos/{repository}") } else { format!("repos/{repository}/{path}") };
        let url = url_with_query(&self.rest, &path, query)?;
        let request = Request::get(url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github+json");
        decode(self.http.as_ref(), request).await
    }

    /// A repository you maintain: its default branch's last finished run when it failed, and its open critical or
    /// high security alerts (Dependabot, and code scanning where it is set up). Best effort: a part that can't be
    /// read is a remark, except a rate limit, which fails the fetch.
    async fn maintained(&self, repository: &str) -> Result<(Vec<InboxItem>, Vec<Remark>), PluginError> {
        let (mut items, mut remarks) = (vec![], vec![]);
        let note = |what: &str, error: PluginError, remarks: &mut Vec<Remark>| match rate_limit_of(&error) {
            Some(limit) => Err(limit),
            None => {
                remarks.push(partial(what, &error));
                Ok(())
            }
        };
        let info: Info = match self.get(repository, "", &[]).await {
            Ok(info) => info,
            Err(error) => {
                note("Repositories you maintain", error, &mut remarks)?;
                return Ok((items, remarks));
            }
        };
        let branch = info.default_branch;
        let runs = self
            .get::<Runs>(
                repository,
                "actions/runs",
                &[("branch", &branch), ("status", "completed"), ("per_page", "1"), ("exclude_pull_requests", "true")],
            )
            .await;
        match runs {
            Ok(runs) => items.extend(
                runs.workflow_runs
                    .into_iter()
                    .next()
                    .and_then(|run| run.failing(&self.account_id, repository, &branch)),
            ),
            // No Actions on this repository.
            Err(PluginError::Status(404)) => {}
            Err(error) => note("Checks on the default branch", error, &mut remarks)?,
        }
        let per_page = ALERTS_PER_REPOSITORY.to_string();
        let severe = [("state", "open"), ("severity", "critical,high"), ("per_page", per_page.as_str())];
        match self.get::<Vec<DependabotAlert>>(repository, "dependabot/alerts", &severe).await {
            Ok(alerts) => items.extend(alerts.into_iter().map(|a| a.item(&self.account_id, repository))),
            Err(error) => note("Security alerts", error, &mut remarks)?,
        }
        match self.get::<Vec<CodeScanningAlert>>(repository, "code-scanning/alerts", &severe).await {
            Ok(alerts) => items.extend(alerts.into_iter().map(|a| a.item(&self.account_id, repository))),
            // Code scanning needs GitHub Advanced Security, often off: no remark at every refresh.
            Err(PluginError::Status(403 | 404)) => {}
            Err(error) => note("Security alerts", error, &mut remarks)?,
        }
        Ok((items, remarks))
    }
}

#[derive(Deserialize)]
struct Info {
    default_branch: String,
}

#[derive(Deserialize)]
struct Runs {
    #[serde(default)]
    workflow_runs: Vec<Run>,
}

#[derive(Deserialize)]
struct Run {
    id: u64,
    name: Option<String>,
    display_title: Option<String>,
    conclusion: Option<String>,
    html_url: String,
    updated_at: DateTime<Utc>,
}

impl Run {
    /// The default branch is red: to fix, said by the checks badge (and notified once).
    fn failing(self, account_id: &str, repository: &str, branch: &str) -> Option<InboxItem> {
        if !matches!(self.conclusion.as_deref(), Some("failure" | "timed_out" | "startup_failure")) {
            return None;
        }
        let title = self.display_title.or(self.name.clone()).unwrap_or_else(|| branch.to_string());
        Some(maintained_item(
            account_id,
            format!("ci/{repository}/{}", self.id),
            remora_core::text::readable(&title),
            format!("{repository} · {branch}"),
            self.name,
            self.html_url,
            self.updated_at,
            None,
            vec![Badge::new(badge::CHECKS_FAILING, "Checks failed", Tone::Negative).notifying("Checks failed")],
        ))
    }
}

#[derive(Deserialize)]
struct DependabotAlert {
    number: u64,
    html_url: String,
    created_at: DateTime<Utc>,
    security_advisory: Advisory,
    dependency: Option<Dependency>,
}

#[derive(Deserialize)]
struct Advisory {
    summary: String,
    severity: Option<String>,
}

#[derive(Deserialize)]
struct Dependency {
    package: Option<Package>,
}

#[derive(Deserialize)]
struct Package {
    name: String,
}

impl DependabotAlert {
    fn item(self, account_id: &str, repository: &str) -> InboxItem {
        let package = self.dependency.and_then(|d| d.package).map(|p| p.name);
        let title = match package {
            Some(package) => format!("{package}: {}", self.security_advisory.summary),
            None => self.security_advisory.summary,
        };
        maintained_item(
            account_id,
            format!("alert/{repository}/dependabot/{}", self.number),
            title,
            repository.into(),
            None,
            self.html_url,
            self.created_at,
            severity(self.security_advisory.severity.as_deref()),
            vec![],
        )
    }
}

#[derive(Deserialize)]
struct CodeScanningAlert {
    number: u64,
    html_url: String,
    created_at: DateTime<Utc>,
    rule: Rule,
}

#[derive(Deserialize)]
struct Rule {
    description: Option<String>,
    security_severity_level: Option<String>,
}

impl CodeScanningAlert {
    fn item(self, account_id: &str, repository: &str) -> InboxItem {
        maintained_item(
            account_id,
            format!("alert/{repository}/code-scanning/{}", self.number),
            self.rule.description.unwrap_or_else(|| format!("Code scanning alert #{}", self.number)),
            repository.into(),
            None,
            self.html_url,
            self.created_at,
            severity(self.rule.security_severity_level.as_deref()),
            vec![],
        )
    }
}

/// Critical alerts are urgent, high ones high.
fn severity(level: Option<&str>) -> Option<Priority> {
    match level {
        Some("critical") => Some(Priority::Urgent),
        Some("high") => Some(Priority::High),
        _ => None,
    }
}

/// Something to fix in a repository you maintain.
#[allow(clippy::too_many_arguments)]
fn maintained_item(
    account_id: &str,
    local_id: String,
    title: String,
    context: String,
    preview: Option<String>,
    url: String,
    date: DateTime<Utc>,
    priority: Option<Priority>,
    badges: Vec<Badge>,
) -> InboxItem {
    InboxItem {
        id: InboxItem::make_id(account_id, &local_id),
        account_id: account_id.into(),
        plugin_id: "github".into(),
        bundle: InboxBundle::fix(),
        title,
        context,
        preview,
        url: Some(url),
        app_url: None,
        author: None,
        participants: vec![],
        badges,
        date,
        needs_action: true,
        priority,
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

#[derive(Deserialize)]
pub struct GraphQlResponse {
    data: Option<Payload>,
    errors: Option<Vec<Message>>,
}

#[derive(Deserialize)]
struct Message {
    message: String,
    #[serde(rename = "type")]
    kind: Option<String>,
}

#[derive(Deserialize)]
struct Payload {
    viewer: Login,
    authored: Search,
    reviewing: Search,
    /// Issues assigned to you. Absent from an answer to the older query.
    #[serde(default)]
    assigned: IssueSearch,
}

#[derive(Deserialize, Default)]
struct IssueSearch {
    #[serde(rename = "issueCount", default)]
    issue_count: Option<u32>,
    #[serde(default = "Vec::new")]
    nodes: Vec<Option<Issue>>,
}

/// An issue assigned to you: something to do, like a Linear or Jira issue.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Issue {
    id: String,
    number: u64,
    title: String,
    url: String,
    updated_at: DateTime<Utc>,
    repository: Repository,
    author: Option<Actor>,
    /// The open pull requests that will close it (linked in the sidebar or "Fixes #12").
    #[serde(default)]
    closed_by_pull_requests_references: Connection<Link>,
}

#[derive(Deserialize, Default)]
struct Link {
    url: String,
}

impl Issue {
    /// Waits on the review of the pull requests that will close it, when there are.
    fn item(self, account_id: &str) -> InboxItem {
        let changes: Vec<String> =
            self.closed_by_pull_requests_references.nodes.into_iter().flatten().map(|link| link.url).collect();
        let item = InboxItem {
            id: InboxItem::make_id(account_id, &self.id),
            account_id: account_id.into(),
            plugin_id: "github".into(),
            bundle: InboxBundle::tasks(),
            title: remora_core::text::readable(&self.title),
            context: format!("{} #{}", self.repository.name_with_owner, self.number),
            preview: None,
            url: Some(self.url),
            app_url: None,
            author: self.author.as_ref().map(Actor::person),
            participants: vec![],
            badges: vec![],
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
        };
        if changes.is_empty() {
            item
        } else {
            item.waiting_on_review(changes)
        }
    }
}

#[derive(Deserialize)]
struct Login {
    login: String,
}

#[derive(Deserialize)]
struct Search {
    /// All the matches, beyond the 50 listed.
    #[serde(rename = "issueCount", default)]
    issue_count: Option<u32>,
    nodes: Vec<Option<PullRequest>>,
}

#[derive(Deserialize, Default)]
struct Connection<T> {
    #[serde(default = "Vec::new")]
    nodes: Vec<Option<T>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Actor {
    /// "Bot" for an app account (Dependabot, Renovate, an agent): GitHub says it, no name is guessed.
    #[serde(rename = "__typename")]
    typename: Option<String>,
    login: Option<String>,
    name: Option<String>,
    avatar_url: Option<String>,
}

impl Actor {
    fn person(&self) -> Person {
        Person {
            name: self.login.clone().or_else(|| self.name.clone()).unwrap_or_else(|| "?".into()),
            avatar_url: self.avatar_url.clone(),
            review: None,
            bot: self.typename.as_deref() == Some("Bot") || self.login.as_deref().is_some_and(|l| l.ends_with("[bot]")),
        }
    }
}

#[derive(Deserialize)]
struct Review {
    state: String,
    author: Option<Actor>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReviewRequest {
    requested_reviewer: Option<Actor>,
}

#[derive(Deserialize)]
struct CommitNode {
    commit: Rollup,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Rollup {
    status_check_rollup: Option<State>,
}

#[derive(Deserialize)]
struct State {
    state: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Count {
    total_count: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Repository {
    name_with_owner: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PullRequest {
    id: String,
    number: u32,
    title: String,
    url: String,
    is_draft: bool,
    updated_at: DateTime<Utc>,
    additions: Option<u32>,
    deletions: Option<u32>,
    mergeable: Option<String>,
    review_decision: Option<String>,
    repository: Repository,
    author: Option<Actor>,
    comments: Option<Count>,
    latest_opinionated_reviews: Option<Connection<Review>>,
    review_requests: Option<Connection<ReviewRequest>>,
    commits: Option<Connection<CommitNode>>,
    suggested_reviewers: Option<Vec<Option<Suggested>>>,
    changed_files: Option<usize>,
    files: Option<Connection<File>>,
}

#[derive(Deserialize)]
struct File {
    path: String,
    additions: Option<u32>,
    deletions: Option<u32>,
}

#[derive(Deserialize)]
struct Suggested {
    reviewer: Option<Actor>,
}

/// GitHub gives no `reviewDecision` when the repository requires no review: the latest reviews tell then, a request
/// for changes first.
fn decision<'a>(states: impl Iterator<Item = &'a str> + Clone) -> Option<&'static str> {
    if states.clone().any(|s| s == "CHANGES_REQUESTED") {
        Some("CHANGES_REQUESTED")
    } else if states.into_iter().any(|s| s == "APPROVED") {
        Some("APPROVED")
    } else {
        None
    }
}

impl PullRequest {
    /// Paths and line counts only, for the review prep.
    fn change_set(&self) -> Option<ChangeSet> {
        let files: Vec<ChangedFile> = self
            .files
            .iter()
            .flat_map(|c| c.nodes.iter().flatten())
            .map(|f| ChangedFile { path: f.path.clone(), additions: f.additions, deletions: f.deletions })
            .collect();
        (!files.is_empty()).then(|| ChangeSet::new(files, self.changed_files))
    }

    fn item(self, account_id: &str, bundle: InboxBundle, authored: bool) -> InboxItem {
        let checks = match self
            .commits
            .as_ref()
            .and_then(|c| c.nodes.iter().flatten().last())
            .and_then(|c| c.commit.status_check_rollup.as_ref())
            .map(|s| s.state.as_str())
        {
            Some("SUCCESS") => Checks::Passing,
            Some("FAILURE") | Some("ERROR") => Checks::Failing,
            Some("PENDING") | Some("EXPECTED") => Checks::Running,
            _ => Checks::None,
        };
        let states =
            self.latest_opinionated_reviews.iter().flat_map(|c| c.nodes.iter().flatten()).map(|r| r.state.as_str());
        let decision = self.review_decision.as_deref().or_else(|| decision(states));
        let review = CodeReview {
            is_draft: self.is_draft,
            is_approved: decision == Some("APPROVED"),
            changes_requested: decision == Some("CHANGES_REQUESTED"),
            has_conflicts: self.mergeable.as_deref() == Some("CONFLICTING"),
            checks,
            comments: self.comments.as_ref().map(|c| c.total_count).unwrap_or(0),
            additions: self.additions,
            deletions: self.deletions,
        };
        let mut participants: Vec<Person> = self
            .latest_opinionated_reviews
            .iter()
            .flat_map(|c| c.nodes.iter().flatten())
            .filter_map(|r| {
                let mut person = r.author.as_ref()?.person();
                person.review = match r.state.as_str() {
                    "APPROVED" => Some(ReviewState::Approved),
                    "CHANGES_REQUESTED" => Some(ReviewState::ChangesRequested),
                    _ => None,
                };
                Some(person)
            })
            .collect();
        for request in self.review_requests.iter().flat_map(|c| c.nodes.iter().flatten()) {
            if let Some(person) = request.requested_reviewer.as_ref().map(Actor::person) {
                if !participants.iter().any(|p| p.name == person.name) {
                    participants.push(person);
                }
            }
        }
        // Suggestions help you pick reviewers for your own PRs; the file list prepares a review of someone else's.
        let changes = if authored { None } else { self.change_set() };
        let suggested_people = if authored {
            self.suggested_reviewers
                .as_ref()
                .map(|s| s.iter().flatten().filter_map(|s| s.reviewer.as_ref().map(Actor::person)).collect())
        } else {
            None
        };
        InboxItem {
            id: InboxItem::make_id(account_id, &self.id),
            account_id: account_id.into(),
            plugin_id: "github".into(),
            bundle,
            title: remora_core::text::readable(&self.title),
            context: format!("{} #{}", self.repository.name_with_owner, self.number),
            preview: None,
            url: Some(self.url),
            app_url: None,
            author: self.author.as_ref().map(Actor::person),
            participants,
            badges: review.badges(authored),
            date: self.updated_at,
            needs_action: review.needs_action(authored),
            priority: review.priority(authored),
            due: None,
            expires: None,
            changes,
            suggested_people,
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

    /// An app account is a bot as GitHub says it: its `Bot` type, or a `[bot]` login; a person never is. As the Mac.
    #[test]
    fn app_accounts_are_bots() {
        let actor = |json: &str| serde_json::from_str::<Actor>(json).unwrap().person().bot;
        assert!(actor(r#"{"__typename": "Bot", "login": "dependabot"}"#));
        assert!(actor(r#"{"login": "renovate[bot]"}"#));
        assert!(!actor(r#"{"__typename": "User", "login": "alice"}"#));
        assert!(!actor(r#"{"login": "robotics-team"}"#), "a name alone says nothing");
    }

    const RESPONSE: &str = r#"{"data": {
      "viewer": {"login": "alice"},
      "authored": {"nodes": [{
        "id": "PR_1", "number": 7, "title": "Add inbox", "url": "https://github.com/acme/app/pull/7",
        "isDraft": false, "updatedAt": "2026-10-08T10:00:00Z", "additions": 10, "deletions": 2,
        "mergeable": "MERGEABLE", "reviewDecision": "APPROVED",
        "repository": {"nameWithOwner": "acme/app"},
        "author": {"login": "alice", "avatarUrl": "https://avatars/alice"},
        "comments": {"totalCount": 3},
        "latestOpinionatedReviews": {"nodes": [{"state": "APPROVED", "author": {"login": "erin", "avatarUrl": null}}]},
        "reviewRequests": {"nodes": [{"requestedReviewer": {"name": "platform-team"}}]},
        "commits": {"nodes": [{"commit": {"statusCheckRollup": {"state": "FAILURE"}}}]},
        "suggestedReviewers": [{"reviewer": {"login": "grace", "avatarUrl": "https://avatars/grace"}}, null, {"reviewer": null}],
        "changedFiles": 1, "files": {"nodes": [{"path": "src/inbox.rs", "additions": 10, "deletions": 2}]}
      }]},
      "reviewing": {"nodes": [{
        "id": "PR_2", "number": 9, "title": "Fix login", "url": "https://github.com/acme/app/pull/9",
        "isDraft": false, "updatedAt": "2026-10-08T09:00:00Z", "mergeable": "CONFLICTING",
        "repository": {"nameWithOwner": "acme/app"}, "author": {"login": "frank"},
        "suggestedReviewers": [{"reviewer": {"login": "heidi"}}],
        "changedFiles": 3,
        "files": {"nodes": [{"path": "src/search/index.ts", "additions": 40, "deletions": 2}, {"path": "src/search/index.test.ts", "additions": 12, "deletions": 0}]}
      }]}
    }}"#;

    /// The fetch `registry`'s egress test runs through the guard.
    pub(crate) fn egress_fixture() -> EgressFixture {
        EgressFixture {
            settings: vec![("host", "github.com"), ("token", "t")],
            http: Arc::new(StubHttp::paths(&[("/graphql", RESPONSE)])),
        }
    }

    /// A list past its 50 says so; one that fits doesn't. Same cases as macOS.
    #[tokio::test]
    async fn says_when_a_list_is_cut() {
        let remarks = |json: String| async move {
            let plugin = GitHubPlugin::new(
                &config(&[("host", "github.com"), ("token", "t")]),
                Arc::new(StubHttp::paths(&[("/graphql", &json)])),
            )
            .unwrap();
            plugin.fetch().await.unwrap().remarks
        };
        assert!(remarks(RESPONSE.to_string()).await.is_empty(), "no count: nothing to say");
        assert!(remarks(RESPONSE.replace(r#""reviewing": {"nodes""#, r#""reviewing": {"issueCount": 1, "nodes""#))
            .await
            .is_empty());
        let cut =
            remarks(RESPONSE.replace(r#""reviewing": {"nodes""#, r#""reviewing": {"issueCount": 73, "nodes""#)).await;
        assert_eq!(cut, [crate::truncated("GitHub")]);
        assert_eq!(cut[0].text, "GitHub has more than Remora shows: only the latest 50 of each list are listed.");
        assert_eq!((cut[0].code.as_str(), cut[0].args.as_slice()), ("truncated", ["GitHub".to_string()].as_slice()));
    }

    #[tokio::test]
    async fn maps_authored_and_review_requests() {
        let plugin = GitHubPlugin::new(
            &config(&[("host", "github.com"), ("token", "t")]),
            Arc::new(StubHttp::paths(&[("/graphql", RESPONSE)])),
        )
        .unwrap();
        let snapshot = plugin.fetch().await.unwrap();
        assert_eq!(snapshot.identity, "alice");

        let authored = snapshot.items.iter().find(|i| i.bundle == InboxBundle::authored()).unwrap();
        assert_eq!(authored.context, "acme/app #7");
        assert!(authored.has_badge("approved") && authored.has_badge("checks.failing"));
        assert!(authored.badges.iter().any(|b| b.id == "approved" && b.notify.is_some()));
        assert_eq!(
            authored.participants.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(),
            ["erin", "platform-team"]
        );
        assert!(authored.needs_action, "approved with failing checks: your turn");

        let review = snapshot.items.iter().find(|i| i.bundle == InboxBundle::reviews()).unwrap();
        assert!(review.has_badge("conflicts") && review.needs_action && review.priority.is_none());
    }

    /// A review request someone already approved can wait: still in To review, low, so neither counted nor
    /// announced. In a repository that requires no review, GitHub gives no decision and the reviews tell. As macOS.
    #[tokio::test]
    async fn an_approved_review_request_can_wait() {
        async fn review(reviews: &str) -> InboxItem {
            let json = RESPONSE.replace(
                r#""mergeable": "CONFLICTING","#,
                &format!(r#""mergeable": "MERGEABLE", "latestOpinionatedReviews": {{"nodes": [{reviews}]}},"#),
            );
            let http = Arc::new(StubHttp::paths(&[("/graphql", json.as_str())]));
            let plugin = GitHubPlugin::new(&config(&[("host", "github.com"), ("token", "t")]), http).unwrap();
            plugin.fetch().await.unwrap().items.into_iter().find(|i| i.bundle == InboxBundle::reviews()).unwrap()
        }
        let approved = review(r#"{"state": "APPROVED", "author": {"login": "erin"}}"#).await;
        assert!(approved.has_badge("approved") && approved.priority == Some(Priority::Low) && approved.needs_action);
        let blocked = review(
            r#"{"state": "APPROVED", "author": {"login": "erin"}}, {"state": "CHANGES_REQUESTED", "author": {"login": "gus"}}"#,
        )
        .await;
        assert!(blocked.has_badge("changes") && blocked.priority.is_none());
    }

    /// The files of a PR you review (for the review prep), the suggested reviewers of yours (for the
    /// waiting assistant). Same split as macOS.
    #[tokio::test]
    async fn maps_changed_files_and_suggested_reviewers() {
        let plugin = GitHubPlugin::new(
            &config(&[("host", "github.com"), ("token", "t")]),
            Arc::new(StubHttp::paths(&[("/graphql", RESPONSE)])),
        )
        .unwrap();
        let snapshot = plugin.fetch().await.unwrap();
        let authored = snapshot.items.iter().find(|i| i.bundle == InboxBundle::authored()).unwrap();
        let review = snapshot.items.iter().find(|i| i.bundle == InboxBundle::reviews()).unwrap();

        let changes = review.changes.as_ref().unwrap();
        assert_eq!(
            changes.files.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(),
            ["src/search/index.ts", "src/search/index.test.ts"]
        );
        assert_eq!((changes.files[0].additions, changes.files[0].deletions), (Some(40), Some(2)));
        assert_eq!(changes.file_count, 3, "the total, beyond the files listed");
        assert_eq!(authored.changes, None, "your own PRs don't need a review prep");

        let suggested = authored.suggested_people.as_ref().unwrap();
        assert_eq!(suggested.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["grace"]);
        assert_eq!(suggested[0].avatar_url.as_deref(), Some("https://avatars/grace"));
        assert_eq!(review.suggested_people, None, "only for your own PRs");
    }

    #[tokio::test]
    async fn no_files_means_no_change_set() {
        let json = RESPONSE.replace(r#""changedFiles": 3,"#, "").replace(r#""files": {"nodes": [{"path": "src/search/index.ts", "additions": 40, "deletions": 2}, {"path": "src/search/index.test.ts", "additions": 12, "deletions": 0}]}"#, r#""files": {"nodes": []}"#);
        let plugin = GitHubPlugin::new(
            &config(&[("host", "github.com"), ("token", "t")]),
            Arc::new(StubHttp::paths(&[("/graphql", &json)])),
        )
        .unwrap();
        let snapshot = plugin.fetch().await.unwrap();
        let review = snapshot.items.iter().find(|i| i.bundle == InboxBundle::reviews()).unwrap();
        assert_eq!(review.changes, None);
    }

    #[tokio::test]
    async fn surfaces_graphql_errors_and_requires_a_token() {
        let errors = r#"{"errors": [{"message": "Bad credentials"}]}"#;
        let plugin = GitHubPlugin::new(
            &config(&[("host", "https://github.com"), ("token", "t")]),
            Arc::new(StubHttp::paths(&[("/graphql", errors)])),
        )
        .unwrap();
        assert_eq!(plugin.fetch().await.err(), Some(PluginError::Unauthorized), "a rejected token is reconnected");
        assert_eq!(
            GitHubPlugin::new(&config(&[("host", "github.com")]), Arc::new(StubHttp::paths(&[]))).err(),
            Some(PluginError::MissingField("token".into()))
        );
    }

    /// GraphQL points used up: GitHub answers 200 with a RATE_LIMITED error, and the reset is in the headers.
    #[test]
    fn graphql_rate_limit_waits_for_the_reset() {
        let body = r#"{"errors": [{"type": "RATE_LIMITED", "message": "API rate limit exceeded"}]}"#;
        let response: GraphQlResponse = serde_json::from_str(body).unwrap();
        assert_eq!(
            GitHubPlugin::snapshot(response, "a", Some(1_800_000_600)).err(),
            Some(PluginError::RateLimited(Some(1_800_000_600)))
        );
    }

    /// Issues assigned to you are things to do; a list past 50 says so too.
    #[tokio::test]
    async fn issues_assigned_to_you_are_things_to_do() {
        let issue = r#""assigned": {"issueCount": 60, "nodes": [{"id": "I_1", "number": 31, "title": "Crash on *launch*",
          "url": "https://github.com/acme/app/issues/31", "updatedAt": "2026-10-08T11:00:00Z",
          "repository": {"nameWithOwner": "acme/app"}, "author": {"login": "grace"}}]},"#;
        let json = RESPONSE.replacen(
            r#""viewer": {"login": "alice"},"#,
            &format!(r#""viewer": {{"login": "alice"}}, {issue}"#),
            1,
        );
        let plugin = GitHubPlugin::new(
            &config(&[("host", "github.com"), ("token", "t")]),
            Arc::new(StubHttp::paths(&[("/graphql", &json)])),
        )
        .unwrap();
        let snapshot = plugin.fetch().await.unwrap();
        let task = snapshot.items.iter().find(|i| i.bundle == InboxBundle::tasks()).unwrap();
        assert_eq!((task.title.as_str(), task.context.as_str()), ("Crash on launch", "acme/app #31"));
        assert_eq!(task.author.as_ref().map(|a| a.name.as_str()), Some("grace"));
        assert!(task.needs_action);
        assert_eq!(snapshot.remarks, [crate::truncated("GitHub")]);
    }

    /// An assigned issue a pull request will close waits on its review, tied to it. As macOS.
    #[tokio::test]
    async fn an_issue_with_a_closing_pull_request_waits_on_its_review() {
        let issue = r#""assigned": {"issueCount": 1, "nodes": [{"id": "I_1", "number": 31, "title": "Crash",
          "url": "https://github.com/acme/app/issues/31", "updatedAt": "2026-10-08T11:00:00Z",
          "repository": {"nameWithOwner": "acme/app"},
          "closedByPullRequestsReferences": {"nodes": [{"url": "https://github.com/acme/app/pull/32"}]}}]},"#;
        let json = RESPONSE.replacen(
            r#""viewer": {"login": "alice"},"#,
            &format!(r#""viewer": {{"login": "alice"}}, {issue}"#),
            1,
        );
        let plugin = GitHubPlugin::new(
            &config(&[("host", "github.com"), ("token", "t")]),
            Arc::new(StubHttp::paths(&[("/graphql", &json)])),
        )
        .unwrap();
        let snapshot = plugin.fetch().await.unwrap();
        let task = snapshot.items.iter().find(|i| i.context == "acme/app #31").unwrap();
        assert_eq!(task.bundle, InboxBundle::awaiting());
        assert!(!task.needs_action);
        assert_eq!(task.related, Some(vec!["https://github.com/acme/app/pull/32".to_string()]));
    }

    const REPOSITORY: &str = r#"{"default_branch": "main"}"#;
    const RED_RUN: &str = r#"{"workflow_runs": [{"id": 42, "name": "CI", "display_title": "Bump *serde*",
      "conclusion": "failure", "html_url": "https://github.com/acme/web/actions/runs/42",
      "updated_at": "2026-10-08T12:00:00Z"}]}"#;
    const DEPENDABOT: &str = r#"[{"number": 3, "html_url": "https://github.com/acme/web/security/dependabot/3",
      "created_at": "2026-10-07T08:00:00Z", "dependency": {"package": {"name": "lodash"}},
      "security_advisory": {"summary": "Prototype pollution", "severity": "critical"}}]"#;
    const CODE_SCANNING: &str = r#"[{"number": 5, "html_url": "https://github.com/acme/web/security/code-scanning/5",
      "created_at": "2026-10-06T08:00:00Z", "rule": {"description": "SQL injection", "security_severity_level": "high"}}]"#;

    fn maintaining(routes: &[(&'static str, &str)]) -> GitHubPlugin {
        let mut all = vec![("/graphql", RESPONSE)];
        all.extend_from_slice(routes);
        GitHubPlugin::new(
            &config(&[("host", "github.com"), ("token", "t"), ("repositories", "acme/web")]),
            Arc::new(StubHttp::paths(&all)),
        )
        .unwrap()
    }

    /// A red default branch and open critical or high alerts are things to fix. Same cases as macOS.
    #[tokio::test]
    async fn a_failing_default_branch_and_security_alerts_are_to_fix() {
        let plugin = maintaining(&[
            ("/repos/acme/web", REPOSITORY),
            ("/repos/acme/web/actions/runs?branch=main", RED_RUN),
            ("/repos/acme/web/dependabot/alerts?severity=critical%2Chigh", DEPENDABOT),
            ("/repos/acme/web/code-scanning/alerts", CODE_SCANNING),
        ]);
        let snapshot = plugin.fetch().await.unwrap();
        assert!(snapshot.remarks.is_empty(), "{:?}", snapshot.remarks);
        let fix: Vec<_> = snapshot.items.iter().filter(|i| i.bundle == InboxBundle::fix()).collect();
        assert_eq!(fix.len(), 3);
        let ci = fix.iter().find(|i| i.has_badge(badge::CHECKS_FAILING)).unwrap();
        assert_eq!((ci.title.as_str(), ci.context.as_str()), ("Bump serde", "acme/web · main"));
        assert_eq!(ci.preview.as_deref(), Some("CI"));
        assert_eq!(ci.url.as_deref(), Some("https://github.com/acme/web/actions/runs/42"));
        assert!(ci.needs_action && ci.badges[0].notify.is_some());
        let dependabot = fix.iter().find(|i| i.title.starts_with("lodash")).unwrap();
        assert_eq!(dependabot.title, "lodash: Prototype pollution");
        assert_eq!((dependabot.context.as_str(), dependabot.priority), ("acme/web", Some(Priority::Urgent)));
        let scanning = fix.iter().find(|i| i.title == "SQL injection").unwrap();
        assert_eq!(scanning.priority, Some(Priority::High));
        assert_ne!(dependabot.id, scanning.id);
    }

    /// A green run is nothing; no Actions or no code scanning is silent; alerts that can't be read say so.
    #[tokio::test]
    async fn a_green_branch_is_nothing_and_unreadable_alerts_are_a_remark() {
        let green = RED_RUN.replace("failure", "success");
        let plugin = maintaining(&[
            ("/repos/acme/web", REPOSITORY),
            ("/repos/acme/web/actions/runs", &green),
            ("/repos/acme/web/dependabot/alerts", "{\"message\": \"nope\"}"),
        ]);
        let snapshot = plugin.fetch().await.unwrap();
        assert!(!snapshot.items.iter().any(|i| i.bundle == InboxBundle::fix()));
        assert_eq!(snapshot.remarks.len(), 1, "{:?}", snapshot.remarks);
        assert!(
            snapshot.remarks[0].text.starts_with("Security alerts couldn’t be read"),
            "{}",
            snapshot.remarks[0].text
        );

        let missing = maintaining(&[]).fetch().await.unwrap();
        assert!(missing.remarks[0].text.starts_with("Repositories you maintain couldn’t be read"));
        assert!(missing.items.iter().any(|i| i.bundle == InboxBundle::authored()), "pull requests still listed");
    }
}

use crate::code_review::{Checks, CodeReview};
use crate::{decode, HttpClient, PluginConfig, PluginError, Request, SourcePlugin, SourceSnapshot};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use remora_core::{ChangeSet, ChangedFile, ConfigField, Egress, InboxBundle, InboxItem, Person, PluginManifest, Tone};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;

pub fn manifest() -> PluginManifest {
    PluginManifest {
        id: "github".into(),
        name: "GitHub".into(),
        summary: "Pull requests you opened and review requests. Works with GitHub Enterprise.".into(),
        fields: vec![
            ConfigField::host("https://github.com"),
            ConfigField::token("Personal access token", "Classic token with the repo and read:org scopes."),
        ],
        setup_steps: vec![
            "Click “Create a token”: GitHub opens with the repo and read:org scopes selected.".into(),
            "Set an expiration, click “Generate token” and paste it below.".into(),
        ],
        setup_label: "Create a token".into(),
        setup_url: Some("{host}/settings/tokens/new?scopes=repo,read:org&description=Remora".into()),
        egress: Egress {
            hosts: vec!["api.github.com".into(), "github.com".into(), "avatars.githubusercontent.com".into()],
            description: "Reads your pull requests and review requests from GitHub (or your GitHub Enterprise host)."
                .into(),
            external_ai: false,
        },
        logo: Some("github".into()),
    }
}

/// Pull requests you opened and those awaiting your review, through a single GraphQL query.
pub struct GitHubPlugin {
    account_id: String,
    endpoint: String,
    token: String,
    http: Arc<dyn HttpClient>,
}

impl GitHubPlugin {
    pub fn new(config: &PluginConfig, http: Arc<dyn HttpClient>) -> Result<Self, PluginError> {
        let host = config.url("host")?;
        let endpoint = if host.ends_with("://github.com") {
            "https://api.github.com/graphql".to_string()
        } else {
            format!("{host}/api/graphql")
        };
        Ok(GitHubPlugin { account_id: config.account_id.clone(), endpoint, token: config.required("token")?, http })
    }

    pub fn snapshot(response: GraphQlResponse, account_id: &str) -> Result<SourceSnapshot, PluginError> {
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
            [&data.authored, &data.reviewing].iter().any(|s| s.issue_count.is_some_and(|n| n as usize > s.nodes.len()));
        let authored =
            data.authored.nodes.into_iter().flatten().map(|pr| pr.item(account_id, InboxBundle::authored(), true));
        let reviewing =
            data.reviewing.nodes.into_iter().flatten().map(|pr| pr.item(account_id, InboxBundle::reviews(), false));
        let remarks = if cut { vec![crate::truncated("GitHub")] } else { vec![] };
        Ok(SourceSnapshot { identity: data.viewer.login, items: authored.chain(reviewing).collect(), remarks })
    }

    const QUERY: &'static str = r#"
    query($authored: String!, $reviewing: String!) {
      viewer { login }
      authored: search(query: $authored, type: ISSUE, first: 50) { issueCount nodes { ...PR } }
      reviewing: search(query: $reviewing, type: ISSUE, first: 50) { issueCount nodes { ...PR } }
    }
    fragment PR on PullRequest {
      id number title url isDraft updatedAt additions deletions mergeable reviewDecision
      repository { nameWithOwner }
      author { login avatarUrl }
      comments { totalCount }
      latestOpinionatedReviews(first: 20) { nodes { state author { login avatarUrl } } }
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
        ]);
        let body = serde_json::json!({ "query": Self::QUERY, "variables": variables });
        let request =
            Request::post_json(&self.endpoint, &body)?.header("Authorization", format!("bearer {}", self.token));
        let response: GraphQlResponse = decode(self.http.as_ref(), request).await?;
        Self::snapshot(response, &self.account_id)
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
}

#[derive(Deserialize)]
struct Payload {
    viewer: Login,
    authored: Search,
    reviewing: Search,
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
    login: Option<String>,
    name: Option<String>,
    avatar_url: Option<String>,
}

impl Actor {
    fn person(&self) -> Person {
        Person {
            name: self.login.clone().or_else(|| self.name.clone()).unwrap_or_else(|| "?".into()),
            avatar_url: self.avatar_url.clone(),
            tone: None,
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
        let review = CodeReview {
            is_draft: self.is_draft,
            is_approved: self.review_decision.as_deref() == Some("APPROVED"),
            changes_requested: self.review_decision.as_deref() == Some("CHANGES_REQUESTED"),
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
                person.tone = match r.state.as_str() {
                    "APPROVED" => Some(Tone::Accent),
                    "CHANGES_REQUESTED" => Some(Tone::Negative),
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
            id: format!("{account_id}/{}", self.id),
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
            priority: None,
            due: None,
            expires: None,
            changes,
            suggested_people,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stub::*;

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
        assert_eq!(cut[0], "GitHub has more than Remora shows: only the latest 50 of each list are listed.");
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
        assert!(review.has_badge("conflicts") && review.needs_action);
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
}

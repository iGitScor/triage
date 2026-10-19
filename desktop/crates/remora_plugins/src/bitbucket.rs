use crate::code_review::{Checks, CodeReview};
use crate::jira::base64;
use crate::{
    decode, url_with_query, AccountCache, HttpClient, PluginConfig, PluginError, Request, SourcePlugin, SourceSnapshot,
    IDENTITY_LIFETIME,
};
use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use futures::future::join_all;
use remora_core::ReviewState;
use remora_core::{ConfigField, Egress, InboxBundle, InboxItem, Person, PluginManifest};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::sync::Arc;

pub fn manifest() -> PluginManifest {
    PluginManifest {
        id: "bitbucket".into(),
        name: "Bitbucket".into(),
        summary: "Pull requests you opened and those you review. Bitbucket Cloud or Data Center.".into(),
        fields: vec![
            ConfigField::host("https://bitbucket.org"),
            ConfigField {
                key: "workspace".into(),
                label: "Workspace".into(),
                placeholder: "your-workspace".into(),
                default_value: String::new(),
                is_secret: false,
                is_optional: true,
                help: Some("Bitbucket Cloud only: the workspace in your links, bitbucket.org/your-workspace.".into()),
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
                help: Some("Bitbucket Cloud only: your Atlassian account’s e-mail.".into()),
                managed_key: None,
                is_toggle: false,
            },
            ConfigField::token("API token", "Bitbucket Cloud: an API token. Data Center: an HTTP access token."),
        ],
        setup_steps: vec![
            "Bitbucket Cloud: click “Create a token”, choose “Create API token with scopes”, then Bitbucket, with read:user, read:repository and read:pullrequest.".into(),
            "Enter your workspace and the e-mail of your Atlassian account too.".into(),
            "Data Center: set your Bitbucket host, then in Bitbucket open your profile → Manage account → HTTP access tokens, create a read token, and leave the workspace and e-mail empty.".into(),
            "Paste the token below.".into(),
        ],
        setup_label: "Create a token".into(),
        setup_url: Some("https://id.atlassian.com/manage-profile/security/api-tokens".into()),
        egress: Egress {
            hosts: vec!["bitbucket.org".into()],
            description: "Reads your pull requests, review requests, approvals and build status from Bitbucket Cloud or your Bitbucket host."
                .into(),
            external_ai: false,
        },
        logo: Some("bitbucket".into()),
        sign_in: None,
    }
}

/// Bitbucket Cloud has no "review requests" list: the review requests are looked for in the workspace's most
/// recently updated repositories, this many. Same as the Mac.
const REPOSITORIES: &str = "25";
const CLOUD_API: &str = "https://api.bitbucket.org/2.0";
const PULL_REQUEST_FIELDS: &str = "+values.participants,+values.reviewers";

/// Pull requests you opened and those you review, on Bitbucket Cloud or Data Center. Same as the macOS
/// `BitbucketPlugin`.
pub struct BitbucketPlugin {
    account_id: String,
    host: String,
    workspace: String,
    authorization: String,
    cloud: bool,
    http: Arc<dyn HttpClient>,
    cache: Arc<AccountCache>,
}

pub fn is_cloud(host: &str) -> bool {
    let host = reqwest::Url::parse(host).ok().and_then(|u| u.host_str().map(str::to_lowercase)).unwrap_or_default();
    matches!(host.as_str(), "bitbucket.org" | "www.bitbucket.org" | "api.bitbucket.org")
}

impl BitbucketPlugin {
    pub fn new(config: &PluginConfig, http: Arc<dyn HttpClient>) -> Result<Self, PluginError> {
        let host = config.url("host")?;
        let token = config.required("token")?;
        let cloud = is_cloud(&host);
        // Cloud signs in with the Atlassian e-mail and an API token, and reads one workspace; Data Center with an
        // HTTP access token.
        let (workspace, authorization) = if cloud {
            let workspace = config.required("workspace")?;
            let email = config.required("email")?;
            (workspace, format!("Basic {}", base64(format!("{email}:{token}").as_bytes())))
        } else {
            (String::new(), format!("Bearer {token}"))
        };
        Ok(BitbucketPlugin {
            account_id: config.account_id.clone(),
            host,
            workspace,
            authorization,
            cloud,
            http,
            cache: config.cache.clone(),
        })
    }

    async fn get<T: DeserializeOwned>(&self, base: &str, path: &str, query: &[(&str, &str)]) -> Result<T, PluginError> {
        let url = url_with_query(base, path, query)?;
        let request =
            Request::get(url).header("Authorization", self.authorization.clone()).header("Accept", "application/json");
        decode(self.http.as_ref(), request).await
    }

    // Cloud

    async fn fetch_cloud(&self) -> Result<SourceSnapshot, PluginError> {
        let me: CloudUser = self.cache.value("user", IDENTITY_LIFETIME, || self.get(CLOUD_API, "user", &[])).await?;
        let selected = me.account_id.clone().unwrap_or_else(|| me.uuid.clone());
        let authored_path = format!("workspaces/{}/pullrequests/{selected}", self.workspace);
        let authored_query = [("state", "OPEN"), ("pagelen", "50"), ("fields", PULL_REQUEST_FIELDS)];
        let (authored, reviewing) = futures::join!(
            self.get::<CloudPage<CloudPullRequest>>(CLOUD_API, &authored_path, &authored_query),
            self.cloud_review_requests(&me),
        );
        let authored = authored?;
        let mut remarks = vec![];
        // Review requests are a bonus: without them (a token without read:repository), your own pull requests still
        // come through, and a remark says so.
        let reviewing = match reviewing {
            Ok(list) => list,
            Err(error) => match crate::rate_limit_of(&error) {
                Some(limit) => return Err(limit),
                None => {
                    remarks.push(crate::partial("Review requests", &error));
                    vec![]
                }
            },
        };
        if authored.next.is_some() {
            remarks.push(crate::truncated("Bitbucket"));
        }
        let all: Vec<(CloudPullRequest, bool)> =
            authored.values.into_iter().map(|p| (p, true)).chain(reviewing.into_iter().map(|p| (p, false))).collect();
        let mut items = Vec::with_capacity(all.len());
        let mut failed = None;
        for chunk in all.chunks(4) {
            let checks = join_all(chunk.iter().map(|(pr, _)| self.cloud_checks(pr))).await;
            for ((pr, authored), checks) in chunk.iter().zip(checks) {
                let checks = checks.unwrap_or_else(|error| {
                    failed = failed.take().or(Some(error));
                    Checks::None
                });
                items.push(pr.item(&me, *authored, checks, &self.account_id));
            }
        }
        if let Some(error) = failed {
            if let Some(limit) = crate::rate_limit_of(&error) {
                return Err(limit);
            }
            remarks.push(crate::partial("Build status", &error));
        }
        let identity = me.display_name.clone().or(me.nickname.clone()).unwrap_or_default();
        Ok(SourceSnapshot { identity, items, remarks })
    }

    /// The open pull requests of the workspace's recently updated repositories where you are a reviewer, without
    /// those you already approved or sent back.
    async fn cloud_review_requests(&self, me: &CloudUser) -> Result<Vec<CloudPullRequest>, PluginError> {
        let repositories: CloudPage<CloudRepository> = self
            .get(
                CLOUD_API,
                &format!("repositories/{}", self.workspace),
                &[
                    ("role", "member"),
                    ("sort", "-updated_on"),
                    ("pagelen", REPOSITORIES),
                    ("fields", "values.full_name"),
                ],
            )
            .await?;
        let query = format!("state=\"OPEN\" AND reviewers.uuid=\"{}\"", me.uuid);
        let mut found = vec![];
        for chunk in repositories.values.chunks(4) {
            let pages = join_all(chunk.iter().map(|repository| {
                let path = format!("repositories/{}/pullrequests", repository.full_name);
                let query = query.clone();
                async move {
                    self.get::<CloudPage<CloudPullRequest>>(
                        CLOUD_API,
                        &path,
                        &[("q", query.as_str()), ("pagelen", "50"), ("fields", PULL_REQUEST_FIELDS)],
                    )
                    .await
                }
            }))
            .await;
            for page in pages {
                found.extend(page?.values.into_iter().filter(|pr| !pr.is_reviewed_by(me)));
            }
        }
        Ok(found)
    }

    async fn cloud_checks(&self, pr: &CloudPullRequest) -> Result<Checks, PluginError> {
        let Some(repository) =
            pr.destination.as_ref().and_then(|d| d.repository.as_ref()).and_then(|r| r.full_name.clone())
        else {
            return Ok(Checks::None);
        };
        let path = format!("repositories/{repository}/pullrequests/{}/statuses", pr.id);
        let statuses: CloudPage<CloudStatus> = self.get(CLOUD_API, &path, &[("pagelen", "25")]).await?;
        Ok(cloud_checks(statuses.values.iter().map(|s| s.state.as_str())))
    }

    // Data Center

    async fn fetch_data_center(&self) -> Result<SourceSnapshot, PluginError> {
        let me: String = self.cache.value("user", IDENTITY_LIFETIME, || self.whoami()).await?;
        let path = "rest/api/1.0/dashboard/pull-requests";
        // Waiting on you: not those you already approved or marked as needing work.
        let (authored, reviewing) = futures::join!(
            self.get::<DataCenterPage>(&self.host, path, &[("role", "AUTHOR"), ("state", "OPEN"), ("limit", "50")]),
            self.get::<DataCenterPage>(
                &self.host,
                path,
                &[("role", "REVIEWER"), ("state", "OPEN"), ("participantStatus", "UNAPPROVED"), ("limit", "50")]
            ),
        );
        let (authored, reviewing) = (authored?, reviewing?);
        let mut remarks =
            if authored.is_cut() || reviewing.is_cut() { vec![crate::truncated("Bitbucket")] } else { vec![] };
        let all: Vec<(DataCenterPullRequest, bool)> = authored
            .values
            .into_iter()
            .map(|p| (p, true))
            .chain(reviewing.values.into_iter().map(|p| (p, false)))
            .collect();
        let mut items = Vec::with_capacity(all.len());
        let mut failed = None;
        for chunk in all.chunks(4) {
            let checks = join_all(chunk.iter().map(|(pr, _)| self.data_center_checks(pr))).await;
            for ((pr, authored), checks) in chunk.iter().zip(checks) {
                let checks = checks.unwrap_or_else(|error| {
                    failed = failed.take().or(Some(error));
                    Checks::None
                });
                items.push(pr.item(*authored, checks, &self.account_id));
            }
        }
        if let Some(error) = failed {
            if let Some(limit) = crate::rate_limit_of(&error) {
                return Err(limit);
            }
            remarks.push(crate::partial("Build status", &error));
        }
        Ok(SourceSnapshot { identity: me, items, remarks })
    }

    /// Who the token belongs to: Data Center answers with the user name, as plain text.
    async fn whoami(&self) -> Result<String, PluginError> {
        let url = url_with_query(&self.host, "plugins/servlet/applinks/whoami", &[])?;
        let response = self.http.send(Request::get(url).header("Authorization", self.authorization.clone())).await?;
        match response.status {
            // An empty answer is a token Bitbucket didn't take.
            200..=299 => {
                Some(response.body.trim().to_string()).filter(|n| !n.is_empty()).ok_or(PluginError::Unauthorized)
            }
            401 | 403 => Err(PluginError::Unauthorized),
            429 => Err(PluginError::RateLimited(None)),
            code => Err(PluginError::Status(code)),
        }
    }

    async fn data_center_checks(&self, pr: &DataCenterPullRequest) -> Result<Checks, PluginError> {
        let Some(commit) = pr.from_ref.as_ref().and_then(|r| r.latest_commit.as_ref()) else {
            return Ok(Checks::None);
        };
        let path = format!("rest/build-status/1.0/commits/stats/{commit}");
        let stats: BuildStats = self.get(&self.host, &path, &[]).await?;
        Ok(stats.checks())
    }
}

#[async_trait]
impl SourcePlugin for BitbucketPlugin {
    async fn fetch(&self) -> Result<SourceSnapshot, PluginError> {
        if self.cloud {
            self.fetch_cloud().await
        } else {
            self.fetch_data_center().await
        }
    }
}

/// Commit statuses, Cloud's `SUCCESSFUL`, `FAILED`, `INPROGRESS`, `STOPPED`: one failure fails them all.
pub fn cloud_checks<'a>(states: impl Iterator<Item = &'a str>) -> Checks {
    let states: Vec<&str> = states.collect();
    if states.contains(&"FAILED") {
        Checks::Failing
    } else if states.contains(&"INPROGRESS") {
        Checks::Running
    } else if states.contains(&"SUCCESSFUL") {
        Checks::Passing
    } else {
        Checks::None
    }
}

fn review_state(approved: bool, changes: bool) -> Option<ReviewState> {
    if approved {
        Some(ReviewState::Approved)
    } else if changes {
        Some(ReviewState::ChangesRequested)
    } else {
        None
    }
}

// Cloud's wire format

#[derive(Deserialize)]
struct CloudPage<T> {
    #[serde(default = "Vec::new")]
    values: Vec<T>,
    /// Set when there is more.
    next: Option<String>,
}

#[derive(Clone, Deserialize)]
struct CloudLink {
    href: Option<String>,
}

#[derive(Clone, Default, Deserialize)]
struct CloudLinks {
    html: Option<CloudLink>,
    avatar: Option<CloudLink>,
}

#[derive(Clone, Deserialize)]
struct CloudUser {
    uuid: String,
    account_id: Option<String>,
    display_name: Option<String>,
    nickname: Option<String>,
    links: Option<CloudLinks>,
}

impl CloudUser {
    fn person(&self, review: Option<ReviewState>) -> Person {
        Person {
            name: self.display_name.clone().or(self.nickname.clone()).unwrap_or_else(|| "?".into()),
            avatar_url: self.links.as_ref().and_then(|l| l.avatar.as_ref()).and_then(|a| a.href.clone()),
            review,
            bot: false,
        }
    }
}

#[derive(Deserialize)]
struct CloudRepository {
    full_name: String,
}

#[derive(Deserialize)]
struct CloudStatus {
    state: String,
}

#[derive(Deserialize)]
struct CloudParticipant {
    user: Option<CloudUser>,
    role: Option<String>,
    approved: Option<bool>,
    /// "approved", "changes_requested", or none.
    state: Option<String>,
}

impl CloudParticipant {
    fn uuid(&self) -> Option<&str> {
        self.user.as_ref().map(|u| u.uuid.as_str())
    }

    fn approved(&self) -> bool {
        self.approved == Some(true)
    }

    fn changes_requested(&self) -> bool {
        self.state.as_deref() == Some("changes_requested")
    }
}

#[derive(Deserialize)]
struct CloudRepo {
    full_name: Option<String>,
}

#[derive(Deserialize)]
struct CloudDestination {
    repository: Option<CloudRepo>,
}

#[derive(Deserialize)]
struct CloudPullRequest {
    id: u64,
    title: String,
    draft: Option<bool>,
    comment_count: Option<u32>,
    updated_on: DateTime<Utc>,
    author: Option<CloudUser>,
    destination: Option<CloudDestination>,
    links: Option<CloudLinks>,
    #[serde(default)]
    participants: Vec<CloudParticipant>,
}

impl CloudPullRequest {
    /// You approved it or asked for changes: nothing to review until it changes.
    fn is_reviewed_by(&self, me: &CloudUser) -> bool {
        self.participants.iter().any(|p| p.uuid() == Some(me.uuid.as_str()) && (p.approved() || p.changes_requested()))
    }

    fn item(&self, me: &CloudUser, authored: bool, checks: Checks, account_id: &str) -> InboxItem {
        let author = self.author.as_ref().map(|a| a.uuid.as_str());
        let others: Vec<&CloudParticipant> =
            self.participants.iter().filter(|p| p.uuid() != author && p.uuid() != Some(me.uuid.as_str())).collect();
        let review = CodeReview {
            is_draft: self.draft.unwrap_or(false),
            is_approved: others.iter().any(|p| p.approved()),
            changes_requested: others.iter().any(|p| p.changes_requested()),
            has_conflicts: false,
            checks,
            comments: self.comment_count.unwrap_or(0),
            additions: None,
            deletions: None,
        };
        let repository =
            self.destination.as_ref().and_then(|d| d.repository.as_ref()).and_then(|r| r.full_name.clone());
        let repository = repository.unwrap_or_default();
        InboxItem {
            id: InboxItem::make_id(account_id, format!("{repository}/{}", self.id)),
            account_id: account_id.into(),
            plugin_id: "bitbucket".into(),
            bundle: if authored { InboxBundle::authored() } else { InboxBundle::reviews() },
            title: remora_core::text::readable(&self.title),
            context: format!("{repository} #{}", self.id),
            preview: None,
            url: self.links.as_ref().and_then(|l| l.html.as_ref()).and_then(|h| h.href.clone()),
            app_url: None,
            author: self.author.as_ref().map(|a| a.person(None)),
            participants: others
                .iter()
                .filter(|p| p.role.as_deref() == Some("REVIEWER") || p.approved())
                .filter_map(|p| p.user.as_ref().map(|u| u.person(review_state(p.approved(), p.changes_requested()))))
                .collect(),
            badges: review.badges(authored),
            date: self.updated_on,
            needs_action: review.needs_action(authored),
            priority: review.priority(authored),
            due: None,
            expires: None,
            changes: None,
            suggested_people: None,
            recurrence: None,
            reactions: None,
            reason: None,
        }
    }
}

// Data Center's wire format

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DataCenterPage {
    #[serde(default)]
    values: Vec<DataCenterPullRequest>,
    is_last_page: Option<bool>,
}

impl DataCenterPage {
    fn is_cut(&self) -> bool {
        self.is_last_page == Some(false)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DataCenterUser {
    name: Option<String>,
    display_name: Option<String>,
}

impl DataCenterUser {
    fn person(&self, review: Option<ReviewState>) -> Person {
        Person {
            name: self.display_name.clone().or(self.name.clone()).unwrap_or_else(|| "?".into()),
            avatar_url: None,
            review,
            bot: false,
        }
    }
}

#[derive(Deserialize)]
struct DataCenterParticipant {
    user: Option<DataCenterUser>,
    approved: Option<bool>,
    /// "APPROVED", "NEEDS_WORK" or "UNAPPROVED".
    status: Option<String>,
}

impl DataCenterParticipant {
    fn approved(&self) -> bool {
        self.status.as_deref() == Some("APPROVED") || self.approved == Some(true)
    }

    /// "Needs work" is Data Center's "changes requested".
    fn needs_work(&self) -> bool {
        self.status.as_deref() == Some("NEEDS_WORK")
    }
}

#[derive(Deserialize)]
struct DataCenterProject {
    key: Option<String>,
}

#[derive(Deserialize)]
struct DataCenterRepository {
    slug: Option<String>,
    project: Option<DataCenterProject>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DataCenterRef {
    latest_commit: Option<String>,
    repository: Option<DataCenterRepository>,
}

#[derive(Deserialize)]
struct DataCenterLink {
    href: Option<String>,
}

#[derive(Deserialize)]
struct DataCenterLinks {
    #[serde(rename = "self", default)]
    own: Vec<DataCenterLink>,
}

#[derive(Deserialize)]
struct DataCenterMerge {
    outcome: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DataCenterProperties {
    comment_count: Option<u32>,
    merge_result: Option<DataCenterMerge>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DataCenterPullRequest {
    id: u64,
    title: String,
    draft: Option<bool>,
    /// Milliseconds since 1970.
    updated_date: Option<i64>,
    author: Option<DataCenterParticipant>,
    #[serde(default)]
    reviewers: Vec<DataCenterParticipant>,
    from_ref: Option<DataCenterRef>,
    to_ref: Option<DataCenterRef>,
    links: Option<DataCenterLinks>,
    properties: Option<DataCenterProperties>,
}

impl DataCenterPullRequest {
    fn item(&self, authored: bool, checks: Checks, account_id: &str) -> InboxItem {
        let properties = self.properties.as_ref();
        let review = CodeReview {
            is_draft: self.draft.unwrap_or(false),
            is_approved: self.reviewers.iter().any(DataCenterParticipant::approved),
            changes_requested: self.reviewers.iter().any(DataCenterParticipant::needs_work),
            has_conflicts: properties.and_then(|p| p.merge_result.as_ref()).and_then(|m| m.outcome.as_deref())
                == Some("CONFLICTED"),
            checks,
            comments: properties.and_then(|p| p.comment_count).unwrap_or(0),
            additions: None,
            deletions: None,
        };
        let repository = self.to_ref.as_ref().and_then(|r| r.repository.as_ref());
        let path = [
            repository.and_then(|r| r.project.as_ref()).and_then(|p| p.key.clone()),
            repository.and_then(|r| r.slug.clone()),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join("/");
        InboxItem {
            id: InboxItem::make_id(account_id, format!("{path}/{}", self.id)),
            account_id: account_id.into(),
            plugin_id: "bitbucket".into(),
            bundle: if authored { InboxBundle::authored() } else { InboxBundle::reviews() },
            title: remora_core::text::readable(&self.title),
            context: format!("{path} #{}", self.id),
            preview: None,
            url: self.links.as_ref().and_then(|l| l.own.first()).and_then(|l| l.href.clone()),
            app_url: None,
            author: self.author.as_ref().and_then(|a| a.user.as_ref()).map(|u| u.person(None)),
            participants: self
                .reviewers
                .iter()
                .filter_map(|r| r.user.as_ref().map(|u| u.person(review_state(r.approved(), r.needs_work()))))
                .collect(),
            badges: review.badges(authored),
            date: self.updated_date.and_then(|ms| Utc.timestamp_millis_opt(ms).single()).unwrap_or_else(Utc::now),
            needs_action: review.needs_action(authored),
            priority: review.priority(authored),
            due: None,
            expires: None,
            changes: None,
            suggested_people: None,
            recurrence: None,
            reactions: None,
            reason: None,
        }
    }
}

/// What the builds of a commit add up to.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BuildStats {
    successful: Option<u32>,
    failed: Option<u32>,
    in_progress: Option<u32>,
}

impl BuildStats {
    fn checks(&self) -> Checks {
        if self.failed.unwrap_or(0) > 0 {
            Checks::Failing
        } else if self.in_progress.unwrap_or(0) > 0 {
            Checks::Running
        } else if self.successful.unwrap_or(0) > 0 {
            Checks::Passing
        } else {
            Checks::None
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::stub::config;
    use crate::Response;
    use std::collections::HashMap;

    /// Answers Bitbucket by the longest matching end of the path, and the Data Center dashboard by its role. Checks
    /// the Authorization header. Same fixtures as the Mac (`BitbucketPluginTests.swift`).
    struct BitbucketStub {
        routes: HashMap<&'static str, String>,
        authorization: String,
    }

    #[async_trait]
    impl HttpClient for BitbucketStub {
        async fn send(&self, request: Request) -> Result<Response, PluginError> {
            let auth = request.headers.iter().find(|(k, _)| k == "Authorization").map(|(_, v)| v.clone());
            assert_eq!(auth.as_deref(), Some(self.authorization.as_str()));
            let url = reqwest::Url::parse(&request.url).unwrap();
            let mut key = url.path().to_string();
            if key.ends_with("/dashboard/pull-requests") {
                let role =
                    url.query_pairs().find(|(k, _)| k == "role").map(|(_, v)| v.into_owned()).unwrap_or_default();
                key = format!("{key}?{role}");
            }
            let body = self.routes.iter().filter(|(suffix, _)| key.ends_with(*suffix)).max_by_key(|(s, _)| s.len());
            Ok(match body {
                Some((_, body)) => Response { status: 200, body: body.clone(), etag: None, reset: None },
                None => Response { status: 404, body: String::new(), etag: None, reset: None },
            })
        }
    }

    const USER: &str = r#"{"uuid": "{a1}", "account_id": "alice-id", "display_name": "Alice", "nickname": "alice"}"#;

    const AUTHORED: &str = r#"
        {"values": [{"id": 7, "title": "Retry failed webhooks", "state": "OPEN", "draft": false, "comment_count": 2,
          "updated_on": "2026-10-12T09:14:03.123456+00:00",
          "author": {"uuid": "{a1}", "display_name": "Alice"},
          "destination": {"repository": {"full_name": "acme/inbox"}},
          "links": {"html": {"href": "https://bitbucket.org/acme/inbox/pull-requests/7"}},
          "participants": [
            {"user": {"uuid": "{b2}", "display_name": "Bob", "links": {"avatar": {"href": "https://secure.gravatar.com/avatar/b"}}},
             "role": "REVIEWER", "approved": true, "state": "approved"}]}],
         "pagelen": 50}"#;

    const REPOSITORIES_PAGE: &str = r#"{"values": [{"full_name": "acme/web"}, {"full_name": "acme/api"}]}"#;

    const REVIEWING: &str = r#"
        {"values": [
          {"id": 12, "title": "Add dark mode", "state": "OPEN", "comment_count": 0,
           "updated_on": "2026-10-12T08:00:00.000001+00:00",
           "author": {"uuid": "{e5}", "display_name": "Erin"},
           "destination": {"repository": {"full_name": "acme/web"}},
           "links": {"html": {"href": "https://bitbucket.org/acme/web/pull-requests/12"}},
           "participants": [
             {"user": {"uuid": "{a1}", "display_name": "Alice"}, "role": "REVIEWER", "approved": false},
             {"user": {"uuid": "{b2}", "display_name": "Bob"}, "role": "REVIEWER", "approved": false, "state": "changes_requested"}]},
          {"id": 13, "title": "Bump dependencies", "state": "OPEN",
           "updated_on": "2026-10-11T08:00:00+00:00",
           "author": {"uuid": "{e5}", "display_name": "Erin"},
           "destination": {"repository": {"full_name": "acme/web"}},
           "participants": [{"user": {"uuid": "{a1}", "display_name": "Alice"}, "role": "REVIEWER", "approved": true, "state": "approved"}]}
        ]}"#;

    fn cloud_routes() -> HashMap<&'static str, String> {
        HashMap::from([
            ("/2.0/user", USER.to_string()),
            ("/2.0/workspaces/acme/pullrequests/alice-id", AUTHORED.to_string()),
            ("/2.0/repositories/acme", REPOSITORIES_PAGE.to_string()),
            ("/2.0/repositories/acme/web/pullrequests", REVIEWING.to_string()),
            ("/2.0/repositories/acme/api/pullrequests", r#"{"values": []}"#.to_string()),
            (
                "/2.0/repositories/acme/inbox/pullrequests/7/statuses",
                r#"{"values": [{"state": "SUCCESSFUL"}, {"state": "FAILED"}]}"#.to_string(),
            ),
            (
                "/2.0/repositories/acme/web/pullrequests/12/statuses",
                r#"{"values": [{"state": "INPROGRESS"}]}"#.to_string(),
            ),
        ])
    }

    const DATA_CENTER_AUTHORED: &str = r#"
        {"values": [{"id": 42, "title": "Draft: new login page", "draft": true, "updatedDate": 1760259243000,
          "author": {"user": {"name": "alice", "displayName": "Alice"}, "status": "UNAPPROVED"},
          "reviewers": [],
          "fromRef": {"latestCommit": "c0ffee"},
          "toRef": {"repository": {"slug": "portal", "project": {"key": "WEB"}}},
          "links": {"self": [{"href": "https://bitbucket.acme.io/projects/WEB/repos/portal/pull-requests/42"}]}}],
         "isLastPage": true}"#;

    const DATA_CENTER_REVIEWING: &str = r#"
        {"values": [{"id": 9, "title": "Rotate keys", "updatedDate": 1760255643000,
          "author": {"user": {"name": "erin", "displayName": "Erin"}},
          "reviewers": [
            {"user": {"name": "alice", "displayName": "Alice"}, "status": "UNAPPROVED"},
            {"user": {"name": "bob", "displayName": "Bob"}, "status": "NEEDS_WORK"}],
          "fromRef": {"latestCommit": "beef"},
          "toRef": {"repository": {"slug": "vault", "project": {"key": "OPS"}}},
          "links": {"self": [{"href": "https://bitbucket.acme.io/projects/OPS/repos/vault/pull-requests/9"}]},
          "properties": {"commentCount": 3, "mergeResult": {"outcome": "CONFLICTED"}}}],
         "isLastPage": false}"#;

    fn cloud_auth() -> String {
        format!("Basic {}", base64(b"alice@acme.com:t0ken"))
    }

    fn cloud(routes: HashMap<&'static str, String>) -> BitbucketPlugin {
        let stub = BitbucketStub { routes, authorization: cloud_auth() };
        BitbucketPlugin::new(
            &config(&[
                ("host", "https://bitbucket.org"),
                ("workspace", "acme"),
                ("email", "alice@acme.com"),
                ("token", "t0ken"),
            ]),
            Arc::new(stub),
        )
        .unwrap()
    }

    fn ids(item: &InboxItem) -> Vec<&str> {
        item.badges.iter().map(|b| b.id.as_str()).collect()
    }

    #[tokio::test]
    async fn cloud_pull_requests_you_opened_and_review() {
        let snapshot = cloud(cloud_routes()).fetch().await.unwrap();
        assert_eq!(snapshot.identity, "Alice");
        assert!(snapshot.remarks.is_empty());
        let contexts: Vec<&str> = snapshot.items.iter().map(|i| i.context.as_str()).collect();
        assert_eq!(contexts, ["acme/inbox #7", "acme/web #12"], "#13 is approved by you");

        let mine = &snapshot.items[0];
        assert!(mine.bundle == InboxBundle::authored() && mine.needs_action);
        assert_eq!(ids(mine), ["approved", "checks.failing", "comments"]);
        assert_eq!(
            mine.participants.iter().map(|p| (p.name.as_str(), p.review)).collect::<Vec<_>>(),
            [("Bob", Some(ReviewState::Approved))]
        );
        assert_eq!(mine.url.as_deref(), Some("https://bitbucket.org/acme/inbox/pull-requests/7"));
        assert_eq!(mine.date.timestamp_millis(), 1_791_796_443_123);

        let review = &snapshot.items[1];
        assert!(review.bundle == InboxBundle::reviews() && review.needs_action);
        assert_eq!(ids(review), ["changes", "checks.running"]);
        assert_eq!(review.author.as_ref().map(|a| a.name.as_str()), Some("Erin"));
        assert_eq!(review.participants.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["Bob"]);
    }

    #[tokio::test]
    async fn cloud_says_when_pull_requests_are_cut() {
        let mut routes = cloud_routes();
        routes.insert(
            "/2.0/workspaces/acme/pullrequests/alice-id",
            AUTHORED.replace(r#""pagelen": 50"#, r#""next": "https://api.bitbucket.org/2.0/next""#),
        );
        assert_eq!(cloud(routes).fetch().await.unwrap().remarks, [crate::truncated("Bitbucket")]);
    }

    /// A token that can't list repositories: your own pull requests still come, with a remark.
    #[tokio::test]
    async fn review_requests_are_best_effort() {
        let mut routes = cloud_routes();
        routes.remove("/2.0/repositories/acme");
        let snapshot = cloud(routes).fetch().await.unwrap();
        assert_eq!(snapshot.items.iter().map(|i| i.context.as_str()).collect::<Vec<_>>(), ["acme/inbox #7"]);
        assert_eq!(snapshot.remarks.len(), 1);
    }

    #[test]
    fn cloud_needs_the_workspace_and_email() {
        let stub = || Arc::new(crate::stub::StubHttp::paths(&[]));
        let plugin = BitbucketPlugin::new(&config(&[("host", "bitbucket.org"), ("token", "t")]), stub());
        assert_eq!(plugin.err(), Some(PluginError::MissingField("workspace".into())));
        let plugin =
            BitbucketPlugin::new(&config(&[("host", "bitbucket.org"), ("workspace", "acme"), ("token", "t")]), stub());
        assert_eq!(plugin.err(), Some(PluginError::MissingField("email".into())));
    }

    /// Data Center: an HTTP access token as Bearer, the dashboard, "needs work", conflicts and build statistics.
    #[tokio::test]
    async fn data_center_uses_the_dashboard() {
        let routes = HashMap::from([
            ("/plugins/servlet/applinks/whoami", "alice\n".to_string()),
            ("/rest/api/1.0/dashboard/pull-requests?AUTHOR", DATA_CENTER_AUTHORED.to_string()),
            ("/rest/api/1.0/dashboard/pull-requests?REVIEWER", DATA_CENTER_REVIEWING.to_string()),
            (
                "/rest/build-status/1.0/commits/stats/c0ffee",
                r#"{"successful": 2, "failed": 0, "inProgress": 0}"#.into(),
            ),
            ("/rest/build-status/1.0/commits/stats/beef", r#"{"successful": 0, "failed": 0, "inProgress": 1}"#.into()),
        ]);
        let stub = BitbucketStub { routes, authorization: "Bearer pat".into() };
        let plugin =
            BitbucketPlugin::new(&config(&[("host", "bitbucket.acme.io"), ("token", "pat")]), Arc::new(stub)).unwrap();
        let snapshot = plugin.fetch().await.unwrap();
        assert_eq!(snapshot.identity, "alice");
        assert_eq!(snapshot.remarks, [crate::truncated("Bitbucket")]);
        let contexts: Vec<&str> = snapshot.items.iter().map(|i| i.context.as_str()).collect();
        assert_eq!(contexts, ["WEB/portal #42", "OPS/vault #9"]);

        let draft = &snapshot.items[0];
        assert!(draft.bundle == InboxBundle::authored() && !draft.needs_action);
        assert_eq!(ids(draft), ["draft", "checks.passing"]);
        assert_eq!(draft.url.as_deref(), Some("https://bitbucket.acme.io/projects/WEB/repos/portal/pull-requests/42"));
        assert_eq!(draft.date.timestamp(), 1_760_259_243);

        let review = &snapshot.items[1];
        assert!(review.bundle == InboxBundle::reviews() && review.needs_action);
        assert_eq!(ids(review), ["changes", "conflicts", "checks.running", "comments"]);
        assert_eq!(
            review.participants.iter().map(|p| p.review).collect::<Vec<_>>(),
            [None, Some(ReviewState::ChangesRequested)]
        );
    }

    #[test]
    fn reads_build_statuses() {
        assert!(cloud_checks(["SUCCESSFUL", "STOPPED"].into_iter()) == Checks::Passing);
        assert!(cloud_checks(std::iter::empty()) == Checks::None);
        assert!(is_cloud("https://bitbucket.org") && !is_cloud("https://bitbucket.acme.io"));
    }

    /// The fetch `registry`'s egress test runs through the guard.
    pub(crate) fn egress_fixture() -> crate::stub::EgressFixture {
        crate::stub::EgressFixture {
            settings: vec![
                ("host", "bitbucket.org"),
                ("workspace", "acme"),
                ("email", "alice@acme.com"),
                ("token", "t0ken"),
            ],
            http: Arc::new(BitbucketStub { routes: cloud_routes(), authorization: cloud_auth() }),
        }
    }
}

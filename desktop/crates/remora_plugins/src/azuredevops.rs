use crate::code_review::{Checks, CodeReview};
use crate::jira::base64;
use crate::linear::{due_badge, due_date};
use crate::{
    decode, url_with_query, AccountCache, HttpClient, PluginConfig, PluginError, Request, SourcePlugin, SourceSnapshot,
    IDENTITY_LIFETIME,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures::future::join_all;
use remora_core::badge;
use remora_core::ReviewState;
use remora_core::{
    Badge, ChangeSet, ChangedFile, ConfigField, Egress, InboxBundle, InboxItem, Person, PluginManifest, Priority, Tone,
};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

pub fn manifest() -> PluginManifest {
    PluginManifest {
        id: "azuredevops".into(),
        name: "Azure DevOps".into(),
        summary: "Pull requests you review or opened, with votes and policy checks, and work items assigned to you. \
                  Works with Azure DevOps Server."
            .into(),
        fields: vec![
            ConfigField {
                key: "host".into(),
                label: "Organization URL".into(),
                placeholder: "https://dev.azure.com/your-organization".into(),
                default_value: String::new(),
                is_secret: false,
                is_optional: false,
                is_toggle: false,
                help: Some(
                    "Azure DevOps Server: your collection, for example https://tfs.acme.example/tfs/DefaultCollection."
                        .into(),
                ),
                managed_key: None,
            },
            ConfigField::token("Personal access token", "Scopes: Code (Read) and Work Items (Read)."),
        ],
        setup_steps: vec![
            "Set your organization URL first: https://dev.azure.com/your-organization, or your server’s collection."
                .into(),
            "Click “Create a token”, name it Remora, and select Code (Read) and Work Items (Read).".into(),
            "Click Create and paste the token below.".into(),
        ],
        setup_label: "Create a token".into(),
        setup_url: Some("{host}/_usersSettings/tokens".into()),
        egress: Egress {
            hosts: vec!["dev.azure.com".into()],
            description: "Reads your pull requests, votes, policy checks, changed file paths and assigned work items \
                          from your Azure DevOps organization or server."
                .into(),
            external_ai: false,
        },
        logo: None,
        sign_in: None,
    }
}

/// Each list stops at one page: a full one means the tool has more.
const PAGE_SIZE: usize = 50;
/// Azure DevOps Server 2020 and later, and the cloud.
const API_VERSION: &str = "6.0";
/// Open work items assigned to you, whatever the process (Basic, Agile, Scrum, CMMI).
const ASSIGNED_WIQL: &str = "SELECT [System.Id] FROM WorkItems WHERE [System.AssignedTo] = @Me \
    AND [System.State] NOT IN ('Done', 'Closed', 'Completed', 'Resolved', 'Removed') \
    ORDER BY [System.ChangedDate] DESC";
/// The policies that are checks: Build and Status. The others (reviewers, linked work items, comments) are already
/// the votes, or rules.
const CHECK_POLICIES: [&str; 2] = ["0609b952-1397-4640-95ec-e00a01b2c241", "cbdc66da-9728-4af8-aada-9a5a32e4a226"];

/// Pull requests you review or opened, and work items assigned to you, on Azure DevOps or Azure DevOps Server.
/// Same queries and mapping as the macOS app (`AzureDevOpsPlugin.swift`).
pub struct AzureDevOpsPlugin {
    account_id: String,
    host: String,
    authorization: String,
    http: Arc<dyn HttpClient>,
    cache: Arc<AccountCache>,
}

impl AzureDevOpsPlugin {
    pub fn new(config: &PluginConfig, http: Arc<dyn HttpClient>) -> Result<Self, PluginError> {
        let token = config.required("token")?;
        Ok(AzureDevOpsPlugin {
            account_id: config.account_id.clone(),
            host: config.url("host")?,
            // A personal access token goes as the password of Basic, with no user name.
            authorization: format!("Basic {}", base64(format!(":{token}").as_bytes())),
            http,
            cache: config.cache.clone(),
        })
    }

    fn request(&self, request: Request) -> Request {
        request
            .header("Authorization", self.authorization.clone())
            .header("Accept", "application/json")
            // A refused token answers 401, not a sign-in page.
            .header("X-TFS-FedAuthRedirect", "Suppress")
    }

    async fn get<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, &str)],
        version: Option<&str>,
    ) -> Result<T, PluginError> {
        let mut query = query.to_vec();
        if let Some(version) = version {
            query.push(("api-version", version));
        }
        let url = url_with_query(&self.host, path, &query)?;
        decode(self.http.as_ref(), self.request(Request::get(url))).await
    }

    async fn evaluations(&self, pr: &PullRequest) -> Result<Vec<Evaluation>, PluginError> {
        let project = &pr.repository.project.id;
        let artifact =
            format!("vstfs:///CodeReview/CodeReviewId/{project}/{}", pr.code_review_id.unwrap_or(pr.pull_request_id));
        let version = format!("{API_VERSION}-preview.1");
        let list: List<Evaluation> = self
            .get(&format!("{project}/_apis/policy/evaluations"), &[("artifactId", &artifact)], Some(&version))
            .await?;
        Ok(list.value)
    }

    async fn last_iteration(&self, pr: &PullRequest) -> Result<Option<Iteration>, PluginError> {
        let path =
            format!("_apis/git/repositories/{}/pullRequests/{}/iterations", pr.repository.id, pr.pull_request_id);
        let list: List<Iteration> = self.get(&path, &[], Some(API_VERSION)).await?;
        Ok(list.value.into_iter().max_by_key(|i| i.id))
    }

    /// Review prep: file paths only, from the latest push against the target branch. No content is read.
    async fn changes(&self, pr: &PullRequest, iteration: u64) -> Result<Vec<Change>, PluginError> {
        let path = format!(
            "_apis/git/repositories/{}/pullRequests/{}/iterations/{iteration}/changes",
            pr.repository.id, pr.pull_request_id
        );
        let page: Changes = self.get(&path, &[("$top", "100")], Some(API_VERSION)).await?;
        Ok(page.change_entries.unwrap_or_default())
    }

    /// Policy checks for every pull request, and changed files for those you review. Best-effort: a call that fails
    /// leaves that part out and is returned, so the fetch can say so.
    async fn item(&self, pr: PullRequest, me: &str) -> (InboxItem, Option<PluginError>) {
        let authored = pr.created_by.as_ref().and_then(|c| c.id.as_deref()) == Some(me);
        let files = async {
            if authored {
                return (None, None);
            }
            let iteration = self.last_iteration(&pr).await;
            let changes = match &iteration {
                Ok(Some(last)) => Some(self.changes(&pr, last.id).await),
                _ => None,
            };
            (Some(iteration), changes)
        };
        let (checks, (iteration, changes)) = futures::join!(self.evaluations(&pr), files);
        let failed = [
            checks.as_ref().err(),
            iteration.as_ref().and_then(|i| i.as_ref().err()),
            changes.as_ref().and_then(|c| c.as_ref().err()),
        ]
        .into_iter()
        .flatten()
        .next()
        .cloned();
        let item = Self::map(
            pr,
            &checks.unwrap_or_default(),
            iteration.and_then(Result::ok).flatten(),
            changes.and_then(Result::ok),
            authored,
            &self.account_id,
            &self.host,
        );
        (item, failed)
    }

    /// The query for ids, then their fields in one request. Returns whether the query stopped at its limit.
    async fn work_items(&self) -> Result<(Vec<WorkItem>, bool), PluginError> {
        let top = PAGE_SIZE.to_string();
        let url = url_with_query(&self.host, "_apis/wit/wiql", &[("$top", &top), ("api-version", API_VERSION)])?;
        let request = Request::post_json(url, &serde_json::json!({ "query": ASSIGNED_WIQL }))?;
        let found =
            decode::<WiqlResult>(self.http.as_ref(), self.request(request)).await?.work_items.unwrap_or_default();
        if found.is_empty() {
            return Ok((vec![], false));
        }
        let ids = found.iter().map(|r| r.id.to_string()).collect::<Vec<_>>().join(",");
        let list: List<WorkItem> =
            self.get("_apis/wit/workitems", &[("ids", &ids), ("errorPolicy", "omit")], Some(API_VERSION)).await?;
        // The order of the query: most recently changed first.
        let mut by_id: HashMap<u64, WorkItem> = list.value.into_iter().map(|w| (w.id, w)).collect();
        let cut = found.len() >= PAGE_SIZE;
        Ok((found.iter().filter_map(|r| by_id.remove(&r.id)).collect(), cut))
    }

    /// Each pull request once, yours and those still waiting on your vote: one you voted on or declined is no longer
    /// your turn, as GitHub drops a review request once you review.
    fn pull_requests(all: Vec<PullRequest>, me: &str) -> Vec<PullRequest> {
        let mut seen = HashSet::new();
        all.into_iter()
            .filter(|pr| {
                if !seen.insert(pr.pull_request_id) {
                    return false;
                }
                if pr.created_by.as_ref().and_then(|c| c.id.as_deref()) == Some(me) {
                    return true;
                }
                pr.reviewers
                    .iter()
                    .flatten()
                    .find(|r| r.id.as_deref() == Some(me))
                    .is_some_and(|mine| mine.vote.unwrap_or(0) == 0 && mine.has_declined != Some(true))
            })
            .collect()
    }

    fn map(
        pr: PullRequest,
        evaluations: &[Evaluation],
        iteration: Option<Iteration>,
        changes: Option<Vec<Change>>,
        authored: bool,
        account_id: &str,
        host: &str,
    ) -> InboxItem {
        let reviewers = pr.reviewers.unwrap_or_default();
        let votes: Vec<i32> = reviewers.iter().map(|r| r.vote.unwrap_or(0)).collect();
        let changes_requested = votes.iter().any(|v| *v < 0);
        let review = CodeReview {
            is_draft: pr.is_draft.unwrap_or(false),
            is_approved: !changes_requested && votes.iter().any(|v| *v >= 5),
            changes_requested,
            has_conflicts: pr.merge_status.as_deref() == Some("conflicts"),
            checks: checks(evaluations),
            comments: 0,
            additions: None,
            deletions: None,
        };
        let participants = reviewers
            .iter()
            .map(|r| {
                let vote = r.vote.unwrap_or(0);
                let review = if vote >= 5 {
                    Some(ReviewState::Approved)
                } else if vote < 0 {
                    Some(ReviewState::ChangesRequested)
                } else {
                    None
                };
                Person { name: r.display_name.clone(), avatar_url: None, review, bot: false }
            })
            .collect();
        let files: Vec<ChangedFile> = changes
            .unwrap_or_default()
            .into_iter()
            .filter_map(|c| c.item)
            .filter(|i| i.is_folder != Some(true))
            .filter_map(|i| i.path)
            .map(|path| ChangedFile { path: path.trim_start_matches('/').into(), additions: None, deletions: None })
            .collect();
        let pushed = iteration.and_then(|i| i.updated_date.or(i.created_date));
        let repository = &pr.repository;
        let id = pr.pull_request_id.to_string();
        InboxItem {
            id: InboxItem::make_id(account_id, format!("pr/{id}")),
            account_id: account_id.into(),
            plugin_id: "azuredevops".into(),
            bundle: if authored { InboxBundle::authored() } else { InboxBundle::reviews() },
            title: remora_core::text::readable(&pr.title),
            context: format!("{} !{id}", repository.full_name()),
            preview: None,
            url: web_url(host, &[&repository.project.name, "_git", &repository.name, "pullrequest", &id]),
            app_url: None,
            author: pr.created_by.map(|c| Person { name: c.display_name, avatar_url: None, review: None, bot: false }),
            participants,
            badges: review.badges(authored),
            // The list has no last-updated date: the latest push stands for it when known.
            date: pushed.map_or(pr.creation_date, |p| p.max(pr.creation_date)),
            needs_action: review.needs_action(authored),
            priority: review.priority(authored),
            due: None,
            expires: None,
            changes: (!files.is_empty()).then(|| ChangeSet::new(files, None)),
            suggested_people: None,
            recurrence: None,
            reactions: None,
            reason: None,
        }
    }
}

/// Build and status policies that apply: one rejected fails, one queued or running runs, else they pass.
fn checks(evaluations: &[Evaluation]) -> Checks {
    let states: Vec<&str> = evaluations
        .iter()
        .filter(|e| {
            let config = e.configuration.as_ref();
            config.and_then(|c| c.kind.as_ref()).is_some_and(|k| CHECK_POLICIES.contains(&k.id.as_str()))
                && config.and_then(|c| c.is_enabled) != Some(false)
                && e.status != "notApplicable"
        })
        .map(|e| e.status.as_str())
        .collect();
    if states.is_empty() {
        Checks::None
    } else if states.iter().any(|s| matches!(*s, "rejected" | "broken")) {
        Checks::Failing
    } else if states.iter().any(|s| matches!(*s, "queued" | "running")) {
        Checks::Running
    } else {
        Checks::Passing
    }
}

/// The host followed by each segment, encoded: project and repository names may hold spaces.
fn web_url(host: &str, segments: &[&str]) -> Option<String> {
    let mut url = reqwest::Url::parse(host).ok()?;
    url.path_segments_mut().ok()?.pop_if_empty().extend(segments);
    Some(url.to_string())
}

#[async_trait]
impl SourcePlugin for AzureDevOpsPlugin {
    async fn fetch(&self) -> Result<SourceSnapshot, PluginError> {
        let me: ConnectionUser = self
            .cache
            .value("user", IDENTITY_LIFETIME, || async {
                let data: ConnectionData = self.get("_apis/connectionData", &[], None).await?;
                Ok(data.authenticated_user)
            })
            .await?;
        let top = PAGE_SIZE.to_string();
        let base = [("searchCriteria.status", "active"), ("$top", top.as_str())];
        let authored_query = [base.as_slice(), &[("searchCriteria.creatorId", me.id.as_str())]].concat();
        let reviewing_query = [base.as_slice(), &[("searchCriteria.reviewerId", me.id.as_str())]].concat();
        let (authored, reviewing, work) = futures::join!(
            self.get::<List<PullRequest>>("_apis/git/pullrequests", &authored_query, Some(API_VERSION)),
            self.get::<List<PullRequest>>("_apis/git/pullrequests", &reviewing_query, Some(API_VERSION)),
            self.work_items()
        );
        let (authored, reviewing) = (authored?.value, reviewing?.value);
        let mut cut = authored.len() >= PAGE_SIZE || reviewing.len() >= PAGE_SIZE;
        let pulls = Self::pull_requests(authored.into_iter().chain(reviewing).collect(), &me.id);
        let mut items = Vec::with_capacity(pulls.len());
        let mut failed = None;
        let mut pending = pulls.into_iter().peekable();
        while pending.peek().is_some() {
            let chunk: Vec<_> = pending.by_ref().take(4).collect();
            for (item, error) in join_all(chunk.into_iter().map(|pr| self.item(pr, &me.id))).await {
                items.push(item);
                failed = failed.or(error);
            }
        }
        let mut remarks = vec![];
        if let Some(error) = failed {
            if let Some(limit) = crate::rate_limit_of(&error) {
                return Err(limit);
            }
            // Without the checks, a failed build would look fine: say so.
            remarks.push(crate::partial("Policy checks or changed files", &error));
        }
        // Work items are a bonus, as on the Mac: their failure is a remark, and pull requests still come through.
        match work {
            Ok((work, stopped)) => {
                cut |= stopped;
                items.extend(work.iter().map(|w| w.item(&self.host, &self.account_id)));
            }
            Err(error) => match crate::rate_limit_of(&error) {
                Some(limit) => return Err(limit),
                None => remarks.push(crate::partial("Work items", &error)),
            },
        }
        if cut {
            remarks.insert(0, crate::truncated("Azure DevOps"));
        }
        Ok(SourceSnapshot { identity: me.provider_display_name, items, remarks })
    }
}

#[derive(Deserialize)]
struct List<T> {
    value: Vec<T>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConnectionData {
    authenticated_user: ConnectionUser,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ConnectionUser {
    id: String,
    provider_display_name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Identity {
    id: Option<String>,
    display_name: String,
}

#[derive(Deserialize)]
struct Project {
    id: String,
    name: String,
}

#[derive(Deserialize)]
struct Repository {
    id: String,
    name: String,
    project: Project,
}

impl Repository {
    /// "Acme/app", or "app" when the repository is named after its project.
    fn full_name(&self) -> String {
        if self.name == self.project.name {
            self.name.clone()
        } else {
            format!("{}/{}", self.project.name, self.name)
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Reviewer {
    id: Option<String>,
    display_name: String,
    /// 10 approved, 5 approved with suggestions, 0 no vote, -5 waiting for the author, -10 rejected.
    vote: Option<i32>,
    has_declined: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PullRequest {
    pull_request_id: u64,
    code_review_id: Option<u64>,
    title: String,
    is_draft: Option<bool>,
    creation_date: DateTime<Utc>,
    merge_status: Option<String>,
    created_by: Option<Identity>,
    repository: Repository,
    reviewers: Option<Vec<Reviewer>>,
}

#[derive(Deserialize)]
struct PolicyKind {
    id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PolicyConfiguration {
    is_enabled: Option<bool>,
    #[serde(rename = "type")]
    kind: Option<PolicyKind>,
}

#[derive(Deserialize)]
struct Evaluation {
    /// approved, rejected, running, queued, notApplicable, broken.
    status: String,
    configuration: Option<PolicyConfiguration>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Iteration {
    id: u64,
    created_date: Option<DateTime<Utc>>,
    updated_date: Option<DateTime<Utc>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Changes {
    change_entries: Option<Vec<Change>>,
}

#[derive(Deserialize)]
struct Change {
    item: Option<ChangeItem>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChangeItem {
    path: Option<String>,
    is_folder: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WiqlResult {
    work_items: Option<Vec<WorkItemReference>>,
}

#[derive(Deserialize)]
struct WorkItemReference {
    id: u64,
}

#[derive(Deserialize)]
struct WorkItemFields {
    #[serde(rename = "System.Title")]
    title: Option<String>,
    #[serde(rename = "System.State")]
    state: Option<String>,
    #[serde(rename = "System.WorkItemType")]
    kind: Option<String>,
    #[serde(rename = "System.TeamProject")]
    project: Option<String>,
    #[serde(rename = "System.ChangedDate")]
    changed: Option<DateTime<Utc>>,
    #[serde(rename = "Microsoft.VSTS.Common.Priority")]
    priority: Option<i64>,
    #[serde(rename = "Microsoft.VSTS.Scheduling.DueDate")]
    due: Option<String>,
}

#[derive(Deserialize)]
struct WorkItem {
    id: u64,
    fields: WorkItemFields,
}

impl WorkItem {
    /// Priority 1 is high, 2 normal (the default), 3 and 4 low.
    fn item_priority(&self) -> Priority {
        match self.fields.priority {
            Some(1) => Priority::High,
            Some(3 | 4) => Priority::Low,
            _ => Priority::Normal,
        }
    }

    fn item(&self, host: &str, account_id: &str) -> InboxItem {
        let priority = self.item_priority();
        let mut badges = vec![];
        if priority == Priority::High {
            badges.push(Badge::new(badge::PRIORITY_HIGH, "High", Tone::Warning));
        }
        let due = self.fields.due.as_deref().and_then(due_date);
        if let Some(due) = due {
            badges.push(due_badge(due));
        }
        if let Some(state) = &self.fields.state {
            badges.push(Badge::new(badge::STATUS, state, Tone::Neutral));
        }
        let id = self.id.to_string();
        let kind = [self.fields.kind.as_deref(), Some(id.as_str())].into_iter().flatten().collect::<Vec<_>>().join(" ");
        let context =
            [self.fields.project.as_deref(), Some(kind.as_str())].into_iter().flatten().collect::<Vec<_>>().join(" · ");
        InboxItem {
            id: InboxItem::make_id(account_id, format!("workitem/{id}")),
            account_id: account_id.into(),
            plugin_id: "azuredevops".into(),
            bundle: InboxBundle::tasks(),
            title: remora_core::text::readable(self.fields.title.as_deref().unwrap_or(&kind)),
            context,
            preview: None,
            url: self.fields.project.as_deref().and_then(|p| web_url(host, &[p, "_workitems", "edit", &id])),
            app_url: None,
            author: None,
            participants: vec![],
            badges,
            date: self.fields.changed.unwrap_or_else(Utc::now),
            needs_action: priority != Priority::Low,
            priority: Some(priority),
            due,
            expires: None,
            changes: None,
            suggested_people: None,
            recurrence: None,
            reactions: None,
            reason: None,
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::stub::*;

    // The same fixtures as the macOS tests (`AzureDevOpsPluginTests.swift`).
    const ME: &str = r#"{"authenticatedUser": {"id": "a1", "providerDisplayName": "Alice Martin"}}"#;
    /// Both lists answer the same: the plugin tells yours from those you review by who created them.
    const PULLS: &str = r#"{"count": 3, "value": [
      {"pullRequestId": 12, "codeReviewId": 12, "title": "Speed up *CI*", "status": "active", "isDraft": false,
       "creationDate": "2026-10-08T10:00:00.1234567Z", "mergeStatus": "succeeded",
       "createdBy": {"id": "a1", "displayName": "Alice Martin"},
       "repository": {"id": "r1", "name": "app", "project": {"id": "p1", "name": "Acme"}},
       "reviewers": [{"id": "e1", "displayName": "Erin", "vote": 10}, {"id": "f1", "displayName": "Frank", "vote": 0}]},
      {"pullRequestId": 13, "codeReviewId": 13, "title": "Add avatars", "status": "active",
       "creationDate": "2026-10-09T08:00:00Z", "mergeStatus": "conflicts",
       "createdBy": {"id": "b1", "displayName": "Bob"},
       "repository": {"id": "r1", "name": "app", "project": {"id": "p1", "name": "Acme"}},
       "reviewers": [{"id": "a1", "displayName": "Alice Martin", "vote": 0, "isRequired": true},
                     {"id": "e1", "displayName": "Erin", "vote": -5}]},
      {"pullRequestId": 14, "codeReviewId": 14, "title": "Old cleanup", "status": "active",
       "creationDate": "2026-10-01T08:00:00Z", "createdBy": {"id": "c1", "displayName": "Carol"},
       "repository": {"id": "r2", "name": "Acme", "project": {"id": "p1", "name": "Acme"}},
       "reviewers": [{"id": "a1", "displayName": "Alice Martin", "vote": 10}]}
    ]}"#;
    const EVALUATIONS: &str = r#"{"value": [
      {"status": "approved", "configuration": {"isEnabled": true, "type": {"id": "fa4e907d-c16b-4a4c-9dfa-4916e5d171ab"}}},
      {"status": "rejected", "configuration": {"isEnabled": true, "type": {"id": "0609b952-1397-4640-95ec-e00a01b2c241"}}}
    ]}"#;
    const ITERATIONS: &str = r#"{"value": [{"id": 1, "createdDate": "2026-10-09T08:00:00Z"},
               {"id": 2, "createdDate": "2026-10-10T07:00:00Z", "updatedDate": "2026-10-10T07:00:00Z"}]}"#;
    const CHANGES: &str = r#"{"changeEntries": [{"item": {"path": "/src/db/add_avatars.sql"}, "changeType": "add"},
                       {"item": {"path": "/src", "isFolder": true}, "changeType": "edit"}]}"#;
    const WIQL: &str = r#"{"workItems": [{"id": 42}, {"id": 43}]}"#;
    const WORK_ITEMS: &str = r#"{"count": 2, "value": [
      {"id": 43, "fields": {"System.Title": "Tidy the logs", "System.State": "New", "System.WorkItemType": "Task",
        "System.TeamProject": "Acme", "System.ChangedDate": "2026-10-11T09:00:00Z", "Microsoft.VSTS.Common.Priority": 4}},
      {"id": 42, "fields": {"System.Title": "Export *CSV*", "System.State": "Active", "System.WorkItemType": "Bug",
        "System.TeamProject": "Acme", "System.ChangedDate": "2026-10-12T09:00:00.12Z", "Microsoft.VSTS.Common.Priority": 1,
        "Microsoft.VSTS.Scheduling.DueDate": "2020-01-01T00:00:00Z", "System.Description": "<p>Not decoded</p>"}}
    ]}"#;

    fn routes() -> Vec<(&'static str, &'static str)> {
        vec![
            ("/_apis/connectionData", ME),
            ("/_apis/git/pullrequests", PULLS),
            ("/_apis/policy/evaluations", EVALUATIONS),
            ("/pullRequests/13/iterations", ITERATIONS),
            ("/pullRequests/13/iterations/2/changes", CHANGES),
            ("/_apis/wit/wiql", WIQL),
            ("/_apis/wit/workitems", WORK_ITEMS),
        ]
    }

    /// Checks the token goes as Basic's password, then answers like `StubHttp`, or 429 on `failing`.
    struct Stub(StubHttp, Option<&'static str>);

    #[async_trait::async_trait]
    impl HttpClient for Stub {
        async fn send(&self, request: Request) -> Result<crate::Response, PluginError> {
            let header = |name: &str| request.headers.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone());
            assert_eq!(header("Authorization").as_deref(), Some("Basic OnQ="));
            assert_eq!(header("X-TFS-FedAuthRedirect").as_deref(), Some("Suppress"));
            let path = reqwest::Url::parse(&request.url).map(|u| u.path().to_string()).unwrap_or_default();
            if self.1.is_some_and(|failing| path.ends_with(failing)) {
                return Ok(crate::Response { status: 429, body: String::new(), etag: None, reset: None });
            }
            self.0.send(request).await
        }
    }

    /// The fetch `registry`'s egress test runs through the guard.
    pub(crate) fn egress_fixture() -> EgressFixture {
        EgressFixture {
            settings: vec![("host", "dev.azure.com/acme/"), ("token", "t")],
            http: Arc::new(Stub(StubHttp::paths(&routes()), None)),
        }
    }

    fn plugin(routes: &[(&'static str, &str)], failing: Option<&'static str>) -> AzureDevOpsPlugin {
        let http = Arc::new(Stub(StubHttp::paths(routes), failing));
        AzureDevOpsPlugin::new(&config(&[("host", "dev.azure.com/acme/"), ("token", "t")]), http).unwrap()
    }

    #[tokio::test]
    async fn pull_requests_with_votes_checks_and_files_then_work_items() {
        let snapshot = plugin(&routes(), None).fetch().await.unwrap();

        assert_eq!(snapshot.identity, "Alice Martin");
        assert!(snapshot.remarks.is_empty());
        assert_eq!(
            snapshot.items.iter().map(|i| i.bundle.clone()).collect::<Vec<_>>(),
            [InboxBundle::authored(), InboxBundle::reviews(), InboxBundle::tasks(), InboxBundle::tasks()],
            "already voted on: left out"
        );

        let mine = &snapshot.items[0];
        assert_eq!(mine.title, "Speed up CI");
        assert_eq!(mine.context, "Acme/app !12");
        assert_eq!(mine.url.as_deref(), Some("https://dev.azure.com/acme/Acme/_git/app/pullrequest/12"));
        assert!(mine.has_badge("approved") && mine.has_badge("checks.failing") && mine.needs_action);
        assert_eq!(
            mine.participants.iter().find(|p| p.name == "Erin").and_then(|p| p.review),
            Some(ReviewState::Approved)
        );
        assert_eq!(mine.participants.iter().find(|p| p.name == "Frank").and_then(|p| p.review), None);
        assert_eq!(mine.changes, None, "your own pull requests don't need a review prep");

        let review = &snapshot.items[1];
        assert_eq!(review.author.as_ref().map(|a| a.name.as_str()), Some("Bob"));
        assert!(review.has_badge("changes") && review.has_badge("conflicts") && !review.has_badge("approved"));
        assert_eq!(
            review.participants.iter().find(|p| p.name == "Erin").and_then(|p| p.review),
            Some(ReviewState::ChangesRequested)
        );
        assert_eq!(
            review.changes.as_ref().map(|c| c.files.clone()),
            Some(vec![ChangedFile { path: "src/db/add_avatars.sql".into(), additions: None, deletions: None }])
        );
        assert_eq!(review.date.to_rfc3339(), "2026-10-10T07:00:00+00:00", "the latest push");

        let bug = &snapshot.items[2];
        assert_eq!(bug.title, "Export CSV");
        assert_eq!(bug.context, "Acme · Bug 42");
        assert_eq!(bug.url.as_deref(), Some("https://dev.azure.com/acme/Acme/_workitems/edit/42"));
        assert!(bug.priority == Some(Priority::High) && bug.has_badge("priority.high") && bug.has_badge("overdue"));
        assert!(bug.has_badge("status") && bug.needs_action);
        let task = &snapshot.items[3];
        assert!(task.priority == Some(Priority::Low) && !task.needs_action);
    }

    /// Checks or work items that can't be read are said; a rate limit fails the fetch, so the backoff waits it out.
    #[tokio::test]
    async fn failed_parts_are_said_and_a_rate_limit_is_waited_out() {
        let routes: Vec<_> = routes()
            .into_iter()
            .filter(|(path, _)| !matches!(*path, "/_apis/policy/evaluations" | "/_apis/wit/wiql"))
            .collect();
        let snapshot = plugin(&routes, None).fetch().await.unwrap();
        assert_eq!(snapshot.items.len(), 2, "the pull requests still come");
        assert_eq!(
            snapshot.remarks.iter().map(|r| r.args[0].as_str()).collect::<Vec<_>>(),
            ["Policy checks or changed files", "Work items"]
        );

        let limited = plugin(&super::tests::routes(), Some("/iterations")).fetch().await;
        assert_eq!(limited.err(), Some(PluginError::RateLimited(None)));
    }

    #[test]
    fn build_and_status_policies_are_the_checks() {
        let evaluation = |status: &str, kind: &str| Evaluation {
            status: status.into(),
            configuration: Some(PolicyConfiguration {
                is_enabled: Some(true),
                kind: Some(PolicyKind { id: kind.into() }),
            }),
        };
        let build = "0609b952-1397-4640-95ec-e00a01b2c241";
        let reviewers = "fa4e907d-c16b-4a4c-9dfa-4916e5d171ab";
        assert!(checks(&[]) == Checks::None);
        assert!(checks(&[evaluation("rejected", reviewers)]) == Checks::None, "votes aren't checks");
        assert!(checks(&[evaluation("approved", build), evaluation("running", build)]) == Checks::Running);
        assert!(checks(&[evaluation("approved", build), evaluation("notApplicable", build)]) == Checks::Passing);
        assert!(checks(&[evaluation("queued", build), evaluation("broken", build)]) == Checks::Failing);
    }
}

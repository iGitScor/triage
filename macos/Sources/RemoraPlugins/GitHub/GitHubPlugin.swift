import Foundation
import RemoraCore

/// Pull requests you opened and those awaiting your review, through a single GraphQL query; for the repositories you
/// maintain, a failing default branch and open critical or high security alerts, through the REST API.
public struct GitHubPlugin: SourcePlugin {
    public static let manifest = PluginManifest(
        id: "github",
        name: "GitHub",
        symbol: "arrow.triangle.pull",
        summary: "Pull requests you opened, review requests and issues assigned to you. Works with GitHub Enterprise.",
        fields: [
            .host("https://github.com"),
            .token(
                "Personal access token",
                help:
                    "A fine-grained token with read-only Pull requests and Issues, or a classic token with repo and read:org."
            ),
            ConfigField(
                key: "repositories", label: "Repositories I maintain", placeholder: "acme/web, acme/api",
                isOptional: true,
                help:
                    "Optional. A failing default branch and open critical or high security alerts go to To fix. The token also needs read-only Actions, Dependabot alerts and Code scanning alerts."
            ),
        ],
        setupSteps: [
            // Read-only and limited to what Remora reads, rather than the whole `repo` scope.
            "Click “Create a token”: GitHub opens a fine-grained token named Remora, with read-only Pull requests and Issues.",
            "Pick the owner (you, or your organization) and All repositories, set an expiration, generate it and paste it below.",
            "Repositories of several organizations: connect GitHub once per organization, or use a classic token with repo and read:org.",
            "To see the checks and security alerts of repositories you maintain, also give the token read-only Actions, Dependabot alerts and Code scanning alerts.",
        ],
        setupURL: { config in
            let host = (try? config.url("host")) ?? URL(string: "https://github.com")!
            return host.appending(
                path: "settings/personal-access-tokens/new",
                query: [
                    "name": "Remora", "description": "Remora reads your pull requests, reviews and issues",
                    "pull_requests": "read", "issues": "read", "metadata": "read",
                ])
        },
        egress: Egress(
            hosts: ["api.github.com", "github.com", "avatars.githubusercontent.com"],
            description:
                "Reads your pull requests, review requests and assigned issues from GitHub (or your GitHub Enterprise host), and the checks and security alerts of the repositories you maintain."
        ),
        logo: "github"
    )

    private let accountID: UUID
    private let endpoint: URL
    /// The REST API: api.github.com, or the Enterprise host's /api/v3.
    private let rest: URL
    private let token: String
    private let repositories: [String]
    private let http: HTTPClient

    public init(config: PluginConfig, http: HTTPClient) throws {
        let host = try config.url("host")
        accountID = config.accountID
        endpoint =
            host.host == "github.com"
            ? URL(string: "https://api.github.com/graphql")!
            : host.appending(path: "api/graphql")
        rest = host.host == "github.com" ? URL(string: "https://api.github.com")! : host.appending(path: "api/v3")
        token = try config.required("token")
        repositories = Maintained.repositories(config.values["repositories"], nested: false)
        self.http = http
    }

    public func fetch() async throws -> SourceSnapshot {
        let body = GraphQLRequest(
            query: Self.query,
            variables: [
                "authored": "is:pr is:open archived:false author:@me sort:updated-desc",
                "reviewing": "is:pr is:open archived:false review-requested:@me sort:updated-desc",
                "assigned": "is:issue is:open archived:false assignee:@me sort:updated-desc",
            ])
        let request = try URLRequest.post(endpoint, json: body, headers: ["Authorization": "bearer \(token)"])
        let (response, answer) = try await http.decodeWithResponse(GraphQLResponse.self, from: request)
        var snapshot = try Self.snapshot(from: response, accountID: accountID, resume: answer.resumeDate())
        for repository in repositories {
            snapshot.items += try await maintained(repository, remarks: &snapshot.remarks)
        }
        return snapshot
    }

    /// The security alerts listed per repository, most recent first.
    static let alertsPerRepository = 10

    private func get<T: Decodable>(_ repository: String, _ path: String = "", _ query: [String: String] = [:])
        async throws -> T
    {
        let url = rest.appending(path: "repos/\(repository)\(path.isEmpty ? "" : "/\(path)")", query: query)
        let headers = ["Authorization": "Bearer \(token)", "Accept": "application/vnd.github+json"]
        return try await http.decode(T.self, from: .get(url, headers: headers), using: .api(snakeCase: true))
    }

    /// A repository you maintain: its default branch's last finished run when it failed, and its open critical or
    /// high security alerts (Dependabot, and code scanning where it is set up). Best effort: a part that can't be
    /// read is a remark, except a rate limit, which fails the fetch. Same as Windows.
    private func maintained(_ repository: String, remarks: inout [String]) async throws -> [InboxItem] {
        guard
            let info: Repo = try await BestEffort.value(
                "Repositories you maintain", remarks: &remarks, { try await get(repository) })
        else { return [] }
        var items: [InboxItem] = []
        let branch = info.defaultBranch
        do {
            let runs: Runs = try await get(
                repository, "actions/runs",
                ["branch": branch, "status": "completed", "per_page": "1", "exclude_pull_requests": "true"])
            if let item = runs.workflowRuns.first?.failing(accountID: accountID, repository: repository, branch: branch)
            {
                items.append(item)
            }
        } catch HTTPError.status(404) {
            // No Actions on this repository.
        } catch {
            try BestEffort.note("Checks on the default branch", error, remarks: &remarks)
        }
        let severe = ["state": "open", "severity": "critical,high", "per_page": String(Self.alertsPerRepository)]
        let dependabot: [DependabotAlert]? = try await BestEffort.value("Security alerts", remarks: &remarks) {
            try await get(repository, "dependabot/alerts", severe)
        }
        items += (dependabot ?? []).map { $0.item(accountID: accountID, repository: repository) }
        do {
            let scanning: [CodeScanningAlert] = try await get(repository, "code-scanning/alerts", severe)
            items += scanning.map { $0.item(accountID: accountID, repository: repository) }
        } catch HTTPError.status(let code) where code == 403 || code == 404 {
            // Code scanning needs GitHub Advanced Security, often off: no remark at every refresh.
        } catch {
            try BestEffort.note("Security alerts", error, remarks: &remarks)
        }
        return items
    }

    static func snapshot(from response: GraphQLResponse, accountID: UUID, resume: Date? = nil) throws -> SourceSnapshot
    {
        // GraphQL points used up: a 200 whose error says so, the reset time in the headers.
        if response.errors?.contains(where: { $0.type == "RATE_LIMITED" }) == true {
            throw HTTPError.rateLimited(resume)
        }
        guard let data = response.data else {
            let message = response.errors?.first?.message ?? L("GitHub returned no data.")
            // A token GitHub no longer accepts: the user reconnects.
            if message.caseInsensitiveCompare("Bad credentials") == .orderedSame { throw HTTPError.unauthorized }
            throw HTTPError.api(message)
        }
        let authored = data.authored.nodes.compactMap { $0 }.map {
            $0.item(accountID: accountID, bundle: .authored, authored: true)
        }
        let reviewing = data.reviewing.nodes.compactMap { $0 }.map {
            $0.item(accountID: accountID, bundle: .reviews, authored: false)
        }
        let assigned = data.assigned?.nodes.compactMap { $0 }.map { $0.item(accountID: accountID) } ?? []
        let truncated =
            [data.authored, data.reviewing].contains { ($0.issueCount ?? 0) > $0.nodes.count }
            || (data.assigned.map { ($0.issueCount ?? 0) > $0.nodes.count } ?? false)
        return SourceSnapshot(
            identity: data.viewer.login, items: authored + reviewing + assigned,
            remarks: truncated ? [SourceSnapshot.truncated("GitHub")] : []
        )
    }

    static let query = """
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
        """
}

// MARK: - Wire format

struct GraphQLRequest: Encodable {
    var query: String
    var variables: [String: String]
}

struct GraphQLResponse: Decodable {
    struct Payload: Decodable {
        var viewer: Login
        var authored: Search
        var reviewing: Search
        /// Issues assigned to you. Absent from an answer to the older query.
        var assigned: IssueSearch?
    }

    struct IssueSearch: Decodable {
        var issueCount: Int?
        var nodes: [AssignedIssue?]
    }

    struct Login: Decodable { var login: String }
    struct Search: Decodable {
        /// All the matches, beyond the 50 listed.
        var issueCount: Int?
        var nodes: [PullRequest?]
    }
    struct Message: Decodable {
        var message: String
        var type: String?
    }

    var data: Payload?
    var errors: [Message]?
}

struct Connection<Node: Decodable>: Decodable {
    var nodes: [Node?]?
    var all: [Node] { (nodes ?? []).compactMap { $0 } }
}

/// An issue assigned to you: something to do, like a Linear or Jira issue. Same as Windows (`github.rs`).
struct AssignedIssue: Decodable {
    var id: String
    var number: Int
    var title: String
    var url: URL
    var updatedAt: Date
    var repository: PullRequest.Repository
    var author: PullRequest.Actor?

    func item(accountID: UUID) -> InboxItem {
        InboxItem(
            id: InboxItem.makeID(account: accountID, local: "\(id)"),
            accountID: accountID,
            pluginID: GitHubPlugin.manifest.id,
            bundle: .tasks,
            title: Readable.text(title),
            context: "\(repository.nameWithOwner) #\(number)",
            url: url,
            author: author?.person,
            date: updatedAt,
            needsAction: true
        )
    }
}

struct PullRequest: Decodable {
    struct Actor: Decodable {
        /// "Bot" for an app account (Dependabot, Renovate, an agent): GitHub says it, no name is guessed.
        var typename: String?
        var login: String?
        var name: String?
        var avatarUrl: String?
        var person: Person {
            Person(
                name: login ?? name ?? "?", avatarURL: URL(lenient: avatarUrl),
                bot: typename == "Bot" || login?.hasSuffix("[bot]") == true)
        }

        private enum CodingKeys: String, CodingKey {
            case typename = "__typename"
            case login, name, avatarUrl
        }
    }

    struct Review: Decodable {
        var state: String
        var author: Actor?
    }
    struct Request: Decodable { var requestedReviewer: Actor? }
    struct Commit: Decodable { var commit: Rollup }
    struct Rollup: Decodable { var statusCheckRollup: State? }
    struct State: Decodable { var state: String }
    struct Count: Decodable { var totalCount: Int }
    struct Repository: Decodable { var nameWithOwner: String }

    var id: String
    var number: Int
    var title: String
    var url: URL
    var isDraft: Bool
    var updatedAt: Date
    var additions: Int?
    var deletions: Int?
    var mergeable: String?
    var reviewDecision: String?
    var repository: Repository
    var author: Actor?
    var comments: Count?
    var latestOpinionatedReviews: Connection<Review>?
    var reviewRequests: Connection<Request>?
    var commits: Connection<Commit>?
    var suggestedReviewers: [Suggested?]?
    var changedFiles: Int?
    var files: Connection<File>?
    struct File: Decodable {
        var path: String
        var additions: Int?
        var deletions: Int?
    }
    struct Suggested: Decodable { var reviewer: Actor? }

    func item(accountID: UUID, bundle: InboxBundle, authored: Bool) -> InboxItem {
        let reviews = latestOpinionatedReviews?.all ?? []
        let decision = reviewDecision ?? Self.decision(reviews.map(\.state))
        let review = CodeReview(
            isDraft: isDraft,
            isApproved: decision == "APPROVED",
            changesRequested: decision == "CHANGES_REQUESTED",
            hasConflicts: mergeable == "CONFLICTING",
            checks: checks,
            comments: comments?.totalCount ?? 0,
            additions: additions,
            deletions: deletions
        )
        return InboxItem(
            id: InboxItem.makeID(account: accountID, local: "\(id)"),
            accountID: accountID,
            pluginID: GitHubPlugin.manifest.id,
            bundle: bundle,
            title: Readable.text(title),
            context: "\(repository.nameWithOwner) #\(number)",
            url: url,
            author: author?.person,
            participants: participants(reviews: reviews),
            badges: review.badges(authored: authored),
            date: updatedAt,
            needsAction: review.needsAction(authored: authored),
            priority: review.priority(authored: authored),
            suggestedPeople: authored ? suggestedReviewers?.compactMap { $0?.reviewer?.person } : nil,
            changes: authored ? nil : changeSet
        )
    }

    /// GitHub gives no `reviewDecision` when the repository requires no review: the latest reviews tell then, a
    /// request for changes first.
    static func decision(_ states: [String]) -> String? {
        states.contains("CHANGES_REQUESTED") ? "CHANGES_REQUESTED" : states.contains("APPROVED") ? "APPROVED" : nil
    }

    /// Paths and line counts only, for the review prep.
    private var changeSet: ChangeSet? {
        guard let files = files?.all, !files.isEmpty else { return nil }
        return ChangeSet(
            files: files.map { ChangedFile(path: $0.path, additions: $0.additions, deletions: $0.deletions) },
            fileCount: changedFiles
        )
    }

    private var checks: CodeReview.Checks {
        switch commits?.all.last?.commit.statusCheckRollup?.state {
        case "SUCCESS": .passing
        case "FAILURE", "ERROR": .failing
        case "PENDING", "EXPECTED": .running
        default: .none
        }
    }

    private func participants(reviews: [Review]) -> [Person] {
        var people = reviews.compactMap { review -> Person? in
            guard var person = review.author?.person else { return nil }
            person.review =
                review.state == "APPROVED"
                ? ReviewState.approved
                : review.state == "CHANGES_REQUESTED" ? ReviewState.changesRequested : nil
            return person
        }
        for request in reviewRequests?.all ?? [] {
            guard let person = request.requestedReviewer?.person,
                !people.contains(where: { $0.name == person.name })
            else { continue }
            people.append(person)
        }
        return people
    }
}

// MARK: - Repositories you maintain

struct Repo: Decodable { var defaultBranch: String }

struct Runs: Decodable {
    var workflowRuns: [Run] = []

    struct Run: Decodable {
        var id: Int
        var name: String?
        var displayTitle: String?
        var conclusion: String?
        var htmlUrl: String
        var updatedAt: Date

        func failing(accountID: UUID, repository: String, branch: String) -> InboxItem? {
            guard ["failure", "timed_out", "startup_failure"].contains(conclusion ?? "") else { return nil }
            return Maintained.failing(
                accountID: accountID, pluginID: GitHubPlugin.manifest.id, local: "ci/\(repository)/\(id)",
                title: Readable.text(displayTitle ?? name ?? branch), context: "\(repository) · \(branch)",
                preview: name, url: URL(lenient: htmlUrl), date: updatedAt)
        }
    }
}

/// Critical alerts are urgent, high ones high.
private func severity(_ level: String?) -> Priority? {
    switch level {
    case "critical": .urgent
    case "high": .high
    default: nil
    }
}

private func alert(
    accountID: UUID, local: String, title: String, repository: String, url: String, date: Date, priority: Priority?
) -> InboxItem {
    InboxItem(
        id: InboxItem.makeID(account: accountID, local: local), accountID: accountID,
        pluginID: GitHubPlugin.manifest.id, bundle: .fix, title: title, context: repository, url: URL(lenient: url),
        date: date, needsAction: true, priority: priority)
}

struct DependabotAlert: Decodable {
    struct Advisory: Decodable {
        var summary: String
        var severity: String?
    }
    struct Dependency: Decodable { var package: Package? }
    struct Package: Decodable { var name: String }

    var number: Int
    var htmlUrl: String
    var createdAt: Date
    var securityAdvisory: Advisory
    var dependency: Dependency?

    func item(accountID: UUID, repository: String) -> InboxItem {
        let summary = securityAdvisory.summary
        return alert(
            accountID: accountID, local: "alert/\(repository)/dependabot/\(number)",
            title: dependency?.package.map { "\($0.name): \(summary)" } ?? summary, repository: repository,
            url: htmlUrl, date: createdAt, priority: severity(securityAdvisory.severity))
    }
}

struct CodeScanningAlert: Decodable {
    struct Rule: Decodable {
        var description: String?
        var securitySeverityLevel: String?
    }

    var number: Int
    var htmlUrl: String
    var createdAt: Date
    var rule: Rule

    func item(accountID: UUID, repository: String) -> InboxItem {
        alert(
            accountID: accountID, local: "alert/\(repository)/code-scanning/\(number)",
            title: rule.description ?? "Code scanning alert #\(number)", repository: repository, url: htmlUrl,
            date: createdAt, priority: severity(rule.securitySeverityLevel))
    }
}

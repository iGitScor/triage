import Foundation
import RemoraCore

/// Pull requests you opened and those awaiting your review, through a single GraphQL query.
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
        ],
        setupSteps: [
            // Read-only and limited to what Remora reads, rather than the whole `repo` scope.
            "Click “Create a token”: GitHub opens a fine-grained token named Remora, with read-only Pull requests and Issues.",
            "Pick the owner (you, or your organization) and All repositories, set an expiration, generate it and paste it below.",
            "Repositories of several organizations: connect GitHub once per organization, or use a classic token with repo and read:org.",
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
                "Reads your pull requests, review requests and assigned issues from GitHub (or your GitHub Enterprise host)."
        ),
        logo: "github"
    )

    private let accountID: UUID
    private let endpoint: URL
    private let token: String
    private let http: HTTPClient

    public init(config: PluginConfig, http: HTTPClient) throws {
        let host = try config.url("host")
        accountID = config.accountID
        endpoint =
            host.host == "github.com"
            ? URL(string: "https://api.github.com/graphql")!
            : host.appending(path: "api/graphql")
        token = try config.required("token")
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
        return try Self.snapshot(from: response, accountID: accountID, resume: answer.resumeDate())
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
        let review = CodeReview(
            isDraft: isDraft,
            isApproved: reviewDecision == "APPROVED",
            changesRequested: reviewDecision == "CHANGES_REQUESTED",
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
            suggestedPeople: authored ? suggestedReviewers?.compactMap { $0?.reviewer?.person } : nil,
            changes: authored ? nil : changeSet
        )
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

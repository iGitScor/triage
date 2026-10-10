import Foundation
import RemoraCore

/// Pull requests you opened and those awaiting your review, through a single GraphQL query.
public struct GitHubPlugin: SourcePlugin {
    public static let manifest = PluginManifest(
        id: "github",
        name: "GitHub",
        symbol: "arrow.triangle.pull",
        summary: "Pull requests you opened and review requests. Works with GitHub Enterprise.",
        fields: [
            .host("https://github.com"),
            .token("Personal access token", help: "Classic token with the repo and read:org scopes."),
        ],
        setupSteps: [
            "Click “Create a token”: GitHub opens with the repo and read:org scopes selected.",
            "Set an expiration, click “Generate token” and paste it below.",
        ],
        setupURL: { config in
            let host = (try? config.url("host")) ?? URL(string: "https://github.com")!
            return host.appending(
                path: "settings/tokens/new",
                query: [
                    "scopes": "repo,read:org", "description": "Remora",
                ])
        },
        egress: Egress(
            hosts: ["api.github.com", "github.com", "avatars.githubusercontent.com"],
            description: "Reads your pull requests and review requests from GitHub (or your GitHub Enterprise host)."
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
            ])
        let request = try URLRequest.post(endpoint, json: body, headers: ["Authorization": "bearer \(token)"])
        let response = try await http.decode(GraphQLResponse.self, from: request)
        return try Self.snapshot(from: response, accountID: accountID)
    }

    static func snapshot(from response: GraphQLResponse, accountID: UUID) throws -> SourceSnapshot {
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
        let truncated = [data.authored, data.reviewing].contains { ($0.issueCount ?? 0) > $0.nodes.count }
        return SourceSnapshot(
            identity: data.viewer.login, items: authored + reviewing,
            remarks: truncated ? [SourceSnapshot.truncated("GitHub")] : []
        )
    }

    static let query = """
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
    }

    struct Login: Decodable { var login: String }
    struct Search: Decodable {
        /// All the matches, beyond the 50 listed.
        var issueCount: Int?
        var nodes: [PullRequest?]
    }
    struct Message: Decodable { var message: String }

    var data: Payload?
    var errors: [Message]?
}

struct Connection<Node: Decodable>: Decodable {
    var nodes: [Node?]?
    var all: [Node] { (nodes ?? []).compactMap { $0 } }
}

struct PullRequest: Decodable {
    struct Actor: Decodable {
        var login: String?
        var name: String?
        var avatarUrl: String?
        var person: Person { Person(name: login ?? name ?? "?", avatarURL: URL(lenient: avatarUrl)) }
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
            id: "\(accountID.uuidString)/\(id)",
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
            person.tone =
                review.state == "APPROVED"
                ? ReviewerTone.approved
                : review.state == "CHANGES_REQUESTED" ? ReviewerTone.changes : ReviewerTone.waiting
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

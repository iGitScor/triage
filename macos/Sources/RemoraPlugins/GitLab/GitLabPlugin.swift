import Foundation
import RemoraCore

/// Merge requests you opened and those where you are a reviewer, on gitlab.com or self-hosted.
public struct GitLabPlugin: SourcePlugin {
    public static let manifest = PluginManifest(
        id: "gitlab",
        name: "GitLab",
        symbol: "arrow.triangle.merge",
        summary: "Merge requests you opened and those you review. Works with self-hosted GitLab.",
        fields: [
            .host("https://gitlab.com"),
            .token("Personal access token", help: "Token with the read_api scope."),
        ],
        setupSteps: [
            "If you use self-hosted GitLab, set the host first.",
            "Click “Create a token”: GitLab opens with the read_api scope selected.",
            "Click “Create personal access token” and paste it below.",
        ],
        setupURL: { config in
            let host = (try? config.url("host")) ?? URL(string: "https://gitlab.com")!
            return host.appending(path: "-/user_settings/personal_access_tokens", query: [
                "name": "Remora", "scopes": "read_api",
            ])
        },
        egress: Egress(hosts: ["gitlab.com"], description: "Reads your merge requests, approvals and pipelines from your GitLab host."),
        logo: "gitlab"
    )

    private let accountID: UUID
    private let host: URL
    private let token: String
    private let http: HTTPClient

    public init(config: PluginConfig, http: HTTPClient) throws {
        accountID = config.accountID
        host = try config.url("host")
        token = try config.required("token")
        self.http = http
    }

    public func fetch() async throws -> SourceSnapshot {
        let user: User = try await get("user")
        async let authored: [MergeRequest] = get("merge_requests", [
            "scope": "created_by_me", "state": "opened", "per_page": "50",
        ])
        async let reviewing: [MergeRequest] = get("merge_requests", [
            "scope": "all", "state": "opened", "per_page": "50", "reviewer_username": user.username,
        ])
        let all = try await [(authored, true), (reviewing, false)].flatMap { list, isAuthored in
            list.map { ($0, isAuthored) }
        }
        // Approvals, pipeline and changed files for every merge request in one GraphQL request, instead of two or
        // three REST calls each. An instance where it fails (older GitLab, GraphQL off) gets the REST calls.
        let details = all.isEmpty ? [:] : ((try? await graphQLDetails()) ?? [:])
        let items = try await all.concurrentMap(limit: 4) { mr, isAuthored in
            if let found = details[mr.id] {
                return Self.item(
                    mr: mr, approvals: found.approvals, detail: found.detail, files: isAuthored ? nil : found.files,
                    authored: isAuthored, accountID: self.accountID, host: self.host
                )
            }
            return await self.item(for: mr, authored: isAuthored)
        }
        // A full page means there may be more: GitLab's count header isn't always sent.
        let truncated = try await [authored, reviewing].contains { $0.count >= 50 }
        return SourceSnapshot(identity: user.username, items: items, remarks: truncated ? [SourceSnapshot.truncated("GitLab")] : [])
    }

    /// The REST way, one merge request at a time. Best-effort: a call that fails leaves that part out instead of
    /// failing the whole account.
    private func item(for mr: MergeRequest, authored: Bool) async -> InboxItem {
        let base = "projects/\(mr.projectId)/merge_requests/\(mr.iid)"
        async let approvals: Approvals? = try? get("\(base)/approvals")
        async let detail: Detail? = try? get(base)
        // Review prep: file paths only. The diff text in this response is never decoded.
        let files: [DiffFile]? = authored ? nil : try? await get("\(base)/diffs", ["per_page": "50"])
        return await Self.item(
            mr: mr, approvals: approvals ?? Approvals(), detail: detail ?? Detail(), files: files,
            authored: authored, accountID: accountID, host: host
        )
    }

    static let detailsQuery = """
    query { currentUser {
      authored: authoredMergeRequests(state: opened, first: 50) { nodes { ...status } }
      reviewing: reviewRequestedMergeRequests(state: opened, first: 50) { nodes { ...status diffStats { path additions deletions } } }
    } }
    fragment status on MergeRequest { id approved approvedBy { nodes { username avatarUrl } } headPipeline { status } }
    """

    /// The details of every open merge request you wrote or review, keyed by its REST id.
    private func graphQLDetails() async throws -> [Int: GraphQL.Details] {
        let request = try URLRequest.post(host.appending(path: "api/graphql"), json: ["query": Self.detailsQuery], headers: ["PRIVATE-TOKEN": token])
        let response = try await http.decode(GraphQL.Response.self, from: request, using: .api())
        return response.details
    }

    static func item(
        mr: MergeRequest,
        approvals: Approvals,
        detail: Detail,
        files: [DiffFile]? = nil,
        authored: Bool,
        accountID: UUID,
        host: URL
    ) -> InboxItem {
        let approvers = Set((approvals.approvedBy ?? []).map(\.user.username))
        let isDraft = mr.draft ?? mr.workInProgress ?? false
        let review = CodeReview(
            isDraft: isDraft,
            isApproved: !approvers.isEmpty && approvals.approved != false,
            changesRequested: mr.detailedMergeStatus == "requested_changes",
            hasConflicts: mr.hasConflicts ?? false,
            checks: checks(detail.headPipeline?.status),
            comments: mr.userNotesCount ?? 0
        )

        var participants = (mr.reviewers ?? []).map { reviewer in
            Person(
                name: reviewer.username,
                avatarURL: URL(lenient: reviewer.avatarUrl, relativeTo: host),
                tone: approvers.contains(reviewer.username) ? ReviewerTone.approved : ReviewerTone.waiting
            )
        }
        for approver in approvals.approvedBy ?? [] where !participants.contains(where: { $0.name == approver.user.username }) {
            participants.append(Person(
                name: approver.user.username,
                avatarURL: URL(lenient: approver.user.avatarUrl, relativeTo: host),
                tone: ReviewerTone.approved
            ))
        }

        let project = mr.references?.full.components(separatedBy: "!").first ?? "project \(mr.projectId)"
        return InboxItem(
            id: "\(accountID.uuidString)/\(mr.id)",
            accountID: accountID,
            pluginID: manifest.id,
            bundle: authored ? .authored : .reviews,
            title: Readable.text(mr.title),
            context: "\(project) !\(mr.iid)",
            url: URL(lenient: mr.webUrl),
            author: Person(name: mr.author.username, avatarURL: URL(lenient: mr.author.avatarUrl, relativeTo: host)),
            participants: participants,
            badges: review.badges(authored: authored),
            date: mr.updatedAt,
            needsAction: review.needsAction(authored: authored),
            changes: files.flatMap { files in
                files.isEmpty ? nil : ChangeSet(files: files.map {
                    ChangedFile(path: $0.newPath ?? $0.oldPath ?? "?", additions: $0.additions, deletions: $0.deletions)
                })
            }
        )
    }

    private static func checks(_ status: String?) -> CodeReview.Checks {
        switch status {
        case "success": .passing
        case "failed": .failing
        case "running", "pending", "created", "preparing", "waiting_for_resource": .running
        default: .none
        }
    }

    private func get<T: Decodable>(_ path: String, _ query: [String: String] = [:]) async throws -> T {
        let url = host.appending(path: "api/v4/\(path)", query: query)
        return try await http.decode(T.self, from: .get(url, headers: ["PRIVATE-TOKEN": token]), using: .api(snakeCase: true))
    }
}

// MARK: - Wire format

extension GitLabPlugin {
    struct User: Decodable {
        var username: String
        var avatarUrl: String?
    }

    struct MergeRequest: Decodable {
        struct References: Decodable { var full: String }

        var id: Int
        var iid: Int
        var projectId: Int
        var title: String
        var webUrl: String
        var draft: Bool?
        var workInProgress: Bool?
        var updatedAt: Date
        var author: User
        var reviewers: [User]?
        var userNotesCount: Int?
        var hasConflicts: Bool?
        var references: References?
        var detailedMergeStatus: String?
    }

    struct Approvals: Decodable {
        struct Approver: Decodable { var user: User }
        var approved: Bool?
        var approvedBy: [Approver]?
    }

    /// One file of `/diffs`: only the paths are decoded, never the `diff` text. GraphQL adds the line counts.
    struct DiffFile: Decodable {
        var newPath: String?
        var oldPath: String?
        var additions: Int?
        var deletions: Int?
    }

    struct Detail: Decodable {
        struct Pipeline: Decodable { var status: String }
        var headPipeline: Pipeline?
    }

    /// The GraphQL answer, turned into the REST shapes above so both ways build the same items.
    enum GraphQL {
        struct Response: Decodable {
            struct Body: Decodable { var currentUser: CurrentUser? }
            struct CurrentUser: Decodable { var authored: Connection?; var reviewing: Connection? }
            struct Connection: Decodable { var nodes: [Node?]? }
            var data: Body?

            var details: [Int: Details] {
                let nodes = [data?.currentUser?.authored, data?.currentUser?.reviewing].compactMap { $0?.nodes }.flatMap { $0 }.compactMap { $0 }
                var found: [Int: Details] = [:]
                for node in nodes {
                    guard let id = node.restID else { continue }
                    // The same merge request in both lists: keep the one with files.
                    if found[id]?.files == nil || node.diffStats != nil { found[id] = node.details }
                }
                return found
            }
        }

        struct Node: Decodable {
            struct Person: Decodable { var username: String; var avatarUrl: String? }
            struct People: Decodable { var nodes: [Person?]? }
            struct Pipeline: Decodable { var status: String }
            struct Stat: Decodable { var path: String; var additions: Int?; var deletions: Int? }

            var id: String
            var approved: Bool?
            var approvedBy: People?
            var headPipeline: Pipeline?
            var diffStats: [Stat]?

            /// "gid://gitlab/MergeRequest/101" → 101, the id REST uses.
            var restID: Int? { id.split(separator: "/").last.flatMap { Int($0) } }

            var details: Details {
                Details(
                    approvals: Approvals(
                        approved: approved,
                        approvedBy: (approvedBy?.nodes ?? []).compactMap { $0 }.map { Approvals.Approver(user: User(username: $0.username, avatarUrl: $0.avatarUrl)) }
                    ),
                    detail: Detail(headPipeline: headPipeline.map { Detail.Pipeline(status: $0.status.lowercased()) }),
                    files: diffStats.map { $0.map { DiffFile(newPath: $0.path, additions: $0.additions, deletions: $0.deletions) } }
                )
            }
        }

        struct Details {
            var approvals: Approvals
            var detail: Detail
            var files: [DiffFile]?
        }
    }
}

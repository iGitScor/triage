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
        let items = try await all.concurrentMap { mr, isAuthored in
            try await self.item(for: mr, authored: isAuthored)
        }
        return SourceSnapshot(identity: user.username, items: items)
    }

    private func item(for mr: MergeRequest, authored: Bool) async throws -> InboxItem {
        let base = "projects/\(mr.projectId)/merge_requests/\(mr.iid)"
        async let approvals: Approvals = get("\(base)/approvals")
        async let detail: Detail = get(base)
        // Review prep: file paths only. The diff text in this response is never decoded.
        let files: [DiffFile]? = authored ? nil : try? await get("\(base)/diffs", ["per_page": "50"])
        return try await Self.item(
            mr: mr, approvals: approvals, detail: detail, files: files,
            authored: authored, accountID: accountID, host: host
        )
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
            title: mr.title,
            context: "\(project) !\(mr.iid)",
            url: URL(lenient: mr.webUrl),
            author: Person(name: mr.author.username, avatarURL: URL(lenient: mr.author.avatarUrl, relativeTo: host)),
            participants: participants,
            badges: review.badges(authored: authored),
            date: mr.updatedAt,
            needsAction: review.needsAction(authored: authored),
            changes: files.flatMap { files in
                files.isEmpty ? nil : ChangeSet(files: files.map { ChangedFile(path: $0.newPath ?? $0.oldPath ?? "?") })
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

    /// One file of `/diffs`: only the paths are decoded, never the `diff` text.
    struct DiffFile: Decodable {
        var newPath: String?
        var oldPath: String?
    }

    struct Detail: Decodable {
        struct Pipeline: Decodable { var status: String }
        var headPipeline: Pipeline?
    }
}

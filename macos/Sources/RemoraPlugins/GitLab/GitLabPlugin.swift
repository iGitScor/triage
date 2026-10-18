import Foundation
import RemoraCore

/// Merge requests you opened and those where you are a reviewer, on gitlab.com or self-hosted; for the projects you
/// maintain, a failed pipeline on the default branch.
public struct GitLabPlugin: SourcePlugin {
    public static let manifest = PluginManifest(
        id: "gitlab",
        name: "GitLab",
        symbol: "arrow.triangle.merge",
        summary: "Merge requests you opened and those you review, and your To-Dos. Works with self-hosted GitLab.",
        fields: [
            .host("https://gitlab.com"),
            .token("Personal access token", help: "Token with the read_api scope."),
            ConfigField(
                key: "repositories", label: "Projects I maintain", placeholder: "acme/web, acme/tools/cli",
                isOptional: true,
                help: "Optional. When the last pipeline of a project's default branch failed, it goes to To fix."),
        ],
        setupSteps: [
            "If you use self-hosted GitLab, set the host first.",
            "Click “Create a token”: GitLab opens with the read_api scope selected.",
            "Click “Create personal access token” and paste it below.",
        ],
        setupURL: { config in
            let host = (try? config.url("host")) ?? URL(string: "https://gitlab.com")!
            return host.appending(
                path: "-/user_settings/personal_access_tokens",
                query: [
                    "name": "Remora", "scopes": "read_api",
                ])
        },
        egress: Egress(
            hosts: ["gitlab.com"],
            description:
                "Reads your merge requests, approvals, pipelines and To-Dos from your GitLab host, and the default branch's pipelines of the projects you maintain."
        ),
        logo: "gitlab"
    )

    private let accountID: UUID
    private let host: URL
    private let token: String
    private let projects: [String]
    private let http: HTTPClient
    private let cache: any AccountCache

    public init(config: PluginConfig, http: HTTPClient) throws {
        accountID = config.accountID
        host = try config.url("host")
        token = try config.required("token")
        projects = Maintained.repositories(config.values["repositories"], nested: true)
        self.http = http
        cache = config.cache
    }

    public func fetch() async throws -> SourceSnapshot {
        let user: User = try await cache.value("user", lifetime: CacheLifetime.identity) { try await get("user") }
        async let authored: [MergeRequest] = get(
            "merge_requests",
            [
                "scope": "created_by_me", "state": "opened", "per_page": Self.perPage,
            ])
        async let reviewing: [MergeRequest] = get(
            "merge_requests",
            [
                "scope": "all", "state": "opened", "per_page": Self.perPage, "reviewer_username": user.username,
            ])
        async let todos = BestEffort.attempt {
            try await get("todos", ["state": "pending", "per_page": Self.perPage]) as [Todo]
        }
        // A merge request of yours where you are also a reviewer comes once, as yours: the same id twice would share
        // its Done, snooze and pin between two rows.
        let mine = try await authored
        let mineIDs = Set(mine.map(\.id))
        let all = try await mine.map { ($0, true) } + reviewing.filter { !mineIDs.contains($0.id) }.map { ($0, false) }
        // Approvals, pipeline and changed files for every merge request in one GraphQL request, instead of two or
        // three REST calls each. An instance where it fails (older GitLab, GraphQL off) gets the REST calls.
        var details: [Int: GraphQL.Details] = [:]
        if !all.isEmpty {
            do {
                details = try await graphQLDetails()
            } catch HTTPError.rateLimited(let until) {
                throw HTTPError.rateLimited(until)
            } catch {}  // An older GitLab, or GraphQL off: the REST calls below.
        }
        let found = details
        let results = try await all.concurrentMap(limit: 4) { mr, isAuthored -> (InboxItem, Error?) in
            if let found = found[mr.id] {
                let item = Self.item(
                    mr: mr, approvals: found.approvals, detail: found.detail, files: isAuthored ? nil : found.files,
                    authored: isAuthored, accountID: self.accountID, host: self.host
                )
                return (item, nil)
            }
            return await self.item(for: mr, authored: isAuthored)
        }
        // A full page means there may be more: GitLab's count header isn't always sent.
        let truncated = try await [authored, reviewing].contains { $0.count >= Self.pageSize }
        var remarks = truncated ? [SourceSnapshot.truncated("GitLab")] : []
        // Without approvals, an approved merge request would wait as if it weren't: say so.
        if let failed = results.lazy.compactMap(\.1).first {
            try BestEffort.note("Approvals, pipelines or changed files", failed, remarks: &remarks)
        }
        // To-Dos are a bonus: their failure is a remark, and merge requests still come through.
        let toDo = try BestEffort.unwrap(await todos, "To-Dos", remarks: &remarks) ?? []
        var failing: [InboxItem] = []
        for project in projects {
            let item = try await BestEffort.value("Projects you maintain", remarks: &remarks) {
                try await failed(project)
            }
            if let item = item ?? nil { failing.append(item) }
        }
        return SourceSnapshot(
            identity: user.username,
            items: results.map(\.0) + toDo.compactMap { $0.item(accountID: accountID) } + failing,
            remarks: remarks)
    }

    /// A project you maintain whose default branch's last finished pipeline failed: to fix. Same as Windows.
    private func failed(_ project: String) async throws -> InboxItem? {
        // The project by its path, encoded as one segment; then by its id.
        var components = URLComponents(url: host.appending(path: "api/v4/projects"), resolvingAgainstBaseURL: false)!
        components.percentEncodedPath += "/" + project.replacingOccurrences(of: "/", with: "%2F")
        let info = try await http.decode(
            Project.self, from: .get(components.url!, headers: ["PRIVATE-TOKEN": token]), using: .api(snakeCase: true))
        guard let branch = info.defaultBranch else { return nil }
        let pipelines: [BranchPipeline] = try await get(
            "projects/\(info.id)/pipelines", ["ref": branch, "scope": "finished", "per_page": "1"])
        guard let pipeline = pipelines.first, pipeline.status == "failed" else { return nil }
        // The commit's title says what broke it; without it, the pipeline still shows.
        let commit: Commit? = try? await get("projects/\(info.id)/repository/commits/\(pipeline.sha)")
        return Maintained.failing(
            accountID: accountID, pluginID: Self.manifest.id, local: "ci/\(project)/\(pipeline.id)",
            title: commit.map { Readable.text($0.title) } ?? "Pipeline #\(pipeline.id)",
            context: "\(project) · \(branch)", preview: nil, url: URL(lenient: pipeline.webUrl),
            date: pipeline.updatedAt)
    }

    /// The REST way, one merge request at a time. Best-effort: a call that fails leaves that part out instead of
    /// failing the whole account, and is returned so the fetch can say so.
    private func item(for mr: MergeRequest, authored: Bool) async -> (InboxItem, Error?) {
        let base = "projects/\(mr.projectId)/merge_requests/\(mr.iid)"
        async let approvals = BestEffort.attempt { try await get("\(base)/approvals") as Approvals }
        async let detail = BestEffort.attempt { try await get(base) as Detail }
        // Review prep: file paths only. The diff text in this response is never decoded.
        let files: Result<[DiffFile], Error>? =
            authored ? nil : await BestEffort.attempt { try await get("\(base)/diffs", ["per_page": Self.perPage]) }
        let (approved, detailed) = await (approvals, detail)
        let failed = [approved.error, detailed.error, files?.error].compactMap { $0 }.first
        let item = Self.item(
            mr: mr, approvals: (try? approved.get()) ?? Approvals(), detail: (try? detailed.get()) ?? Detail(),
            files: try? files?.get(), authored: authored, accountID: accountID, host: host
        )
        return (item, failed)
    }

    /// One page, asked for by every list request: a full page of merge requests means the list was cut.
    static let pageSize = 50
    static let perPage = String(pageSize)

    static let detailsQuery = """
        query { currentUser {
          authored: authoredMergeRequests(state: opened, first: \(pageSize)) { nodes { ...status } }
          reviewing: reviewRequestedMergeRequests(state: opened, first: \(pageSize)) { nodes { ...status diffStats { path additions deletions } } }
        } }
        fragment status on MergeRequest { id approved approvedBy { nodes { username avatarUrl } } headPipeline { status } }
        """

    /// The details of every open merge request you wrote or review, keyed by its REST id.
    private func graphQLDetails() async throws -> [Int: GraphQL.Details] {
        let request = try URLRequest.post(
            host.appending(path: "api/graphql"), json: ["query": Self.detailsQuery], headers: ["PRIVATE-TOKEN": token])
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
                review: approvers.contains(reviewer.username) ? ReviewState.approved : nil
            )
        }
        for approver in approvals.approvedBy ?? []
        where !participants.contains(where: { $0.name == approver.user.username }) {
            participants.append(
                Person(
                    name: approver.user.username,
                    avatarURL: URL(lenient: approver.user.avatarUrl, relativeTo: host),
                    review: ReviewState.approved
                ))
        }

        let project = mr.references?.full.components(separatedBy: "!").first ?? "project \(mr.projectId)"
        return InboxItem(
            id: InboxItem.makeID(account: accountID, local: "\(mr.id)"),
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
                files.isEmpty
                    ? nil
                    : ChangeSet(
                        files: files.map {
                            ChangedFile(
                                path: $0.newPath ?? $0.oldPath ?? "?", additions: $0.additions, deletions: $0.deletions)
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
        return try await http.decode(
            T.self, from: .get(url, headers: ["PRIVATE-TOKEN": token]), using: .api(snakeCase: true))
    }
}

// MARK: - Wire format

extension GitLabPlugin {
    struct User: Decodable {
        var username: String
        var avatarUrl: String?
    }

    /// A GitLab To-Do: an issue assigned to you, a mention, something you marked. Merge requests are left out,
    /// already shown with their approvals and pipelines. Same as Windows (`gitlab.rs`).
    struct Todo: Decodable, Sendable {
        struct Target: Decodable, Sendable {
            var title: String?
            var iid: Int?
        }
        struct Author: Decodable, Sendable {
            var name: String?
            var username: String?
            var avatarUrl: String?
        }
        struct Project: Decodable, Sendable { var pathWithNamespace: String? }

        var id: Int
        var actionName: String
        var targetType: String
        var target: Target?
        var targetUrl: String?
        var body: String?
        var author: Author?
        var project: Project?
        var updatedAt: Date?
        var createdAt: Date?

        func item(accountID: UUID) -> InboxItem? {
            guard targetType != "MergeRequest" else { return nil }
            let mention = actionName == "mentioned" || actionName == "directly_addressed"
            let title = Readable.text(target?.title ?? body ?? "")
            let project = project?.pathWithNamespace ?? ""
            let context = targetType == "Issue" && target?.iid != nil ? "\(project) #\(target!.iid!)" : project
            let preview = mention ? body.map(Readable.text).flatMap { $0.isEmpty || $0 == title ? nil : $0 } : nil
            return InboxItem(
                id: InboxItem.makeID(account: accountID, local: "todo/\(id)"),
                accountID: accountID,
                pluginID: GitLabPlugin.manifest.id,
                bundle: mention ? .mentions : .tasks,
                title: title,
                context: context,
                preview: preview,
                url: URL(lenient: targetUrl),
                author: author.flatMap { author in
                    (author.username ?? author.name).map { Person(name: $0, avatarURL: URL(lenient: author.avatarUrl)) }
                },
                date: updatedAt ?? createdAt ?? .now,
                needsAction: true
            )
        }
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
            struct CurrentUser: Decodable {
                var authored: Connection?
                var reviewing: Connection?
            }
            struct Connection: Decodable { var nodes: [Node?]? }
            var data: Body?

            var details: [Int: Details] {
                let nodes = [data?.currentUser?.authored, data?.currentUser?.reviewing].compactMap { $0?.nodes }.flatMap
                { $0 }.compactMap { $0 }
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
            struct Person: Decodable {
                var username: String
                var avatarUrl: String?
            }
            struct People: Decodable { var nodes: [Person?]? }
            struct Pipeline: Decodable { var status: String }
            struct Stat: Decodable {
                var path: String
                var additions: Int?
                var deletions: Int?
            }

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
                        approvedBy: (approvedBy?.nodes ?? []).compactMap { $0 }.map {
                            Approvals.Approver(user: User(username: $0.username, avatarUrl: $0.avatarUrl))
                        }
                    ),
                    detail: Detail(headPipeline: headPipeline.map { Detail.Pipeline(status: $0.status.lowercased()) }),
                    files: diffStats.map {
                        $0.map { DiffFile(newPath: $0.path, additions: $0.additions, deletions: $0.deletions) }
                    }
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

// MARK: - Projects you maintain

struct Project: Decodable {
    var id: Int
    var defaultBranch: String?
}

struct BranchPipeline: Decodable {
    var id: Int
    var status: String
    var sha: String
    var webUrl: String
    var updatedAt: Date
}

struct Commit: Decodable { var title: String }

import Foundation
import RemoraCore

/// Pull requests you review or opened, and work items assigned to you, on Azure DevOps or Azure DevOps Server.
/// Same queries and mapping on Windows (`azuredevops.rs`).
public struct AzureDevOpsPlugin: SourcePlugin {
    public static let manifest = PluginManifest(
        id: "azuredevops",
        name: "Azure DevOps",
        symbol: "arrow.triangle.branch",
        summary:
            "Pull requests you review or opened, with votes and policy checks, and work items assigned to you. Works with Azure DevOps Server.",
        fields: [
            ConfigField(
                key: "host", label: "Organization URL", placeholder: "https://dev.azure.com/your-organization",
                help:
                    "Azure DevOps Server: your collection, for example https://tfs.acme.example/tfs/DefaultCollection."
            ),
            .token("Personal access token", help: "Scopes: Code (Read) and Work Items (Read)."),
        ],
        setupSteps: [
            "Set your organization URL first: https://dev.azure.com/your-organization, or your server’s collection.",
            "Click “Create a token”, name it Remora, and select Code (Read) and Work Items (Read).",
            "Click Create and paste the token below.",
        ],
        setupURL: { config in (try? config.url("host"))?.appending(path: "_usersSettings/tokens") },
        egress: Egress(
            hosts: ["dev.azure.com"],
            description:
                "Reads your pull requests, votes, policy checks, changed file paths and assigned work items from your Azure DevOps organization or server."
        )
    )

    /// Each list stops at one page: a full one means the tool has more.
    static let pageSize = 50
    static let top = String(pageSize)
    /// Azure DevOps Server 2020 and later, and the cloud.
    static let apiVersion = "6.0"
    /// Open work items assigned to you, whatever the process (Basic, Agile, Scrum, CMMI).
    static let assignedWIQL = """
        SELECT [System.Id] FROM WorkItems WHERE [System.AssignedTo] = @Me \
        AND [System.State] NOT IN ('Done', 'Closed', 'Completed', 'Resolved', 'Removed') \
        ORDER BY [System.ChangedDate] DESC
        """
    /// The policies that are checks: Build and Status. The others (reviewers, linked work items, comments) are
    /// already the votes, or rules.
    static let checkPolicies: Set<String> = [
        "0609b952-1397-4640-95ec-e00a01b2c241", "cbdc66da-9728-4af8-aada-9a5a32e4a226",
    ]

    private let accountID: UUID
    private let host: URL
    private let headers: [String: String]
    private let http: HTTPClient
    private let cache: any AccountCache

    public init(config: PluginConfig, http: HTTPClient) throws {
        accountID = config.accountID
        host = try config.url("host")
        let token = try config.required("token")
        headers = [
            // A personal access token goes as the password of Basic, with no user name.
            "Authorization": "Basic " + Data(":\(token)".utf8).base64EncodedString(),
            "Accept": "application/json",
            // A refused token answers 401, not a sign-in page.
            "X-TFS-FedAuthRedirect": "Suppress",
        ]
        self.http = http
        cache = config.cache
    }

    public func fetch() async throws -> SourceSnapshot {
        let me: ConnectionData.User = try await cache.value("user", lifetime: CacheLifetime.identity) {
            try await (get("_apis/connectionData", version: nil) as ConnectionData).authenticatedUser
        }
        async let authored: List<PullRequest> = get(
            "_apis/git/pullrequests",
            ["searchCriteria.creatorId": me.id, "searchCriteria.status": "active", "$top": Self.top])
        async let reviewing: List<PullRequest> = get(
            "_apis/git/pullrequests",
            ["searchCriteria.reviewerId": me.id, "searchCriteria.status": "active", "$top": Self.top])
        async let work = BestEffort.attempt { try await workItems() }
        let lists = try await [authored, reviewing]
        let pulls = Self.pullRequests(lists.flatMap(\.value), me: me.id)
        let results = try await pulls.concurrentMap(limit: 4) { pr in await self.item(for: pr, me: me.id) }
        var remarks: [String] = []
        // Without the checks, a failed build would look fine: say so.
        if let failed = results.lazy.compactMap(\.1).first {
            try BestEffort.note("Policy checks or changed files", failed, remarks: &remarks)
        }
        // Work items are a bonus: their failure is a remark, and pull requests still come through.
        let (assigned, cut) = try BestEffort.unwrap(await work, "Work items", remarks: &remarks) ?? ([], false)
        if cut || lists.contains(where: { $0.value.count >= Self.pageSize }) {
            remarks.insert(SourceSnapshot.truncated("Azure DevOps"), at: 0)
        }
        let tasks = assigned.map { $0.item(host: host, accountID: accountID) }
        return SourceSnapshot(identity: me.providerDisplayName, items: results.map(\.0) + tasks, remarks: remarks)
    }

    /// Each pull request once, yours and those still waiting on your vote: one you voted on or declined is no
    /// longer your turn, as GitHub drops a review request once you review.
    static func pullRequests(_ all: [PullRequest], me: String) -> [PullRequest] {
        var seen = Set<Int>()
        return all.filter { pr in
            guard seen.insert(pr.pullRequestId).inserted else { return false }
            if pr.createdBy?.id == me { return true }
            guard let mine = pr.reviewers?.first(where: { $0.id == me }) else { return false }
            return (mine.vote ?? 0) == 0 && mine.hasDeclined != true
        }
    }

    /// Policy checks for every pull request, and changed files for those you review. Best-effort: a call that fails
    /// leaves that part out and is returned, so the fetch can say so.
    private func item(for pr: PullRequest, me: String) async -> (InboxItem, Error?) {
        let authored = pr.createdBy?.id == me
        async let evaluations = BestEffort.attempt { try await self.evaluations(of: pr) }
        let iteration: Result<Iteration?, Error>? =
            authored ? nil : await BestEffort.attempt { try await self.lastIteration(of: pr) }
        var last: Iteration?
        if case .success(let found)? = iteration { last = found }
        var changes: Result<[Change], Error>?
        if let last {
            changes = await BestEffort.attempt { try await self.changes(of: pr, iteration: last.id) }
        }
        let checks = await evaluations
        let failed = [checks.error, iteration?.error, changes?.error].compactMap { $0 }.first
        let item = Self.item(
            pr: pr, evaluations: (try? checks.get()) ?? [], iteration: last, changes: try? changes?.get(),
            authored: authored, accountID: accountID, host: host)
        return (item, failed)
    }

    private func evaluations(of pr: PullRequest) async throws -> [Evaluation] {
        let project = pr.repository.project.id
        let artifact = "vstfs:///CodeReview/CodeReviewId/\(project)/\(pr.codeReviewId ?? pr.pullRequestId)"
        let list: List<Evaluation> = try await get(
            "\(project)/_apis/policy/evaluations", ["artifactId": artifact], version: "\(Self.apiVersion)-preview.1")
        return list.value
    }

    private func lastIteration(of pr: PullRequest) async throws -> Iteration? {
        let list: List<Iteration> = try await get(
            "_apis/git/repositories/\(pr.repository.id)/pullRequests/\(pr.pullRequestId)/iterations")
        return list.value.max { $0.id < $1.id }
    }

    /// Review prep: file paths only, from the latest push against the target branch. No content is read.
    private func changes(of pr: PullRequest, iteration: Int) async throws -> [Change] {
        let page: Changes = try await get(
            "_apis/git/repositories/\(pr.repository.id)/pullRequests/\(pr.pullRequestId)/iterations/\(iteration)/changes",
            ["$top": "100"])
        return page.changeEntries ?? []
    }

    /// The query for ids, then their fields in one request. Returns whether the query stopped at its limit.
    private func workItems() async throws -> ([WorkItem], Bool) {
        let url = host.appending(path: "_apis/wit/wiql", query: ["$top": Self.top, "api-version": Self.apiVersion])
        let request = try URLRequest.post(url, json: ["query": Self.assignedWIQL], headers: headers)
        let found = try await http.decode(WIQLResult.self, from: request).workItems ?? []
        guard !found.isEmpty else { return ([], false) }
        let ids = found.map { String($0.id) }.joined(separator: ",")
        let list: List<WorkItem> = try await get("_apis/wit/workitems", ["ids": ids, "errorPolicy": "omit"])
        // The order of the query: most recently changed first.
        let byID = Dictionary(list.value.map { ($0.id, $0) }, uniquingKeysWith: { first, _ in first })
        return (found.compactMap { byID[$0.id] }, found.count >= Self.pageSize)
    }

    static func item(
        pr: PullRequest,
        evaluations: [Evaluation],
        iteration: Iteration?,
        changes: [Change]?,
        authored: Bool,
        accountID: UUID,
        host: URL
    ) -> InboxItem {
        let votes = (pr.reviewers ?? []).map { $0.vote ?? 0 }
        let changesRequested = votes.contains { $0 < 0 }
        let review = CodeReview(
            isDraft: pr.isDraft ?? false,
            isApproved: !changesRequested && votes.contains { $0 >= 5 },
            changesRequested: changesRequested,
            hasConflicts: pr.mergeStatus == "conflicts",
            checks: checks(evaluations)
        )
        let participants = (pr.reviewers ?? []).map { reviewer in
            let vote = reviewer.vote ?? 0
            return Person(
                name: reviewer.displayName,
                review: vote >= 5 ? ReviewState.approved : vote < 0 ? ReviewState.changesRequested : nil)
        }
        let repository = pr.repository
        let files = (changes ?? []).compactMap { change -> ChangedFile? in
            guard change.item?.isFolder != true, let path = change.item?.path else { return nil }
            return ChangedFile(path: String(path.drop { $0 == "/" }))
        }
        let pushed = iteration.flatMap { $0.updatedDate ?? $0.createdDate }
        return InboxItem(
            id: InboxItem.makeID(account: accountID, local: "pr/\(pr.pullRequestId)"),
            accountID: accountID,
            pluginID: manifest.id,
            bundle: authored ? .authored : .reviews,
            title: Readable.text(pr.title),
            context: "\(repository.fullName) !\(pr.pullRequestId)",
            url: host.appending(
                components: repository.project.name, "_git", repository.name, "pullrequest", String(pr.pullRequestId)),
            author: pr.createdBy.map { Person(name: $0.displayName) },
            participants: participants,
            badges: review.badges(authored: authored),
            // The list has no last-updated date: the latest push stands for it when known.
            date: max(pr.creationDate, pushed ?? pr.creationDate),
            needsAction: review.needsAction(authored: authored),
            changes: files.isEmpty ? nil : ChangeSet(files: files)
        )
    }

    /// Build and status policies that apply: one rejected fails, one queued or running runs, else they pass.
    static func checks(_ evaluations: [Evaluation]) -> CodeReview.Checks {
        let states = evaluations.filter {
            checkPolicies.contains($0.configuration?.type?.id ?? "") && $0.configuration?.isEnabled != false
                && $0.status != "notApplicable"
        }.map(\.status)
        if states.isEmpty { return .none }
        if states.contains(where: { $0 == "rejected" || $0 == "broken" }) { return .failing }
        if states.contains(where: { $0 == "queued" || $0 == "running" }) { return .running }
        return .passing
    }

    private func get<T: Decodable>(
        _ path: String, _ query: [String: String] = [:], version: String? = apiVersion
    ) async throws -> T {
        var query = query
        if let version { query["api-version"] = version }
        let url = query.isEmpty ? host.appending(path: path) : host.appending(path: path, query: query)
        return try await http.decode(T.self, from: .get(url, headers: headers))
    }
}

// MARK: - Wire format

extension AzureDevOpsPlugin {
    struct List<Element: Decodable & Sendable>: Decodable, Sendable { var value: [Element] }

    struct ConnectionData: Decodable, Sendable {
        struct User: Decodable, Sendable {
            var id: String
            var providerDisplayName: String
        }
        var authenticatedUser: User
    }

    struct Identity: Decodable, Sendable {
        var id: String?
        var displayName: String
    }

    struct PullRequest: Decodable, Sendable {
        struct Repository: Decodable, Sendable {
            struct Project: Decodable, Sendable {
                var id: String
                var name: String
            }
            var id: String
            var name: String
            var project: Project

            /// "Acme/app", or "app" when the repository is named after its project.
            var fullName: String { name == project.name ? name : "\(project.name)/\(name)" }
        }

        struct Reviewer: Decodable, Sendable {
            var id: String?
            var displayName: String
            /// 10 approved, 5 approved with suggestions, 0 no vote, -5 waiting for the author, -10 rejected.
            var vote: Int?
            var hasDeclined: Bool?
        }

        var pullRequestId: Int
        var codeReviewId: Int?
        var title: String
        var isDraft: Bool?
        var creationDate: Date
        var mergeStatus: String?
        var createdBy: Identity?
        var repository: Repository
        var reviewers: [Reviewer]?
    }

    struct Evaluation: Decodable, Sendable {
        struct Configuration: Decodable, Sendable {
            struct Kind: Decodable, Sendable { var id: String }
            var isEnabled: Bool?
            var type: Kind?
        }
        /// approved, rejected, running, queued, notApplicable, broken.
        var status: String
        var configuration: Configuration?
    }

    struct Iteration: Decodable, Sendable {
        var id: Int
        var createdDate: Date?
        var updatedDate: Date?
    }

    struct Changes: Decodable, Sendable { var changeEntries: [Change]? }

    struct Change: Decodable, Sendable {
        struct Item: Decodable, Sendable {
            var path: String?
            var isFolder: Bool?
        }
        var item: Item?
    }

    struct WIQLResult: Decodable, Sendable {
        struct Reference: Decodable, Sendable { var id: Int }
        var workItems: [Reference]?
    }

    struct WorkItem: Decodable, Sendable {
        struct Fields: Decodable, Sendable {
            var title: String?
            var state: String?
            var type: String?
            var project: String?
            var changed: Date?
            var priority: Int?
            var due: String?

            enum CodingKeys: String, CodingKey {
                case title = "System.Title"
                case state = "System.State"
                case type = "System.WorkItemType"
                case project = "System.TeamProject"
                case changed = "System.ChangedDate"
                case priority = "Microsoft.VSTS.Common.Priority"
                case due = "Microsoft.VSTS.Scheduling.DueDate"
            }
        }

        var id: Int
        var fields: Fields

        /// Priority 1 is high, 2 normal (the default), 3 and 4 low.
        var itemPriority: Priority {
            switch fields.priority {
            case 1: .high
            case 3, 4: .low
            default: .normal
            }
        }

        func item(host: URL, accountID: UUID) -> InboxItem {
            let priority = itemPriority
            var badges: [Badge] = []
            if priority == .high {
                badges.append(Badge(id: .priorityHigh, label: L("High"), symbol: "exclamationmark.2", tone: .warning))
            }
            let due = DueDate.parse(fields.due)
            if let due { badges.append(DueDate.badge(for: due)) }
            if let state = fields.state { badges.append(Badge(id: .status, label: state, tone: .neutral)) }
            let kind = [fields.type, String(id)].compactMap { $0 }.joined(separator: " ")
            return InboxItem(
                id: InboxItem.makeID(account: accountID, local: "workitem/\(id)"),
                accountID: accountID,
                pluginID: AzureDevOpsPlugin.manifest.id,
                bundle: .tasks,
                title: Readable.text(fields.title ?? kind),
                context: [fields.project, kind].compactMap { $0 }.joined(separator: " · "),
                url: fields.project.map { host.appending(components: $0, "_workitems", "edit", String(id)) },
                badges: badges,
                date: fields.changed ?? .now,
                needsAction: priority != .low,
                priority: priority,
                due: due
            )
        }
    }
}

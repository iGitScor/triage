import Foundation
import RemoraCore

/// Pull requests you opened and those you review, on Bitbucket Cloud or Data Center. Same as Windows
/// (`bitbucket.rs`).
public struct BitbucketPlugin: SourcePlugin {
    public static let manifest = PluginManifest(
        id: "bitbucket",
        name: "Bitbucket",
        symbol: "arrow.triangle.branch",
        summary: "Pull requests you opened and those you review. Bitbucket Cloud or Data Center.",
        fields: [
            .host("https://bitbucket.org"),
            ConfigField(
                key: "workspace", label: "Workspace", placeholder: "your-workspace", isOptional: true,
                help: "Bitbucket Cloud only: the workspace in your links, bitbucket.org/your-workspace."),
            ConfigField(
                key: "email", label: "E-mail", placeholder: "you@company.com", isOptional: true,
                help: "Bitbucket Cloud only: your Atlassian account’s e-mail."),
            .token("API token", help: "Bitbucket Cloud: an API token. Data Center: an HTTP access token."),
        ],
        setupSteps: [
            "Bitbucket Cloud: click “Create a token”, choose “Create API token with scopes”, then Bitbucket, with read:user, read:repository and read:pullrequest.",
            "Enter your workspace and the e-mail of your Atlassian account too.",
            "Data Center: set your Bitbucket host, then in Bitbucket open your profile → Manage account → HTTP access tokens, create a read token, and leave the workspace and e-mail empty.",
            "Paste the token below.",
        ],
        setupURL: { _ in URL(string: "https://id.atlassian.com/manage-profile/security/api-tokens") },
        egress: Egress(
            hosts: ["bitbucket.org"],
            description:
                "Reads your pull requests, review requests, approvals and build status from Bitbucket Cloud or your Bitbucket host."
        ),
        logo: "bitbucket"
    )

    /// Bitbucket Cloud has no "review requests" list: the review requests are looked for in the workspace's most
    /// recently updated repositories, this many.
    static let repositories = 25

    private let accountID: UUID
    private let host: URL
    private let workspace: String
    private let authorization: String
    private let isCloud: Bool
    private let http: HTTPClient
    private let cache: any AccountCache

    public init(config: PluginConfig, http: HTTPClient) throws {
        accountID = config.accountID
        host = try config.url("host")
        let token = try config.required("token")
        isCloud = Self.isCloud(host)
        // Cloud signs in with the Atlassian e-mail and an API token, and reads one workspace; Data Center with an
        // HTTP access token.
        if isCloud {
            workspace = try config.required("workspace")
            let email = try config.required("email")
            authorization = "Basic " + Data("\(email):\(token)".utf8).base64EncodedString()
        } else {
            workspace = ""
            authorization = "Bearer \(token)"
        }
        self.http = http
        cache = config.cache
    }

    static func isCloud(_ host: URL) -> Bool {
        ["bitbucket.org", "www.bitbucket.org", "api.bitbucket.org"].contains(host.host?.lowercased() ?? "")
    }

    public func fetch() async throws -> SourceSnapshot {
        isCloud ? try await fetchCloud() : try await fetchDataCenter()
    }

    // MARK: Cloud

    private func fetchCloud() async throws -> SourceSnapshot {
        let me: Cloud.User = try await cache.value("user", lifetime: CacheLifetime.identity) { try await cloud("user") }
        let fields = "+values.participants,+values.reviewers"
        async let authored: Cloud.Page<Cloud.PullRequest> = cloud(
            "workspaces/\(workspace)/pullrequests/\(me.accountId ?? me.uuid)",
            ["state": "OPEN", "pagelen": "50", "fields": fields])
        // Review requests are a bonus: without them (a token without read:repository), your own pull requests still
        // come through, and a remark says so.
        var remarks: [String] = []
        let reviewing =
            try await BestEffort.value("Review requests", remarks: &remarks) { try await cloudReviewRequests(me) }
            ?? []
        let mine = try await authored
        if mine.next != nil { remarks.append(SourceSnapshot.truncated("Bitbucket")) }
        let all = mine.values.map { ($0, true) } + reviewing.map { ($0, false) }
        let results = try await all.concurrentMap(limit: 4) { pr, isAuthored in
            let checks = await BestEffort.attempt { try await self.cloudChecks(pr) }
            return (pr, isAuthored, checks)
        }
        if let failed = results.lazy.compactMap({ $0.2.error }).first {
            try BestEffort.note("Build status", failed, remarks: &remarks)
        }
        let items = results.map { pr, isAuthored, checks in
            pr.item(
                me: me, authored: isAuthored, checks: (try? checks.get()) ?? .none, accountID: accountID)
        }
        return SourceSnapshot(identity: me.displayName ?? me.nickname ?? "", items: items, remarks: remarks)
    }

    /// The open pull requests of the workspace's recently updated repositories where you are a reviewer, without
    /// those you already approved or sent back.
    private func cloudReviewRequests(_ me: Cloud.User) async throws -> [Cloud.PullRequest] {
        let repositories: Cloud.Page<Cloud.Repository> = try await cloud(
            "repositories/\(workspace)",
            [
                "role": "member", "sort": "-updated_on", "pagelen": String(Self.repositories),
                "fields": "values.full_name",
            ])
        let query = "state=\"OPEN\" AND reviewers.uuid=\"\(me.uuid)\""
        let pages = try await repositories.values.concurrentMap(limit: 4) { repository in
            try await self.cloud(
                "repositories/\(repository.fullName)/pullrequests",
                ["q": query, "pagelen": "50", "fields": "+values.participants,+values.reviewers"])
                as Cloud.Page<Cloud.PullRequest>
        }
        return pages.flatMap(\.values).filter { !$0.isReviewed(by: me) }
    }

    private func cloudChecks(_ pr: Cloud.PullRequest) async throws -> CodeReview.Checks {
        guard let repository = pr.destination?.repository?.fullName else { return .none }
        let statuses: Cloud.Page<Cloud.Status> = try await cloud(
            "repositories/\(repository)/pullrequests/\(pr.id)/statuses", ["pagelen": "25"])
        return Self.checks(statuses.values.map(\.state))
    }

    /// Commit statuses, Cloud's `SUCCESSFUL`, `FAILED`, `INPROGRESS`, `STOPPED`: one failure fails them all.
    static func checks(_ states: [String]) -> CodeReview.Checks {
        if states.contains("FAILED") { return .failing }
        if states.contains("INPROGRESS") { return .running }
        return states.contains("SUCCESSFUL") ? .passing : .none
    }

    private func cloud<T: Decodable>(_ path: String, _ query: [String: String] = [:]) async throws -> T {
        let url = URL(string: "https://api.bitbucket.org/2.0")!.appending(path: path, query: query)
        return try await http.decode(
            T.self, from: .get(url, headers: ["Authorization": authorization, "Accept": "application/json"]),
            using: Self.decoder)
    }

    // MARK: Data Center

    private func fetchDataCenter() async throws -> SourceSnapshot {
        let me = try await cache.value("user", lifetime: CacheLifetime.identity) { try await whoami() }
        async let authored: DataCenter.Page = dataCenter(
            "rest/api/1.0/dashboard/pull-requests", ["role": "AUTHOR", "state": "OPEN", "limit": "50"])
        // Waiting on you: not those you already approved or marked as needing work.
        async let reviewing: DataCenter.Page = dataCenter(
            "rest/api/1.0/dashboard/pull-requests",
            ["role": "REVIEWER", "state": "OPEN", "participantStatus": "UNAPPROVED", "limit": "50"])
        let (mine, theirs) = try await (authored, reviewing)
        var remarks = mine.isCut || theirs.isCut ? [SourceSnapshot.truncated("Bitbucket")] : []
        let all = mine.values.map { ($0, true) } + theirs.values.map { ($0, false) }
        let results = try await all.concurrentMap(limit: 4) { pr, isAuthored in
            let checks = await BestEffort.attempt { try await self.dataCenterChecks(pr) }
            return (pr, isAuthored, checks)
        }
        if let failed = results.lazy.compactMap({ $0.2.error }).first {
            try BestEffort.note("Build status", failed, remarks: &remarks)
        }
        let items = results.map { pr, isAuthored, checks in
            pr.item(authored: isAuthored, checks: (try? checks.get()) ?? .none, accountID: accountID)
        }
        return SourceSnapshot(identity: me, items: items, remarks: remarks)
    }

    /// Who the token belongs to: Data Center answers with the user name, as plain text.
    private func whoami() async throws -> String {
        let url = host.appending(path: "plugins/servlet/applinks/whoami")
        let (data, response) = try await http.send(.get(url, headers: ["Authorization": authorization]))
        switch response.statusCode {
        case 200..<300:
            let name = String(decoding: data, as: UTF8.self).trimmingCharacters(in: .whitespacesAndNewlines)
            // An empty answer is a token Bitbucket didn't take.
            guard !name.isEmpty else { throw HTTPError.unauthorized }
            return name
        case 401, 403: throw HTTPError.unauthorized
        case 429: throw HTTPError.rateLimited(response.resumeDate())
        default: throw HTTPError.status(response.statusCode)
        }
    }

    private func dataCenterChecks(_ pr: DataCenter.PullRequest) async throws -> CodeReview.Checks {
        guard let commit = pr.fromRef?.latestCommit else { return .none }
        let stats: DataCenter.BuildStats = try await dataCenter("rest/build-status/1.0/commits/stats/\(commit)")
        return stats.checks
    }

    private func dataCenter<T: Decodable>(_ path: String, _ query: [String: String] = [:]) async throws -> T {
        let url = host.appending(path: path, query: query)
        return try await http.decode(
            T.self, from: .get(url, headers: ["Authorization": authorization, "Accept": "application/json"]),
            using: Self.decoder)
    }

    // MARK: Dates

    /// Cloud's dates carry microseconds ("2026-10-12T09:14:03.123456+00:00"), which ISO8601DateFormatter refuses.
    static let decoder: JSONDecoder = {
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        decoder.dateDecodingStrategy = .custom { decoder in
            let container = try decoder.singleValueContainer()
            let raw = try container.decode(String.self)
            guard let date = date(raw) else {
                throw DecodingError.dataCorruptedError(in: container, debugDescription: "Invalid date \(raw)")
            }
            return date
        }
        return decoder
    }()

    static func date(_ raw: String) -> Date? {
        if let date = Date(iso8601: raw) { return date }
        // Keep milliseconds: "…03.123456+00:00" → "…03.123+00:00".
        let trimmed = raw.replacing(/(\.\d{3})\d+/) { $0.output.1 }
        return Date(iso8601: String(trimmed))
    }
}

// MARK: - Wire format

extension BitbucketPlugin {
    enum Cloud {
        struct Page<Value: Decodable & Sendable>: Decodable, Sendable {
            var values: [Value] = []
            /// Set when there is more.
            var next: String?
        }

        struct Link: Decodable, Sendable { var href: String? }
        struct Links: Decodable, Sendable {
            var html: Link?
            var avatar: Link?
        }

        struct User: Decodable, Sendable {
            var uuid: String
            var accountId: String?
            var displayName: String?
            var nickname: String?
            var links: Links?

            var person: Person {
                Person(name: displayName ?? nickname ?? "?", avatarURL: URL(lenient: links?.avatar?.href))
            }
        }

        struct Repository: Decodable, Sendable { var fullName: String }

        struct Participant: Decodable, Sendable {
            var user: User?
            var role: String?
            var approved: Bool?
            /// "approved", "changes_requested", or none.
            var state: String?

            var changesRequested: Bool { state == "changes_requested" }
        }

        struct Status: Decodable, Sendable { var state: String }

        struct PullRequest: Decodable, Sendable {
            struct Destination: Decodable, Sendable {
                struct Repo: Decodable, Sendable { var fullName: String? }
                var repository: Repo?
            }

            var id: Int
            var title: String
            var draft: Bool?
            var commentCount: Int?
            var updatedOn: Date
            var author: User?
            var destination: Destination?
            var links: Links?
            var participants: [Participant]?

            /// You approved it or asked for changes: nothing to review until it changes.
            func isReviewed(by me: User) -> Bool {
                (participants ?? []).contains {
                    $0.user?.uuid == me.uuid && ($0.approved == true || $0.changesRequested)
                }
            }

            func item(me: User, authored: Bool, checks: CodeReview.Checks, accountID: UUID) -> InboxItem {
                let others = (participants ?? []).filter { $0.user?.uuid != author?.uuid && $0.user?.uuid != me.uuid }
                let review = CodeReview(
                    isDraft: draft ?? false,
                    isApproved: others.contains { $0.approved == true },
                    changesRequested: others.contains(where: \.changesRequested),
                    checks: checks,
                    comments: commentCount ?? 0
                )
                let repository = destination?.repository?.fullName ?? ""
                return InboxItem(
                    id: "\(accountID.uuidString)/\(repository)/\(id)",
                    accountID: accountID,
                    pluginID: BitbucketPlugin.manifest.id,
                    bundle: authored ? .authored : .reviews,
                    title: Readable.text(title),
                    context: "\(repository) #\(id)",
                    url: URL(lenient: links?.html?.href),
                    author: author?.person,
                    participants: others.filter { $0.role == "REVIEWER" || $0.approved == true }.compactMap { one in
                        one.user.map { user in
                            var person = user.person
                            person.tone =
                                one.approved == true
                                ? ReviewerTone.approved
                                : one.changesRequested ? ReviewerTone.changes : ReviewerTone.waiting
                            return person
                        }
                    },
                    badges: review.badges(authored: authored),
                    date: updatedOn,
                    needsAction: review.needsAction(authored: authored)
                )
            }
        }
    }

    enum DataCenter {
        struct Page: Decodable, Sendable {
            var values: [PullRequest] = []
            var isLastPage: Bool?

            var isCut: Bool { isLastPage == false }
        }

        struct User: Decodable, Sendable {
            var name: String?
            var displayName: String?
        }

        struct Participant: Decodable, Sendable {
            var user: User?
            var approved: Bool?
            /// "APPROVED", "NEEDS_WORK" or "UNAPPROVED".
            var status: String?
        }

        struct PullRequest: Decodable, Sendable {
            struct Ref: Decodable, Sendable {
                struct Repository: Decodable, Sendable {
                    struct Project: Decodable, Sendable { var key: String? }
                    var slug: String?
                    var project: Project?
                }
                var latestCommit: String?
                var repository: Repository?
            }
            struct Links: Decodable, Sendable {
                struct Link: Decodable, Sendable { var href: String? }
                var own: [Link]?

                enum CodingKeys: String, CodingKey { case own = "self" }
            }
            struct Properties: Decodable, Sendable {
                struct Merge: Decodable, Sendable { var outcome: String? }
                var commentCount: Int?
                var mergeResult: Merge?
            }

            var id: Int
            var title: String
            var draft: Bool?
            /// Milliseconds since 1970.
            var updatedDate: Double?
            var author: Participant?
            var reviewers: [Participant]?
            var fromRef: Ref?
            var toRef: Ref?
            var links: Links?
            var properties: Properties?

            func item(authored: Bool, checks: CodeReview.Checks, accountID: UUID) -> InboxItem {
                let reviewers = reviewers ?? []
                let review = CodeReview(
                    isDraft: draft ?? false,
                    isApproved: reviewers.contains { $0.status == "APPROVED" || $0.approved == true },
                    // "Needs work" is Data Center's "changes requested".
                    changesRequested: reviewers.contains { $0.status == "NEEDS_WORK" },
                    hasConflicts: properties?.mergeResult?.outcome == "CONFLICTED",
                    checks: checks,
                    comments: properties?.commentCount ?? 0
                )
                let repository = toRef?.repository
                let path = [repository?.project?.key, repository?.slug].compactMap { $0 }.joined(separator: "/")
                return InboxItem(
                    id: "\(accountID.uuidString)/\(path)/\(id)",
                    accountID: accountID,
                    pluginID: BitbucketPlugin.manifest.id,
                    bundle: authored ? .authored : .reviews,
                    title: Readable.text(title),
                    context: "\(path) #\(id)",
                    url: URL(lenient: links?.own?.first?.href),
                    author: author?.user.map { Person(name: $0.displayName ?? $0.name ?? "?") },
                    participants: reviewers.compactMap { reviewer in
                        reviewer.user.map {
                            Person(
                                name: $0.displayName ?? $0.name ?? "?",
                                tone: reviewer.status == "APPROVED"
                                    ? ReviewerTone.approved
                                    : reviewer.status == "NEEDS_WORK" ? ReviewerTone.changes : ReviewerTone.waiting)
                        }
                    },
                    badges: review.badges(authored: authored),
                    date: updatedDate.map { Date(timeIntervalSince1970: $0 / 1000) } ?? .now,
                    needsAction: review.needsAction(authored: authored)
                )
            }
        }

        /// What the builds of a commit add up to.
        struct BuildStats: Decodable, Sendable {
            var successful: Int?
            var failed: Int?
            var inProgress: Int?

            var checks: CodeReview.Checks {
                if (failed ?? 0) > 0 { return .failing }
                if (inProgress ?? 0) > 0 { return .running }
                return (successful ?? 0) > 0 ? .passing : .none
            }
        }
    }
}

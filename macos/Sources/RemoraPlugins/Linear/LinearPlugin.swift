import Foundation
import RemoraCore

/// Open issues assigned to you, plus unread mentions and comments from Linear's inbox.
public struct LinearPlugin: SourcePlugin {
    public static let manifest = PluginManifest(
        id: "linear",
        name: "Linear",
        symbol: "square.stack.3d.up",
        summary: "Issues assigned to you, and the mentions and comments waiting in your Linear inbox.",
        fields: [.token("Personal API key", help: "Linear → Settings → Security & access → Personal API keys.")],
        setupSteps: [
            "Click “Create an API key”: Linear opens its Security & access settings.",
            "Under Personal API keys, create a key named Remora (read access is enough) and paste it below.",
        ],
        setupLabel: "Create an API key",
        setupURL: { _ in URL(string: "https://linear.app/settings/account/security") },
        egress: Egress(
            hosts: ["api.linear.app", "public.linear.app"],
            description: "Reads issues assigned to you and your Linear inbox."),
        logo: "linear"
    )

    /// Notifications that ask something of you. Assignments already arrive as issues; reactions and
    /// status changes stay out of the inbox.
    static let actionableTypes = ["mention", "comment", "reply"]

    private let accountID: UUID
    private let token: String
    private let http: HTTPClient
    private let endpoint = URL(string: "https://api.linear.app/graphql")!

    public init(config: PluginConfig, http: HTTPClient) throws {
        accountID = config.accountID
        token = try config.required("token")
        self.http = http
    }

    public func fetch() async throws -> SourceSnapshot {
        let issues: IssuesData = try await query(Self.issuesQuery)
        // Notifications are a bonus: if they fail, issues still come through, and a remark says so.
        var remarks: [String] = []
        let notifications =
            try await BestEffort.value("Mentions and comments", remarks: &remarks) {
                try await query(Self.notificationsQuery) as NotificationsData
            }?.notifications.nodes ?? []
        var snapshot = Self.snapshot(
            viewer: issues.viewer, notifications: notifications, accountID: accountID, now: .now)
        snapshot.remarks += remarks
        return snapshot
    }

    static func snapshot(viewer: Viewer, notifications: [Notification], accountID: UUID, now: Date) -> SourceSnapshot {
        let weekAgo = now.addingTimeInterval(-7 * .day)
        let unread = notifications.filter { $0.readAt == nil && $0.createdAt > weekAgo && $0.isActionable }
        // An issue someone wrote to you about goes first in its priority: its comment's link is the issue's.
        let discussed = Set(unread.compactMap { $0.url.map(LinkFinder.address) })
        let issues = viewer.assignedIssues.nodes.map {
            $0.item(
                accountID: accountID, now: now,
                discussed: $0.url.map { discussed.contains(LinkFinder.address($0)) } ?? false)
        }
        let mentions = unread.map { $0.item(accountID: accountID) }
        let truncated = viewer.assignedIssues.pageInfo?.hasNextPage == true
        return SourceSnapshot(
            identity: viewer.name, items: issues + mentions,
            remarks: truncated ? [SourceSnapshot.truncated("Linear")] : [])
    }

    /// https://linear.app/acme/issue/ENG-42/slug → linear://acme/issue/ENG-42/slug, opened by the desktop app.
    static func appURL(for url: URL?) -> URL? {
        guard let url, url.host == "linear.app" else { return nil }
        return URL(string: "linear:/" + url.path)
    }

    private func query<T: Decodable>(_ text: String) async throws -> T {
        let request = try URLRequest.post(endpoint, json: ["query": text], headers: ["Authorization": token])
        let response = try await http.decode(Response<T>.self, from: request)
        guard let data = response.data else {
            let message = response.errors?.first?.message ?? L("the request failed.")
            // "Authentication required, not authenticated": a key Linear no longer accepts.
            if message.lowercased().contains("authenticat") { throw HTTPError.unauthorized }
            throw HTTPError.api("Linear: \(message)")
        }
        return data
    }

    static let issuesQuery = """
        query {
          viewer {
            name
            assignedIssues(first: 50, orderBy: updatedAt, filter: { state: { type: { nin: ["completed", "canceled"] } } }) {
              nodes {
                id identifier title url priority dueDate createdAt updatedAt state { name type }
                cycle { startsAt endsAt }
                inverseRelations(first: 10) { nodes { type issue { identifier state { type } } } }
                comments(last: 5) { nodes { resolvedAt user { isMe } botActor { id } } }
                attachments(first: 10) { nodes { url metadata } }
              }
              pageInfo { hasNextPage }
            }
          }
        }
        """

    static let notificationsQuery = """
        query {
          notifications(first: 50) {
            nodes { id type title url readAt createdAt actor { name avatarUrl } }
          }
        }
        """
}

// MARK: - Wire format

extension LinearPlugin {
    struct Response<T: Decodable>: Decodable {
        struct Message: Decodable { var message: String }
        var data: T?
        var errors: [Message]?
    }

    struct Nodes<T: Decodable>: Decodable {
        struct PageInfo: Decodable { var hasNextPage: Bool }
        var nodes: [T]
        var pageInfo: PageInfo?
    }

    struct IssuesData: Decodable { var viewer: Viewer }
    struct NotificationsData: Decodable { var notifications: Nodes<Notification> }

    struct Viewer: Decodable {
        var name: String
        var assignedIssues: Nodes<Issue>
    }

    struct Issue: Decodable {
        struct State: Decodable {
            var name: String
            var type: String?
        }

        var id: String
        var identifier: String
        var title: String
        var url: URL?
        var priority: Int?
        var dueDate: String?
        var updatedAt: Date
        var state: State?
        var attachments: Nodes<Attachment>?
        var createdAt: Date?
        var cycle: Cycle?
        /// Relations pointing at it: "blocks" from another issue blocks this one.
        var inverseRelations: Nodes<Relation>?
        /// The latest comments, oldest first.
        var comments: Nodes<Comment>?

        struct Comment: Decodable {
            struct Author: Decodable { var isMe: Bool? }
            struct Bot: Decodable { var id: String? }
            var resolvedAt: Date?
            var user: Author?
            var botActor: Bot?
        }

        /// The latest comment written by a person is someone else's, and its thread isn't resolved: they wait on
        /// you, even when Linear's notification was read long ago.
        var awaitsYourAnswer: Bool {
            guard let last = (comments?.nodes ?? []).last(where: { $0.botActor == nil && $0.user != nil }) else {
                return false
            }
            return last.user?.isMe == false && last.resolvedAt == nil
        }

        /// Started, then put aside ("Paused", "On hold"): it waits like an issue not started.
        var paused: Bool { state?.type == "started" && TaskStage.paused(state?.name) }

        struct Cycle: Decodable {
            var startsAt: Date?
            var endsAt: Date?
        }

        struct Relation: Decodable {
            struct Other: Decodable {
                /// Only its type is asked for.
                struct Kind: Decodable { var type: String? }
                var identifier: String
                var state: Kind?
            }
            var type: String?
            var issue: Other?
        }

        /// The issues still open that block it.
        var blockers: [String] {
            (inverseRelations?.nodes ?? []).compactMap { relation in
                guard relation.type == "blocks", let other = relation.issue,
                    !["completed", "canceled"].contains(other.state?.type ?? "")
                else { return nil }
                return other.identifier
            }
        }

        func inCurrentCycle(_ now: Date) -> Bool {
            guard let start = cycle?.startsAt, let end = cycle?.endsAt else { return false }
            return start <= now && now < end
        }

        /// Where it stands, from its status type. A comment waiting for you comes first, a blocker after the rest;
        /// a status Linear doesn't type stands with the rest.
        func stage(now: Date, discussed: Bool) -> TaskStage? {
            if discussed || awaitsYourAnswer { return .discussed }
            if !blockers.isEmpty { return .blocked }
            if paused { return .todo }
            switch state?.type {
            case "started": return .started
            case "triage": return .triage
            // A duplicate is another issue's: last, still shown.
            case "backlog", "duplicate": return .backlog
            default: return inCurrentCycle(now) ? .planned : state?.type == "unstarted" ? .todo : nil
            }
        }

        /// Its pull and merge requests (Linear's GitHub and GitLab integrations, or a link pasted by hand).
        var changes: [Attachment] { (attachments?.nodes ?? []).filter { $0.url.map(InReview.isChange) ?? false } }

        /// Up for review: a pull request still open, or a review status ("In Review"). Not from the backlog or triage,
        /// which nobody has started.
        var inReview: Bool {
            guard state?.type != "backlog", state?.type != "triage" else { return false }
            return changes.contains(where: \.isOpen) || InReview.status(state?.name)
        }

        /// `discussed`: a mention or comment on it still unread in your Linear inbox.
        func item(accountID: UUID, now: Date = .now, discussed: Bool = false) -> InboxItem {
            var badges: [Badge] = []
            switch priority {
            case 1:
                badges.append(
                    Badge(
                        id: .priorityUrgent, label: L("Urgent"), symbol: "exclamationmark.3", tone: .negative,
                        notify: .init(title: L("Urgent issue"))))
            case 2:
                badges.append(Badge(id: .priorityHigh, label: L("High"), symbol: "exclamationmark.2", tone: .warning))
            default: break
            }
            let due = DueDate.parse(dueDate)
            if let due { badges.append(DueDate.badge(for: due)) }
            if let state { badges.append(Badge(id: .status, label: state.name, tone: .neutral)) }
            let stage = stage(now: now, discussed: discussed)
            // Someone wrote to you: a low or backlog issue is counted again.
            let priority = stage == .discussed ? max(itemPriority, .normal) : itemPriority
            let unstarted = state?.type != "started" || paused
            let longOpen = createdAt.map { now.timeIntervalSince($0) > TaskStage.longOpen } ?? false
            badges += TaskStage.badges(
                discussed: stage == .discussed, blocked: !blockers.isEmpty, planned: stage == .planned,
                longOpen: priority >= .high && unstarted && longOpen, sprint: false)
            var item = InboxItem(
                id: InboxItem.makeID(account: accountID, local: "\(id)"),
                accountID: accountID,
                pluginID: LinearPlugin.manifest.id,
                bundle: .tasks,
                title: Readable.text(title),
                context: identifier,
                url: url,
                badges: badges,
                date: updatedAt,
                needsAction: priority != .low,
                priority: priority,
                due: due,
                appURL: LinearPlugin.appURL(for: url)
            )
            item.stage = stage
            if inReview { return item.waitingOnReview(changes.compactMap(\.url)) }
            return stage == .blocked ? item.blockedByAnother() : item
        }

        /// Linear's 1 urgent … 4 low (0 = none). Backlog counts as low unless urgent or high.
        var itemPriority: Priority {
            let mapped: Priority =
                switch priority {
                case 1: .urgent
                case 2: .high
                case 4: .low
                default: .normal
                }
            if state?.type == "backlog" || state?.type == "duplicate", mapped < .high { return .low }
            return mapped
        }
    }

    /// A link on an issue. Its metadata is the integration's own: read leniently, so an unexpected shape never fails
    /// the fetch.
    struct Attachment: Decodable {
        var url: URL?
        /// A pull request's state as the integration last saw it ("open", "merged", "closed"…), when it says.
        var status: String?

        /// Open unless the integration says it was merged or closed.
        var isOpen: Bool { !["merged", "closed"].contains(status?.lowercased() ?? "") }

        init(url: URL?, status: String? = nil) {
            self.url = url
            self.status = status
        }

        private enum CodingKeys: String, CodingKey { case url, metadata }
        private struct Metadata: Decodable { var status: String? }

        init(from decoder: any Decoder) throws {
            let container = try decoder.container(keyedBy: CodingKeys.self)
            url = URL(lenient: (try? container.decodeIfPresent(String.self, forKey: .url)) ?? nil)
            status = (try? container.decodeIfPresent(Metadata.self, forKey: .metadata))??.status
        }
    }

    struct Notification: Decodable {
        struct Actor: Decodable {
            var name: String
            var avatarUrl: String?
        }

        var id: String
        var type: String
        var title: String?
        var url: URL?
        var readAt: Date?
        var createdAt: Date
        var actor: Actor?

        var isActionable: Bool {
            let type = type.lowercased()
            return LinearPlugin.actionableTypes.contains { type.contains($0) }
        }

        func item(accountID: UUID) -> InboxItem {
            InboxItem(
                id: InboxItem.makeID(account: accountID, local: "notification/\(id)"),
                accountID: accountID,
                pluginID: LinearPlugin.manifest.id,
                bundle: .mentions,
                title: title.map(Readable.text) ?? L("New comment"),
                context: "Linear",
                url: url,
                author: actor.map { Person(name: $0.name, avatarURL: URL(lenient: $0.avatarUrl)) },
                date: createdAt,
                needsAction: true,
                appURL: LinearPlugin.appURL(for: url)
            )
        }
    }
}

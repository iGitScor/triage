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
        egress: Egress(hosts: ["api.linear.app", "public.linear.app"], description: "Reads issues assigned to you and your Linear inbox."),
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
        // Notifications are a bonus: if their shape differs, issues still come through.
        let notifications = (try? await query(Self.notificationsQuery) as NotificationsData)?.notifications.nodes ?? []
        return Self.snapshot(viewer: issues.viewer, notifications: notifications, accountID: accountID, now: .now)
    }

    static func snapshot(viewer: Viewer, notifications: [Notification], accountID: UUID, now: Date) -> SourceSnapshot {
        let issues = viewer.assignedIssues.nodes.map { $0.item(accountID: accountID) }
        let weekAgo = now.addingTimeInterval(-7 * 86_400)
        let mentions = notifications
            .filter { $0.readAt == nil && $0.createdAt > weekAgo && $0.isActionable }
            .map { $0.item(accountID: accountID) }
        return SourceSnapshot(identity: viewer.name, items: issues + mentions)
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
            throw HTTPError.api("Linear: \(response.errors?.first?.message ?? L("the request failed."))")
        }
        return data
    }

    static let issuesQuery = """
    query {
      viewer {
        name
        assignedIssues(first: 50, orderBy: updatedAt, filter: { state: { type: { nin: ["completed", "canceled"] } } }) {
          nodes { id identifier title url priority dueDate updatedAt state { name type } }
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

    struct Nodes<T: Decodable>: Decodable { var nodes: [T] }

    struct IssuesData: Decodable { var viewer: Viewer }
    struct NotificationsData: Decodable { var notifications: Nodes<Notification> }

    struct Viewer: Decodable {
        var name: String
        var assignedIssues: Nodes<Issue>
    }

    struct Issue: Decodable {
        struct State: Decodable { var name: String; var type: String? }

        var id: String
        var identifier: String
        var title: String
        var url: URL?
        var priority: Int?
        var dueDate: String?
        var updatedAt: Date
        var state: State?

        func item(accountID: UUID) -> InboxItem {
            var badges: [Badge] = []
            switch priority {
            case 1: badges.append(Badge(id: "priority.urgent", label: L("Urgent"), symbol: "exclamationmark.3", tone: .negative,
                                        notify: .init(title: L("Urgent issue"))))
            case 2: badges.append(Badge(id: "priority.high", label: L("High"), symbol: "exclamationmark.2", tone: .warning))
            default: break
            }
            let due = DueDate.parse(dueDate)
            if let due { badges.append(DueDate.badge(for: due)) }
            if let state { badges.append(Badge(id: "status", label: state.name, tone: .neutral)) }
            let priority = itemPriority
            return InboxItem(
                id: "\(accountID.uuidString)/\(id)",
                accountID: accountID,
                pluginID: LinearPlugin.manifest.id,
                bundle: .tasks,
                title: title,
                context: identifier,
                url: url,
                badges: badges,
                date: updatedAt,
                needsAction: priority != .low,
                priority: priority,
                due: due,
                appURL: LinearPlugin.appURL(for: url)
            )
        }

        /// Linear's 1 urgent … 4 low (0 = none). Backlog counts as low unless urgent or high.
        var itemPriority: Priority {
            let mapped: Priority = switch priority {
            case 1: .urgent
            case 2: .high
            case 4: .low
            default: .normal
            }
            if state?.type == "backlog", mapped < .high { return .low }
            return mapped
        }
    }

    struct Notification: Decodable {
        struct Actor: Decodable { var name: String; var avatarUrl: String? }

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
                id: "\(accountID.uuidString)/notification/\(id)",
                accountID: accountID,
                pluginID: LinearPlugin.manifest.id,
                bundle: .mentions,
                title: title ?? L("New comment"),
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

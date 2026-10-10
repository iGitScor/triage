import Foundation
import RemoraCore

/// Open tasks assigned to you in the Notion databases you pick.
/// Notion's public API has no notifications endpoint, so tasks are the actionable signal.
public struct NotionPlugin: SourcePlugin {
    public static let manifest = PluginManifest(
        id: "notion",
        name: "Notion",
        symbol: "doc.text",
        summary: "Open tasks assigned to you in the databases you choose.",
        fields: [
            .token("Personal access token", help: "A personal access token, or the secret of an internal connection."),
            ConfigField(
                key: "databases", label: "Task databases", placeholder: "Database links, comma separated",
                help: "Open the database as a full page, then ••• → Copy link."),
            ConfigField(
                key: "assignee", label: "Assignee property", defaultValue: "Assignee",
                help: "The people property that says who a task is for."),
            ConfigField(key: "doneValues", label: "Done statuses", defaultValue: "Done, Complete, Archived"),
            ConfigField(
                key: "email", label: "Your Notion email", placeholder: "you@company.com", isOptional: true,
                help: "Only needed with an internal connection, to find your tasks."),
        ],
        setupSteps: [
            "Click “Create a token”, then “New token”. Name it Remora, keep the “Notion API” capability, pick an expiration and create it.",
            "Copy the token (Notion shows it only once) and paste it below.",
            "Paste the links of your task databases. No sharing needed: the token sees what you see.",
            "No “New token” button? Your workspace limits tokens to owners: ask one, or use an internal connection’s secret.",
        ],
        setupLabel: "Create a token",
        setupURL: { _ in URL(string: "https://www.notion.so/developers/tokens") },
        egress: Egress(
            hosts: ["api.notion.com"], description: "Reads open tasks from the Notion databases you choose."),
        logo: "notion"
    )

    private let accountID: UUID
    private let token: String
    private let databases: [String]
    private let email: String
    private let assignee: String
    private let doneValues: Set<String>
    private let http: HTTPClient
    private let api = URL(string: "https://api.notion.com/v1")!

    public init(config: PluginConfig, http: HTTPClient) throws {
        accountID = config.accountID
        token = try config.required("token")
        databases = try config.required("databases").split(separator: ",").map { Self.databaseID(from: String($0)) }
        email = config["email"].lowercased()
        assignee = config["assignee"].nilIfEmpty ?? "Assignee"
        doneValues = Set(
            config["doneValues"].split(separator: ",").map { $0.trimmingCharacters(in: .whitespaces).lowercased() })
        self.http = http
    }

    public func fetch() async throws -> SourceSnapshot {
        // Without knowing who you are, the query would return everyone's tasks: better say so.
        guard let me = try await currentUser() else {
            throw HTTPError.api(
                L("Remora can’t tell which Notion user you are: add your Notion email to this account."))
        }
        let found = try await databases.concurrentMap(limit: 4) { id in try await self.tasks(in: id, assignedTo: me.id)
        }
        let truncated = found.contains(where: \.more)
        return SourceSnapshot(
            identity: me.label, items: found.flatMap(\.items),
            remarks: truncated ? [SourceSnapshot.truncated("Notion")] : []
        )
    }

    /// The database's tasks for you, and whether it has more than one page of them.
    private func tasks(in databaseID: String, assignedTo user: String) async throws -> (items: [InboxItem], more: Bool)
    {
        let database: Database = try await send(.get(api.appending(path: "databases/\(databaseID)"), headers: headers))
        var body = Query(sorts: [.init(timestamp: "last_edited_time", direction: "descending")], pageSize: 50)
        body.filter = .init(property: assignee, people: .init(contains: user))
        let query = body
        let pages = try await database.dataSources.concurrentMap(limit: 2) { source -> Results<Page> in
            let request = try URLRequest.post(
                self.api.appending(path: "data_sources/\(source.id)/query"), json: query, headers: self.headers)
            return try await self.send(request)
        }
        let items = pages.flatMap(\.results)
            .filter { !$0.isDone(doneValues) }
            .map { $0.item(accountID: accountID, database: database.name) }
        return (items, pages.contains { $0.nextCursor != nil })
    }

    /// A personal access token is the user itself; an internal connection needs the email to find them.
    private func currentUser() async throws -> (id: String, label: String)? {
        let me: User = try await send(.get(api.appending(path: "users/me"), headers: headers))
        if me.type == "person" { return (me.id, me.name ?? me.person?.email ?? "Notion") }
        if let owner = me.bot?.owner?.user, owner.type == "person" { return (owner.id, owner.name ?? "Notion") }
        guard !email.isEmpty else { return nil }
        return (try await userID(for: email), email)
    }

    private func userID(for email: String) async throws -> String {
        var cursor: String?
        repeat {
            var query = ["page_size": "100"]
            if let cursor { query["start_cursor"] = cursor }
            let page: Results<User> = try await send(.get(api.appending(path: "users", query: query), headers: headers))
            if let user = page.results.first(where: { $0.person?.email?.lowercased() == email }) { return user.id }
            cursor = page.nextCursor
        } while cursor != nil
        throw HTTPError.api(L("No Notion user with the email %@.", email))
    }

    private var headers: [String: String] {
        ["Authorization": "Bearer \(token)", "Notion-Version": "2025-09-03"]
    }

    private func send<T: Decodable>(_ request: URLRequest) async throws -> T {
        try await http.decode(T.self, from: request, using: .api(snakeCase: true))
    }

    /// Accepts a raw ID or a notion.so link and returns the 32-character ID.
    static func databaseID(from raw: String) -> String {
        let trimmed = raw.trimmingCharacters(in: .whitespaces)
        let candidate = trimmed.split(separator: "?").first.map(String.init) ?? trimmed
        let hex = candidate.replacingOccurrences(of: "-", with: "")
        guard let range = hex.range(of: "[0-9a-fA-F]{32}$", options: .regularExpression) else { return trimmed }
        return String(hex[range])
    }
}

// MARK: - Wire format

extension NotionPlugin {
    struct Query: Encodable {
        struct Sort: Encodable {
            var timestamp: String
            var direction: String
        }
        struct Filter: Encodable {
            struct People: Encodable { var contains: String }
            var property: String
            var people: People
        }

        var filter: Filter?
        var sorts: [Sort]
        var pageSize: Int

        enum CodingKeys: String, CodingKey {
            case filter, sorts
            case pageSize = "page_size"
        }
    }

    struct Results<T: Decodable & Sendable>: Decodable, Sendable {
        var results: [T]
        var nextCursor: String?
    }

    struct User: Decodable {
        struct Person: Decodable { var email: String? }
        struct Owner: Decodable { var user: Member? }
        struct Bot: Decodable { var owner: Owner? }
        struct Member: Decodable {
            var id: String
            var type: String?
            var name: String?
        }

        var id: String
        var type: String?
        var name: String?
        var person: Person?
        var bot: Bot?
    }

    struct RichText: Decodable { var plainText: String }

    struct Database: Decodable {
        struct Source: Decodable {
            var id: String
            var name: String?
        }
        var title: [RichText]?
        var dataSources: [Source]
        var name: String { (title ?? []).map(\.plainText).joined().nilIfEmpty ?? dataSources.first?.name ?? "Notion" }
    }

    struct Page: Decodable, Sendable {
        var id: String
        var url: URL?
        var lastEditedTime: Date
        var properties: [String: Property]

        var title: String {
            properties.values.compactMap(\.title).first.map { $0.map(\.plainText).joined() }?.nilIfEmpty
                ?? L("Untitled")
        }
        /// The status property, or a select named "Status": not any select, which could be a priority.
        var status: String? {
            properties.values.first { $0.type == "status" }?.status?.name
                ?? properties.first { $0.key.lowercased() == "status" }?.value.select?.name
        }

        /// Done when its status is one of the done values, or a checkbox named like one ("Done", "Complete") is
        /// ticked. Any other checkbox ("Urgent", "Blocked") says nothing about being done.
        func isDone(_ doneValues: Set<String>) -> Bool {
            if let status, doneValues.contains(status.lowercased()) { return true }
            let doneNames = doneValues.union(["done", "complete", "completed"])
            return properties.contains { doneNames.contains($0.key.lowercased()) && $0.value.checkbox == true }
        }
        var due: Date? { properties.values.compactMap { DueDate.parse($0.date?.start) }.min() }

        func item(accountID: UUID, database: String) -> InboxItem {
            var badges: [Badge] = []
            if let status { badges.append(Badge(id: "status", label: status, tone: .neutral)) }
            if let due { badges.append(DueDate.badge(for: due)) }
            return InboxItem(
                id: "\(accountID.uuidString)/\(id)",
                accountID: accountID,
                pluginID: NotionPlugin.manifest.id,
                bundle: .tasks,
                title: Readable.text(title),
                context: database,
                url: url,
                author: Person(name: database),
                badges: badges,
                date: lastEditedTime,
                needsAction: true,
                due: due
            )
        }
    }

    struct Property: Decodable {
        struct Named: Decodable { var name: String }
        struct DateValue: Decodable { var start: String? }

        var type: String?
        var title: [RichText]?
        var status: Named?
        var select: Named?
        var checkbox: Bool?
        var date: DateValue?
    }
}

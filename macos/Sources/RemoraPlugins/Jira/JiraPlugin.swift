import Foundation
import RemoraCore

/// Issues assigned to you, and recent comments that mention you, on Jira Cloud or Data Center. On Cloud, if turned
/// on, the Confluence pages and comments that mention you too, read on the same site with the same token.
public struct JiraPlugin: SourcePlugin {
    public static let manifest = PluginManifest(
        id: "jira",
        name: "Jira",
        symbol: "checklist",
        summary: "Issues assigned to you, and comments that mention you. Jira Cloud or Data Center.",
        fields: [
            ConfigField(key: "host", label: "Host", placeholder: "https://your-company.atlassian.net"),
            ConfigField(
                key: "email", label: "E-mail", placeholder: "you@company.com", isOptional: true,
                help: "Jira Cloud only: the e-mail you sign in with. Leave empty for Data Center."),
            .token("API token", help: "Jira Cloud: an API token. Data Center: a personal access token."),
            .toggle(
                "confluence", label: "Mentions in Confluence",
                help: "Jira Cloud only: pages and comments that mention you in Confluence, with the same token."),
        ],
        setupSteps: [
            "Set your Jira host first, for example your-company.atlassian.net.",
            "Jira Cloud: click “Create a token”, create an API token named Remora, and enter your e-mail too.",
            "Data Center: in Jira, open your profile → Personal Access Tokens, create one, and leave the e-mail empty.",
            "Paste the token below.",
        ],
        setupURL: { _ in URL(string: "https://id.atlassian.com/manage-profile/security/api-tokens") },
        egress: Egress(
            hosts: ["atlassian.net"],
            description:
                "Reads issues assigned to you and recent comments on issues you watch, from your Jira host. If turned on, the Confluence pages and comments that mention you, on the same site."
        ),
        logo: "jira"
    )

    static let assignedJQL = "assignee = currentUser() AND statusCategory != Done ORDER BY updated DESC"
    /// Issues you watch (Jira adds you to those you create, are assigned or comment on) with recent activity: their
    /// comments are read for mentions of you.
    static let watchedJQL = "watcher = currentUser() AND updated >= -7d ORDER BY updated DESC"
    /// Confluence pages and comments that mention you, changed in the last week.
    static let mentionCQL = #"mention = currentUser() AND lastmodified >= now("-7d") ORDER BY lastmodified DESC"#

    private let accountID: UUID
    private let host: URL
    private let authorization: String
    private let isCloud: Bool
    /// Confluence lives on the same Cloud site, under `/wiki`; Data Center's is a separate server.
    private let readsConfluence: Bool
    private let http: HTTPClient
    private let cache: any AccountCache

    public init(config: PluginConfig, http: HTTPClient) throws {
        accountID = config.accountID
        host = try config.url("host")
        let token = try config.required("token")
        let email = config["email"]
        // Cloud signs in with the e-mail and an API token; Data Center with a personal access token.
        isCloud = !email.isEmpty
        if host.host?.lowercased().hasSuffix(".atlassian.net") == true, !isCloud {
            throw PluginError.missingField("email")
        }
        readsConfluence = isCloud && config["confluence"] == "true"
        authorization =
            isCloud ? "Basic " + Data("\(email):\(token)".utf8).base64EncodedString() : "Bearer \(token)"
        self.http = http
        cache = config.cache
    }

    public func fetch() async throws -> SourceSnapshot {
        let me: Myself = try await cache.value("myself", lifetime: CacheLifetime.identity) { try await get("myself") }
        async let assigned = search(Self.assignedJQL, fields: "summary,priority,duedate,status,updated", max: 50)
        // Mentions are a bonus: if that search fails, assigned issues still come through, and a remark says so.
        var remarks: [String] = []
        let watched =
            try await BestEffort.value("Mentions and comments", remarks: &remarks) {
                try await search(Self.watchedJQL, fields: "summary,comment", max: 25)
            } ?? SearchPage()
        var pages = ConfluencePage()
        if readsConfluence {
            pages =
                try await BestEffort.value("Confluence mentions", remarks: &remarks) { try await confluenceMentions() }
                ?? pages
        }
        var snapshot = try await Self.snapshot(
            me: me, assigned: assigned, watched: watched, confluence: pages, host: host, accountID: accountID,
            now: .now)
        snapshot.remarks += remarks
        return snapshot
    }

    static func snapshot(
        me: Myself, assigned: SearchPage, watched: SearchPage, confluence: ConfluencePage = ConfluencePage(),
        host: URL, accountID: UUID, now: Date
    ) -> SourceSnapshot {
        let issues = assigned.issues.map { $0.item(host: host, accountID: accountID) }
        let weekAgo = now.addingTimeInterval(-7 * .day)
        let mentions = watched.issues.flatMap { issue in
            (issue.fields.comment?.comments ?? [])
                .filter { comment in
                    guard let created = jiraDate(comment.created), created > weekAgo else { return false }
                    return !me.isAuthor(of: comment) && comment.body.mentions(me)
                }
                .map { $0.item(issue: issue, host: host, accountID: accountID) }
        }
        // Not the pages you changed last: you have seen those.
        let pages = confluence.results
            .filter { me.accountId == nil || $0.content?.history?.lastUpdated?.by?.accountId != me.accountId }
            .compactMap { $0.item(site: Self.site(host), accountID: accountID) }
        return SourceSnapshot(
            identity: me.displayName, items: issues + mentions + pages,
            remarks: assigned.isCut ? [SourceSnapshot.truncated("Jira")] : [])
    }

    /// Cloud's search is `search/jql` (the older `search` is gone there); Data Center keeps `api/2/search`.
    private func search(_ jql: String, fields: String, max: Int) async throws -> SearchPage {
        isCloud
            ? try await get("search/jql", ["jql": jql, "fields": fields, "maxResults": String(max)])
            : try await get("search", ["jql": jql, "fields": fields, "maxResults": String(max)])
    }

    /// Confluence's search, on the same site: CQL finds what mentions you, pages and comments alike.
    private func confluenceMentions() async throws -> ConfluencePage {
        let query = ["cql": Self.mentionCQL, "limit": "25", "expand": "content.history.lastUpdated,content.container"]
        return try await http.decode(
            ConfluencePage.self,
            from: .get(
                host.appending(path: "wiki/rest/api/search", query: query),
                headers: ["Authorization": authorization, "Accept": "application/json"]))
    }

    /// The site's address, without a trailing slash: Confluence's links are paths to add to it.
    static func site(_ host: URL) -> String {
        let site = host.absoluteString
        return site.hasSuffix("/") ? String(site.dropLast()) : site
    }

    private func get<T: Decodable>(_ path: String, _ query: [String: String] = [:]) async throws -> T {
        let url = host.appending(path: "rest/api/\(isCloud ? 3 : 2)/\(path)", query: query)
        return try await http.decode(
            T.self, from: .get(url, headers: ["Authorization": authorization, "Accept": "application/json"]))
    }

    /// "2026-10-12T09:14:03.123+0200", Jira's timestamps: ISO 8601 without the colon in the offset.
    static func jiraDate(_ raw: String?) -> Date? {
        guard var raw, !raw.isEmpty else { return nil }
        if let sign = raw.dropLast(4).last, sign == "+" || sign == "-", !raw.hasSuffix("Z"),
            raw.suffix(4).allSatisfy(\.isNumber)
        {
            raw.insert(":", at: raw.index(raw.endIndex, offsetBy: -2))
        }
        return Date(iso8601: raw)
    }
}

// MARK: - Wire format

extension JiraPlugin {
    struct Myself: Decodable {
        /// Cloud's id; mentions carry it.
        var accountId: String?
        /// Data Center's user name; mentions are `[~name]`.
        var name: String?
        var key: String?
        var displayName: String

        func isAuthor(of comment: Comment) -> Bool {
            if let id = accountId, comment.author?.accountId == id { return true }
            if let name, comment.author?.name == name { return true }
            return false
        }
    }

    struct SearchPage: Decodable {
        var issues: [Issue] = []
        /// Cloud: set when there is more.
        var nextPageToken: String?
        /// Data Center: how many match in all.
        var total: Int?

        var isCut: Bool { nextPageToken != nil || (total ?? 0) > issues.count }
    }

    struct Person: Decodable {
        var accountId: String?
        var name: String?
        var displayName: String?
        var avatarUrls: [String: String]?

        var person: RemoraCore.Person? {
            displayName.map { RemoraCore.Person(name: $0, avatarURL: URL(lenient: avatarUrls?["48x48"])) }
        }
    }

    struct Issue: Decodable {
        struct Fields: Decodable {
            struct Named: Decodable { var name: String }
            struct Status: Decodable {
                struct Category: Decodable { var key: String? }
                var name: String
                var statusCategory: Category?
            }
            struct Comments: Decodable { var comments: [Comment] }

            var summary: String?
            var priority: Named?
            var duedate: String?
            var status: Status?
            var updated: String?
            var comment: Comments?
        }

        var id: String
        var key: String
        var fields: Fields

        func url(host: URL) -> URL { host.appending(path: "browse/\(key)") }

        func item(host: URL, accountID: UUID) -> InboxItem {
            var badges: [Badge] = []
            let priority = itemPriority
            switch priority {
            case .urgent:
                badges.append(
                    Badge(
                        id: .priorityUrgent, label: L("Urgent"), symbol: "exclamationmark.3", tone: .negative,
                        notify: .init(title: L("Urgent issue"))))
            case .high:
                badges.append(Badge(id: .priorityHigh, label: L("High"), symbol: "exclamationmark.2", tone: .warning))
            default: break
            }
            let due = DueDate.parse(fields.duedate)
            if let due { badges.append(DueDate.badge(for: due)) }
            if let status = fields.status { badges.append(Badge(id: .status, label: status.name, tone: .neutral)) }
            return InboxItem(
                id: InboxItem.makeID(account: accountID, local: "\(id)"),
                accountID: accountID,
                pluginID: JiraPlugin.manifest.id,
                bundle: .tasks,
                title: Readable.text(fields.summary ?? key),
                context: key,
                url: url(host: host),
                badges: badges,
                date: JiraPlugin.jiraDate(fields.updated) ?? .now,
                needsAction: priority != .low,
                priority: priority,
                due: due
            )
        }

        /// Jira's default priorities by name (Highest … Lowest), and the older Blocker … Trivial.
        var itemPriority: Priority {
            switch fields.priority?.name.lowercased() {
            case "highest", "blocker", "critical": .urgent
            case "high", "major": .high
            case "low", "lowest", "minor", "trivial": .low
            default: .normal
            }
        }
    }

    struct Comment: Decodable {
        var id: String
        var author: Person?
        var body: Body
        var created: String?

        func item(issue: Issue, host: URL, accountID: UUID) -> InboxItem {
            let summary = issue.fields.summary ?? issue.key
            return InboxItem(
                id: InboxItem.makeID(account: accountID, local: "comment/\(id)"),
                accountID: accountID,
                pluginID: JiraPlugin.manifest.id,
                bundle: .mentions,
                title: Readable.text(summary),
                context: issue.key,
                preview: body.text.isEmpty ? nil : Readable.text(body.text),
                url: issue.url(host: host).appending(queryItems: [URLQueryItem(name: "focusedCommentId", value: id)]),
                author: author?.person,
                date: JiraPlugin.jiraDate(created) ?? .now,
                needsAction: true
            )
        }
    }

    /// A comment's body: Atlassian Document Format on Cloud (`api/3`), wiki markup on Data Center (`api/2`).
    enum Body: Decodable {
        case document(Node)
        case wiki(String)

        init(from decoder: Decoder) throws {
            let container = try decoder.singleValueContainer()
            if let text = try? container.decode(String.self) {
                self = .wiki(text)
            } else {
                self = .document(try container.decode(Node.self))
            }
        }

        struct Node: Decodable {
            struct Attributes: Decodable {
                var id: String?
                var text: String?
            }
            var type: String?
            var text: String?
            var attrs: Attributes?
            var content: [Node]?

            var mentionIDs: [String] {
                (type == "mention" ? [attrs?.id].compactMap { $0 } : []) + (content ?? []).flatMap(\.mentionIDs)
            }

            var plainText: String {
                if type == "mention" { return attrs?.text ?? "" }
                if let text { return text }
                let inner = (content ?? []).map(\.plainText).joined()
                return type == "paragraph" ? inner + "\n" : inner
            }
        }

        func mentions(_ me: Myself) -> Bool {
            switch self {
            case .document(let node):
                guard let id = me.accountId else { return false }
                return node.mentionIDs.contains(id)
            case .wiki(let text):
                let names = [me.accountId.map { "accountid:\($0)" }, me.name, me.key].compactMap { $0 }
                return names.contains { text.contains("[~\($0)]") }
            }
        }

        var text: String {
            switch self {
            case .document(let node): node.plainText.trimmingCharacters(in: .whitespacesAndNewlines)
            case .wiki(let text):
                // [~alice] or [~accountid:…] → @alice
                text.replacing(/\[~(?:accountid:)?([^\]]+)\]/) { "@" + $0.output.1 }
                    .trimmingCharacters(in: .whitespacesAndNewlines)
            }
        }
    }
}

// MARK: - Confluence

extension JiraPlugin {
    /// Confluence's search (`wiki/rest/api/search`): pages and comments, each with its content.
    struct ConfluencePage: Decodable {
        var results: [ConfluenceResult] = []
    }

    struct ConfluenceResult: Decodable {
        struct Content: Decodable {
            struct History: Decodable {
                struct Version: Decodable {
                    struct By: Decodable {
                        struct Picture: Decodable { var path: String? }
                        var accountId: String?
                        var displayName: String?
                        var profilePicture: Picture?
                    }
                    var by: By?
                }
                var lastUpdated: Version?
            }
            /// A comment's page.
            struct Container: Decodable { var title: String? }
            struct Links: Decodable { var webui: String? }

            var id: String
            var type: String?
            var title: String?
            var history: History?
            var container: Container?
            var links: Links?

            enum CodingKeys: String, CodingKey {
                case id, type, title, history, container
                case links = "_links"
            }
        }
        struct Space: Decodable { var title: String? }

        var content: Content?
        var excerpt: String?
        var lastModified: String?
        var resultGlobalContainer: Space?

        /// A mention, the page's title first: the verb classifier reads it with the excerpt.
        func item(site: String, accountID: UUID) -> InboxItem? {
            guard let content, let link = content.links?.webui else { return nil }
            let title = (content.type == "comment" ? content.container?.title : nil) ?? content.title ?? ""
            let excerpt = JiraPlugin.excerpt(excerpt ?? "")
            let by = content.history?.lastUpdated?.by
            // A profile picture's path is on the site: `/wiki/aa-avatar/…`.
            let avatar = by?.profilePicture?.path.map { $0.hasPrefix("/") ? site + $0 : $0 }
            return InboxItem(
                id: InboxItem.makeID(account: accountID, local: "confluence/\(content.id)"),
                accountID: accountID,
                pluginID: JiraPlugin.manifest.id,
                bundle: .mentions,
                title: Readable.text(title),
                context: resultGlobalContainer?.title ?? "Confluence",
                preview: excerpt.isEmpty ? nil : excerpt,
                url: URL(string: site + "/wiki" + link),
                author: by?.displayName.map { RemoraCore.Person(name: $0, avatarURL: URL(lenient: avatar)) },
                date: lastModified.flatMap { Date(iso8601: $0) } ?? .now,
                needsAction: true
            )
        }
    }

    /// A search excerpt as text: without its `@@@hl@@@` highlight marks and HTML.
    static func excerpt(_ raw: String) -> String {
        MicrosoftPlugin.plain(raw.replacing("@@@hl@@@", with: "").replacing("@@@endhl@@@", with: ""))
    }
}

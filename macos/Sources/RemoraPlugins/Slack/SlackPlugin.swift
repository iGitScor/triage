import Foundation
import RemoraCore

/// Recent mentions and direct messages, found with Slack's search API.
public struct SlackPlugin: SourcePlugin {
    public static let manifest = PluginManifest(
        id: "slack",
        name: "Slack",
        symbol: "number",
        summary: "Mentions and direct messages from the last few days.",
        fields: [
            .token("User OAuth token", help: "A user token (xoxp-…) with the search:read scope."),
            ConfigField(key: "days", label: "Look back (days)", placeholder: "3", defaultValue: "3"),
        ],
        setupSteps: [
            "Click “Create the Slack app”, pick your workspace, then Next and Create. The app comes pre-filled.",
            "In the app page, open “Install App” and click “Install to <workspace>”, then Allow.",
            "Copy the “User OAuth Token” (it starts with xoxp-) and paste it below.",
        ],
        setupLabel: "Create the Slack app",
        setupURL: { _ in appManifestURL },
        egress: Egress(hosts: ["slack.com"], description: "Searches your recent mentions and direct messages in Slack."),
        logo: "slack"
    )

    /// Opens Slack's "create app" flow pre-filled with the one scope Remora needs.
    static let appManifestURL: URL? = {
        let manifest = #"{"display_information":{"name":"Remora","description":"Mentions and DMs in your menu bar"},"oauth_config":{"scopes":{"user":["search:read"]}},"settings":{"org_deploy_enabled":false,"socket_mode_enabled":false,"token_rotation_enabled":false}}"#
        var components = URLComponents(string: "https://api.slack.com/apps")!
        components.queryItems = [URLQueryItem(name: "new_app", value: "1"), URLQueryItem(name: "manifest_json", value: manifest)]
        return components.url
    }()

    private let accountID: UUID
    private let token: String
    private let days: Int
    private let http: HTTPClient
    private let api = URL(string: "https://slack.com/api")!

    public init(config: PluginConfig, http: HTTPClient) throws {
        accountID = config.accountID
        token = try config.required("token")
        days = max(1, Int(config["days"]) ?? 3)
        self.http = http
    }

    public func fetch() async throws -> SourceSnapshot {
        let auth: Auth = try await call("auth.test")
        let since = Date.now.addingTimeInterval(-Double(days) * 86_400).formatted(.iso8601.year().month().day())
        let me = "<@\(auth.userId)>"
        async let mentions: Search = call("search.messages", ["query": "\(me) -from:\(me) after:\(since)"])
        async let direct: Search = call("search.messages", ["query": "to:me -from:\(me) after:\(since)"])
        return try await Self.snapshot(
            mentions: mentions.messages.matches,
            direct: direct.messages.matches,
            identity: "\(auth.user) · \(auth.team)",
            me: auth.userId,
            teamID: auth.teamId,
            accountID: accountID
        )
    }

    static func snapshot(mentions: [Match], direct: [Match], identity: String, me: String, teamID: String? = nil, accountID: UUID) -> SourceSnapshot {
        let mentionItems = mentions
            .filter { !$0.channel.isDirect }
            .map { $0.item(accountID: accountID, bundle: .mentions, id: "\($0.channel.id)/\($0.ts)", me: me, teamID: teamID) }
        let latestPerConversation = Dictionary(grouping: direct.filter(\.channel.isDirect), by: \.channel.id)
            .compactMap { $0.value.max { $0.date < $1.date } }
            .map { $0.item(accountID: accountID, bundle: .directMessages, id: $0.channel.id, me: me, teamID: teamID) }
        return SourceSnapshot(identity: identity, items: mentionItems + latestPerConversation)
    }

    private func call<T: Decodable>(_ method: String, _ query: [String: String] = [:]) async throws -> T {
        var parameters = query
        if method == "search.messages" {
            parameters.merge(["sort": "timestamp", "sort_dir": "desc", "count": "40"]) { $1 }
        }
        let request = URLRequest.get(api.appending(path: method, query: parameters), headers: ["Authorization": "Bearer \(token)"])
        let envelope = try await http.decode(Envelope<T>.self, from: request, using: .api(snakeCase: true))
        guard envelope.ok, let value = envelope.value else {
            throw envelope.error == "invalid_auth" || envelope.error == "not_authed"
                ? HTTPError.unauthorized
                : HTTPError.api("Slack: \(envelope.error ?? "unknown error")")
        }
        return value
    }
}

// MARK: - Wire format

extension SlackPlugin {
    struct Envelope<T: Decodable>: Decodable {
        var ok: Bool
        var error: String?
        var value: T?

        init(from decoder: Decoder) throws {
            let container = try decoder.container(keyedBy: Key.self)
            ok = try container.decode(Bool.self, forKey: .ok)
            error = try container.decodeIfPresent(String.self, forKey: .error)
            value = ok ? try T(from: decoder) : nil
        }

        enum Key: String, CodingKey { case ok, error }
    }

    struct Auth: Decodable {
        var userId: String
        var user: String
        var team: String
        var teamId: String?
    }

    struct Search: Decodable {
        struct Messages: Decodable { var matches: [Match] }
        var messages: Messages
    }

    struct Match: Decodable {
        struct Channel: Decodable {
            var id: String
            var name: String?
            var isIm: Bool?
            var isMpim: Bool?

            var isDirect: Bool { isIm == true || isMpim == true }
            var label: String {
                if isIm == true { return L("Direct message") }
                if isMpim == true { return L("Group message") }
                return "#\(name ?? id)"
            }
        }

        var ts: String
        var text: String
        var permalink: String?
        var username: String?
        var channel: Channel

        var date: Date { Date(timeIntervalSince1970: Double(ts) ?? 0) }

        /// Slack documents app links for channels and DMs, not for single messages: the app opens the
        /// conversation, the web permalink (⌥-click) the exact message.
        func appURL(teamID: String?) -> URL? {
            guard let teamID else { return nil }
            var components = URLComponents(string: "slack://channel")!
            components.queryItems = [URLQueryItem(name: "team", value: teamID), URLQueryItem(name: "id", value: channel.id)]
            return components.url
        }

        func item(accountID: UUID, bundle: InboxBundle, id: String, me: String, teamID: String? = nil) -> InboxItem {
            let lines = SlackText.plain(text, me: me).split(separator: "\n", omittingEmptySubsequences: true).map(String.init)
            return InboxItem(
                id: "\(accountID.uuidString)/\(id)",
                accountID: accountID,
                pluginID: SlackPlugin.manifest.id,
                bundle: bundle,
                title: lines.first ?? L("(no text)"),
                context: channel.label,
                preview: lines.dropFirst().joined(separator: " ").nilIfEmpty,
                url: URL(lenient: permalink),
                author: username.map { Person(name: $0) },
                date: date,
                needsAction: true,
                appURL: appURL(teamID: teamID)
            )
        }
    }
}

/// Turns Slack mrkdwn (`<@U1|alice>`, `<https://x|label>`, `&amp;`) into readable text.
enum SlackText {
    static func plain(_ text: String, me: String) -> String {
        var result = text.replacingOccurrences(of: "<@\(me)>", with: "@you")
        let replacements: [(String, String)] = [
            (#"<@[A-Z0-9]+\|([^>]+)>"#, "@$1"),
            (#"<@([A-Z0-9]+)>"#, "@someone"),
            (#"<#[A-Z0-9]+\|([^>]*)>"#, "#$1"),
            (#"<!(here|channel|everyone)[^>]*>"#, "@$1"),
            (#"<(https?://[^|>]+)\|([^>]+)>"#, "$2"),
            (#"<(https?://[^>]+)>"#, "$1"),
        ]
        for (pattern, template) in replacements {
            result = result.replacingOccurrences(of: pattern, with: template, options: .regularExpression)
        }
        return result
            .replacingOccurrences(of: "&lt;", with: "<")
            .replacingOccurrences(of: "&gt;", with: ">")
            .replacingOccurrences(of: "&amp;", with: "&")
    }
}

extension String {
    var nilIfEmpty: String? { isEmpty ? nil : self }
}

import Foundation
import RemoraCore

/// Recent mentions and direct messages, found with Slack's search API.
public struct SlackPlugin: SourcePlugin {
    public static let manifest = PluginManifest(
        id: "slack",
        name: "Slack",
        symbol: "number",
        summary: "Mentions, direct messages and replies in your threads from the last few days.",
        fields: [
            .token(
                "User OAuth token",
                help:
                    "A user token (xoxp-…) with search:read, plus channels:history and groups:history for thread replies."
            ),
            ConfigField(key: "days", label: "Look back (days)", placeholder: "3", defaultValue: "3"),
        ],
        setupSteps: [
            "Click “Create the Slack app”, pick your workspace, then Next and Create. The app comes pre-filled.",
            "In the app page, open “Install App” and click “Install to <workspace>”, then Allow.",
            "Copy the “User OAuth Token” (it starts with xoxp-) and paste it below.",
        ],
        setupLabel: "Create the Slack app",
        setupURL: { _ in appManifestURL },
        egress: Egress(
            hosts: ["slack.com"],
            description:
                "Searches your recent mentions and direct messages in Slack, and reads the threads you wrote in."),
        logo: "slack"
    )

    /// Opens Slack's "create app" flow pre-filled with the read-only scopes Remora needs: search, and the channels'
    /// history for replies in your threads.
    static let appManifestURL: URL? = {
        let manifest =
            #"{"display_information":{"name":"Remora","description":"Mentions and DMs in your menu bar"},"oauth_config":{"scopes":{"user":["search:read","channels:history","groups:history"]}},"settings":{"org_deploy_enabled":false,"socket_mode_enabled":false,"token_rotation_enabled":false}}"#
        var components = URLComponents(string: "https://api.slack.com/apps")!
        components.queryItems = [
            URLQueryItem(name: "new_app", value: "1"), URLQueryItem(name: "manifest_json", value: manifest),
        ]
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
        async let mine: Search? = try? call("search.messages", ["query": "from:\(me) is:thread after:\(since)"])
        var snapshot = try await Self.snapshot(
            mentions: mentions.messages.matches,
            direct: direct.messages.matches,
            identity: "\(auth.user) · \(auth.team)",
            me: auth.userId,
            teamID: auth.teamId,
            accountID: accountID
        )
        let threads = await threadReplies(
            mine: await mine?.messages.matches ?? [], me: auth.userId, teamID: auth.teamId)
        snapshot.items += threads.items
        snapshot.remarks += threads.remarks
        return snapshot
    }

    /// Replies in the threads you wrote in lately, posted after your last message there: what search
    /// can't find, since nobody mentioned you. The 10 most recent channel threads, one `conversations.replies` each;
    /// replies that mention you are already mentions, and direct messages are already there. Without the history
    /// scopes, a remark says how to add them.
    func threadReplies(mine: [Match], me: String, teamID: String?) async -> (items: [InboxItem], remarks: [String]) {
        var latest: [String: Match] = [:]
        for message in mine where !message.channel.isDirect {
            let key = "\(message.channel.id)/\(message.threadTS)"
            if (latest[key]?.date ?? .distantPast) < message.date { latest[key] = message }
        }
        let threads = latest.values.sorted { $0.date > $1.date }.prefix(10)
        var items: [InboxItem] = []
        for mineLast in threads {
            let replies: Replies
            do {
                replies = try await call(
                    "conversations.replies",
                    [
                        "channel": mineLast.channel.id, "ts": mineLast.threadTS, "oldest": mineLast.ts, "limit": "50",
                    ])
            } catch HTTPError.api(let message) where message.hasSuffix("missing_scope") {
                return (
                    items,
                    [
                        L(
                            "Slack: replies in your threads need the channels:history and groups:history permissions. Add them to the Remora app in Slack, reinstall it, and paste the new token."
                        )
                    ]
                )
            } catch {
                continue
            }
            let others = replies.messages.filter {
                $0.ts != mineLast.ts && $0.user != me && $0.botId == nil && !$0.text.contains("<@\(me)>")
            }
            guard let reply = others.max(by: { $0.date < $1.date }) else { continue }
            items.append(reply.item(in: mineLast, accountID: accountID, me: me, teamID: teamID))
        }
        return (items, [])
    }

    static func snapshot(
        mentions: [Match], direct: [Match], identity: String, me: String, teamID: String? = nil, accountID: UUID
    ) -> SourceSnapshot {
        let mentionItems =
            mentions
            .filter { !$0.channel.isDirect }
            .map {
                $0.item(
                    accountID: accountID, bundle: .mentions, id: "\($0.channel.id)/\($0.ts)", me: me, teamID: teamID)
            }
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
        let request = URLRequest.get(
            api.appending(path: method, query: parameters), headers: ["Authorization": "Bearer \(token)"])
        let envelope = try await http.decode(Envelope<T>.self, from: request, using: .api(snakeCase: true))
        guard envelope.ok, let value = envelope.value else {
            // A token that no longer works, whatever Slack calls it: the user reconnects.
            throw ["invalid_auth", "not_authed", "token_revoked", "token_expired", "account_inactive"].contains(
                envelope.error ?? "")
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

    /// A thread's messages from `conversations.replies`, the root first.
    struct Replies: Decodable {
        struct Message: Decodable {
            struct Profile: Decodable {
                var displayName: String?
                var realName: String?
            }
            var ts: String
            var text: String = ""
            var user: String?
            var botId: String?
            var userProfile: Profile?

            enum CodingKeys: String, CodingKey { case ts, text, user, botId, userProfile }

            init(from decoder: Decoder) throws {
                let container = try decoder.container(keyedBy: CodingKeys.self)
                ts = try container.decode(String.self, forKey: .ts)
                text = try container.decodeIfPresent(String.self, forKey: .text) ?? ""
                user = try container.decodeIfPresent(String.self, forKey: .user)
                botId = try container.decodeIfPresent(String.self, forKey: .botId)
                userProfile = try? container.decodeIfPresent(Profile.self, forKey: .userProfile)
            }

            var date: Date { Date(timeIntervalSince1970: Double(ts) ?? 0) }

            /// "Thread in #design": one item per thread, its latest reply, opening the thread where you wrote.
            func item(in thread: Match, accountID: UUID, me: String, teamID: String?) -> InboxItem {
                let lines = SlackText.plain(text, me: me).split(separator: "\n", omittingEmptySubsequences: true)
                    .map { $0.trimmingCharacters(in: .whitespaces) }.filter { !$0.isEmpty }
                let name = userProfile?.displayName?.nilIfEmpty ?? userProfile?.realName?.nilIfEmpty
                return InboxItem(
                    id: "\(accountID.uuidString)/\(thread.channel.id)/thread/\(thread.threadTS)",
                    accountID: accountID,
                    pluginID: SlackPlugin.manifest.id,
                    bundle: .mentions,
                    title: lines.first ?? L("(no text)"),
                    context: L("Thread in %@", thread.channel.label),
                    preview: lines.dropFirst().joined(separator: " ").nilIfEmpty,
                    url: URL(lenient: thread.permalink),
                    author: Person(name: name ?? L("Someone")),
                    date: date,
                    needsAction: true,
                    appURL: thread.appURL(teamID: teamID)
                )
            }
        }
        var messages: [Message]
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

        /// What apps put in a message besides (or instead of) `text`.
        struct Attachment: Decodable {
            var fallback: String?
            var pretext: String?
            var title: String?
            var text: String?

            var words: String {
                let parts = [pretext, title, text].compactMap { $0?.trimmingCharacters(in: .whitespacesAndNewlines) }
                    .filter { !$0.isEmpty }
                return parts.isEmpty ? (fallback ?? "") : parts.joined(separator: "\n")
            }
        }

        var ts: String
        var text: String
        var permalink: String?
        var username: String?
        var channel: Channel
        var botId: String?
        var subtype: String?
        var attachments: [Attachment]?
        var blocks: TextLeaves?

        init(from decoder: Decoder) throws {
            let container = try decoder.container(keyedBy: CodingKeys.self)
            ts = try container.decode(String.self, forKey: .ts)
            text = try container.decodeIfPresent(String.self, forKey: .text) ?? ""
            permalink = try container.decodeIfPresent(String.self, forKey: .permalink)
            username = try container.decodeIfPresent(String.self, forKey: .username)
            channel = try container.decode(Channel.self, forKey: .channel)
            botId = try container.decodeIfPresent(String.self, forKey: .botId)
            subtype = try container.decodeIfPresent(String.self, forKey: .subtype)
            attachments = try? container.decodeIfPresent([Attachment].self, forKey: .attachments)
            blocks = try? container.decodeIfPresent(TextLeaves.self, forKey: .blocks)
        }

        enum CodingKeys: String, CodingKey {
            case ts, text, permalink, username, channel, botId, subtype, attachments, blocks
        }

        /// Apps (Google Calendar, Jira, GitHub…) post with a bot id, or with an empty `text` and their content in
        /// attachments or blocks. They inform; nobody waits for a reply.
        var isFromApp: Bool {
            botId != nil || subtype == "bot_message" || text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        }

        /// The message's words: its text, else what its attachments or blocks say.
        var words: String {
            if !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty { return text }
            let fromAttachments = (attachments ?? []).map(\.words).filter { !$0.isEmpty }.joined(separator: "\n")
            return fromAttachments.isEmpty ? (blocks?.strings ?? []).joined(separator: "\n") : fromAttachments
        }

        var date: Date { Date(timeIntervalSince1970: Double(ts) ?? 0) }

        /// The thread this message is in: its permalink carries `thread_ts` for a reply; a thread's first message
        /// is its own root.
        var threadTS: String {
            permalink.flatMap { URLComponents(string: $0)?.queryItems?.first { $0.name == "thread_ts" }?.value } ?? ts
        }

        /// Slack documents app links for channels and DMs, not for single messages: the app opens the
        /// conversation, the web permalink (⌥-click) the exact message.
        func appURL(teamID: String?) -> URL? {
            guard let teamID else { return nil }
            var components = URLComponents(string: "slack://channel")!
            components.queryItems = [
                URLQueryItem(name: "team", value: teamID), URLQueryItem(name: "id", value: channel.id),
            ]
            return components.url
        }

        func item(accountID: UUID, bundle: InboxBundle, id: String, me: String, teamID: String? = nil) -> InboxItem {
            let lines = SlackText.plain(words, me: me).split(separator: "\n", omittingEmptySubsequences: true)
                .map { $0.trimmingCharacters(in: .whitespaces) }.filter { !$0.isEmpty }
            var item = InboxItem(
                id: "\(accountID.uuidString)/\(id)",
                accountID: accountID,
                pluginID: SlackPlugin.manifest.id,
                bundle: isFromApp ? .read : bundle,
                title: lines.first ?? L("(no text)"),
                context: channel.label,
                preview: lines.dropFirst().joined(separator: " ").nilIfEmpty,
                url: URL(lenient: permalink),
                author: username.map { Person(name: $0) },
                date: date,
                needsAction: !isFromApp,
                appURL: appURL(teamID: teamID)
            )
            item.expires = expires
            return item
        }

        /// An app's message about a time to come (a calendar reminder) is over once that time comes: the first time
        /// it mentions after it was posted. People's messages never expire.
        var expires: Date? {
            guard isFromApp else { return nil }
            return SlackText.times(in: words).filter { $0 > date }.min()
        }
    }
}

/// Every string under a "text" key, at any depth: the words of Slack's Block Kit blocks (section, header,
/// context, rich text) without modelling each block type.
struct TextLeaves: Decodable {
    var strings: [String] = []

    private struct Key: CodingKey {
        var stringValue: String
        var intValue: Int? { nil }
        init(stringValue: String) { self.stringValue = stringValue }
        init?(intValue: Int) { nil }
    }

    init(from decoder: Decoder) throws {
        if let object = try? decoder.container(keyedBy: Key.self) {
            for key in object.allKeys.sorted(by: { $0.stringValue < $1.stringValue }) {
                if key.stringValue == "text", let string = try? object.decode(String.self, forKey: key) {
                    strings.append(string)
                } else if let nested = try? object.decode(TextLeaves.self, forKey: key) {
                    strings += nested.strings
                }
            }
        } else if var array = try? decoder.unkeyedContainer() {
            while !array.isAtEnd {
                guard let nested = try? array.decode(TextLeaves.self) else {
                    _ = try? array.decode(Skip.self)
                    continue
                }
                strings += nested.strings
            }
        }
    }

    /// Consumes one array element of any shape.
    private struct Skip: Decodable { init(from decoder: Decoder) throws {} }
}

/// How Slack's date tags read: in the Mac's language and time zone.
struct SlackDateStyle {
    var locale: Locale
    var timeZone: TimeZone
    /// What `{ago}` counts from.
    var now: Date

    static var current: SlackDateStyle { SlackDateStyle(locale: .current, timeZone: .current, now: .now) }
}

/// Turns Slack mrkdwn into readable text: people, channels, links, emails and phone numbers by their label, dates in
/// your language and time zone (`<!date^1791555300^{time}|4:15 PM>` → 16:15), then `Readable` (emoji, no marks).
/// Same rules on Windows (`slack.rs`).
enum SlackText {
    static func plain(_ text: String, me: String, style: SlackDateStyle = .current) -> String {
        var result = dates(text.replacingOccurrences(of: "<@\(me)>", with: "@you"), style: style)
        let replacements: [(String, String)] = [
            (#"<@[A-Z0-9]+\|([^>]+)>"#, "@$1"),
            (#"<@([A-Z0-9]+)>"#, "@someone"),
            (#"<#[A-Z0-9]+\|([^>]*)>"#, "#$1"),
            (#"<!(here|channel|everyone)[^>]*>"#, "@$1"),
            (#"<!subteam\^[A-Z0-9]+\|([^>]+)>"#, "$1"),
            (#"<!subteam\^[A-Z0-9]+>"#, "@team"),
            (#"<!date\^[^>]*>"#, ""),
            (#"<(?:https?|mailto|tel):[^|>]+\|([^>]+)>"#, "$1"),
            (#"<(https?://[^>]+)>"#, "$1"),
            (#"<(?:mailto|tel):([^>]+)>"#, "$1"),
        ]
        for (pattern, template) in replacements {
            result = result.replacingOccurrences(of: pattern, with: template, options: .regularExpression)
        }
        return Readable.text(
            result
                .replacingOccurrences(of: "&lt;", with: "<")
                .replacingOccurrences(of: "&gt;", with: ">")
                .replacingOccurrences(of: "&amp;", with: "&"))
    }

    /// `<!date^seconds^format|fallback>` and `<!date^seconds^format^link|fallback>`.
    static let dateTag = try! NSRegularExpression(pattern: #"<!date\^(\d+)\^([^^|>]*)(?:\^[^|>]*)?(?:\|([^>]*))?>"#)

    /// The times a message's date tags point at.
    static func times(in text: String) -> [Date] {
        dateTag.matches(in: text, range: NSRange(text.startIndex..., in: text)).compactMap { match in
            Range(match.range(at: 1), in: text).flatMap { Double(text[$0]) }.map(Date.init(timeIntervalSince1970:))
        }
    }

    private static func dates(_ text: String, style: SlackDateStyle) -> String {
        var result = ""
        var last = text.startIndex
        for match in dateTag.matches(in: text, range: NSRange(text.startIndex..., in: text)) {
            guard let whole = Range(match.range, in: text), let seconds = Range(match.range(at: 1), in: text),
                let format = Range(match.range(at: 2), in: text), let time = Double(text[seconds])
            else { continue }
            let fallback = Range(match.range(at: 3), in: text).map { String(text[$0]) }
            result += text[last..<whole.lowerBound]
            result += formatted(
                Date(timeIntervalSince1970: time), String(text[format]), fallback: fallback, style: style)
            last = whole.upperBound
        }
        return result + text[last...]
    }

    /// Slack's tokens ({time}, {date_short}, {date_pretty}, {ago}…) in place; an unknown one gives the fallback.
    static func formatted(_ date: Date, _ format: String, fallback: String?, style: SlackDateStyle) -> String {
        let tokens = try! NSRegularExpression(pattern: #"\{([a-z_]+)\}"#)
        var result = ""
        var last = format.startIndex
        for match in tokens.matches(in: format, range: NSRange(format.startIndex..., in: format)) {
            guard let whole = Range(match.range, in: format), let name = Range(match.range(at: 1), in: format) else {
                continue
            }
            guard let value = token(String(format[name]), date, style) else { return fallback ?? "" }
            result += format[last..<whole.lowerBound] + value
            last = whole.upperBound
        }
        return result + format[last...]
    }

    private static func token(_ name: String, _ date: Date, _ style: SlackDateStyle) -> String? {
        let formatter = DateFormatter()
        formatter.locale = style.locale
        formatter.timeZone = style.timeZone
        switch name {
        case "time": formatter.timeStyle = .short
        case "time_secs": formatter.timeStyle = .medium
        case "date_num":
            formatter.dateFormat = "yyyy-MM-dd"
        case "date", "date_pretty": formatter.dateStyle = .long
        case "date_short", "date_short_pretty": formatter.dateStyle = .medium
        case "date_long", "date_long_pretty": formatter.dateStyle = .full
        case "ago":
            let relative = RelativeDateTimeFormatter()
            relative.locale = style.locale
            return relative.localizedString(for: date, relativeTo: style.now)
        default: return nil
        }
        // "Today", "Yesterday": the formatter only words them against the real clock, so the day is counted from
        // `style.now` and the word taken for that many days from today.
        if name.hasSuffix("_pretty") {
            var calendar = Calendar(identifier: .gregorian)
            calendar.timeZone = style.timeZone
            let days =
                calendar.dateComponents(
                    [.day], from: calendar.startOfDay(for: style.now), to: calendar.startOfDay(for: date)
                ).day ?? 0
            if (-1...1).contains(days), let day = calendar.date(byAdding: .day, value: days, to: .now) {
                formatter.doesRelativeDateFormatting = true
                return formatter.string(from: day)
            }
        }
        return formatter.string(from: date)
    }
}

extension String {
    var nilIfEmpty: String? { isEmpty ? nil : self }
}

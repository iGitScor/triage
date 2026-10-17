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
                    "A user token (xoxp-…) with search:read, plus channels:history and groups:history for thread replies, and reactions:read and emoji:read for reactions."
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

    /// Opens Slack's "create app" flow pre-filled with the read-only scopes Remora needs: search, the channels'
    /// history for replies in your threads, and reactions.
    static let appManifestURL: URL? = {
        let manifest =
            #"{"display_information":{"name":"Remora","description":"Mentions and DMs in your menu bar"},"oauth_config":{"scopes":{"user":["search:read","channels:history","groups:history","reactions:read","emoji:read"]}},"settings":{"org_deploy_enabled":false,"socket_mode_enabled":false,"token_rotation_enabled":false}}"#
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
    private let cache: any AccountCache
    private let api = URL(string: "https://slack.com/api")!

    public init(config: PluginConfig, http: HTTPClient) throws {
        accountID = config.accountID
        token = try config.required("token")
        days = max(1, Int(config["days"]) ?? 3)
        self.http = http
        cache = config.cache
    }

    public func fetch() async throws -> SourceSnapshot {
        let auth: Auth = try await cache.value("auth.test", lifetime: CacheLifetime.identity) {
            try await call("auth.test")
        }
        let since = Date.now.addingTimeInterval(-Double(days) * .day).formatted(.iso8601.year().month().day())
        let me = "<@\(auth.userId)>"
        async let mentions: Search = call("search.messages", ["query": "\(me) -from:\(me) after:\(since)"])
        async let direct: Search = call("search.messages", ["query": "to:me -from:\(me) after:\(since)"])
        // Your own messages decide whose turn it is and which threads to read: best-effort, but said when missing.
        async let mine = BestEffort.attempt {
            try await call("search.messages", ["query": "from:\(me) is:thread after:\(since)"]) as Search
        }
        // Your own recent messages: an answer of yours ends a conversation's turn.
        async let sent = BestEffort.attempt {
            try await call("search.messages", ["query": "from:\(me) after:\(since)"]) as Search
        }
        // The workspace's own emoji first (kept a day): an alias of ✅ must count as ✅ wherever reactions decide.
        let custom = await customEmoji()
        async let yours = reactedByYou(me: auth.userId, custom: custom)
        let mentioned = try await mentions.messages.matches
        let written = try await direct.messages.matches
        var partial: [String] = []
        let ownSent = try BestEffort.unwrap(await sent, "Your recent messages", remarks: &partial)
        let ownThreads = try BestEffort.unwrap(await mine, "Your recent messages", remarks: &partial)
        let heads = Self.conversations(
            mentions: mentioned, direct: written, sent: ownSent?.messages.matches ?? [], me: auth.userId)
        let (byYou, reactionsRemark) = await yours
        // Search results carry no reactions: ask for those of the newest messages that matter, unless the token
        // can't read reactions at all.
        let shown =
            reactionsRemark == nil
            ? await reactions(on: Self.needingYou(heads), me: auth.userId, custom: custom) : [:]
        var snapshot = Self.snapshot(
            heads: heads,
            identity: "\(auth.user) · \(auth.team)",
            me: auth.userId,
            teamID: auth.teamId,
            accountID: accountID,
            reactions: byYou.merging(shown) { _, asked in asked }
        )
        let threads = try await threadReplies(
            mine: ownThreads?.messages.matches ?? [], me: auth.userId, teamID: auth.teamId, custom: custom)
        snapshot.items += threads.items
        snapshot.remarks += threads.remarks + [reactionsRemark].compactMap { $0 } + partial
        return snapshot
    }

    /// The workspace's own emoji, by name: `alias:white_check_mark` for another name of a standard one, else the
    /// picture's address. Asked once a day; without the emoji:read permission there are none, and reactions with
    /// a workspace emoji show as `:name:`, their meaning taken from the name.
    func customEmoji() async -> [String: String] {
        let lifetime: TimeInterval = .day
        return
            (try? await cache.value("emoji.list", lifetime: lifetime) {
                let list: CustomEmoji? = try? await call("emoji.list")
                return list?.emoji ?? [:]
            }) ?? [:]
    }

    /// A reaction with a workspace emoji: an alias becomes the emoji it stands for, another gets its picture.
    static func resolve(_ reaction: Reaction, custom: [String: String]) -> Reaction {
        var name = reaction.name
        for _ in 0..<3 {
            guard let value = custom[name], value.hasPrefix("alias:") else { break }
            name = String(value.dropFirst("alias:".count))
        }
        let picture = custom[name].flatMap { $0.hasPrefix("alias:") ? nil : URL(string: $0) }
        return Reaction(name: name, count: reaction.count, mine: reaction.mine, image: picture)
    }

    /// How many messages that need you get their reactions asked for, each kept 10 minutes: Slack's search doesn't
    /// return them, and one request per message adds up.
    static let reactionsAsked = 15
    static let reactionsLifetime: TimeInterval = 600

    /// The messages you reacted to lately, by `channel/ts`, with everyone's reactions: one request for every item.
    /// Without the reactions:read permission, a remark says how to add it.
    func reactedByYou(me: String, custom: [String: String] = [:]) async -> (
        reactions: [String: [Reaction]], remark: String?
    ) {
        do {
            let reacted: Reacted = try await call("reactions.list", ["count": "100", "full": "true"])
            var byMessage: [String: [Reaction]] = [:]
            for item in reacted.items {
                guard let channel = item.channel, let message = item.message, let ts = message.ts,
                    let reactions = Self.reactions(message.reactions, me: me, custom: custom)
                else { continue }
                byMessage["\(channel)/\(ts)"] = reactions
            }
            return (byMessage, nil)
        } catch HTTPError.api(let message) where message.hasSuffix("missing_scope") {
            return (
                [:],
                L(
                    "Slack: reactions need the reactions:read permission. Add it to the Remora app in Slack, reinstall it, and paste the new token."
                )
            )
        } catch {
            return ([:], nil)
        }
    }

    /// The reactions on these messages, by `channel/ts`. A message whose reactions can't be read just has none.
    func reactions(on messages: [Match], me: String, custom: [String: String] = [:]) async -> [String: [Reaction]] {
        await withTaskGroup(of: (String, [Reaction]?).self) { group in
            for message in messages {
                let key = "\(message.channel.id)/\(message.ts)"
                group.addTask {
                    let got: Got? = try? await cache.value("reactions.get \(key)", lifetime: Self.reactionsLifetime) {
                        try await call(
                            "reactions.get", ["channel": message.channel.id, "timestamp": message.ts, "full": "true"])
                    }
                    return (key, Self.reactions(got?.message?.reactions, me: me, custom: custom))
                }
            }
            var byMessage: [String: [Reaction]] = [:]
            for await (key, reactions) in group {
                if let reactions { byMessage[key] = reactions }
            }
            return byMessage
        }
    }

    /// The newest messages that become items, theirs or your answers. Apps' messages aren't asked about.
    static func needingYou(_ heads: [Head]) -> [Match] {
        heads.map(\.message).filter { !$0.isFromApp }.sorted { $0.date > $1.date }.prefix(reactionsAsked).map { $0 }
    }

    /// The message a conversation's item shows: theirs when they spoke last, yours when you did.
    struct Head {
        var message: Match
        var bundle: InboxBundle
        var id: String
    }

    /// How long after a mention a message of yours in the same channel counts as the answer, when it doesn't name
    /// the person: in a busy channel, a later message of yours may be about something else.
    static let answerWindow: TimeInterval = 2 * 3_600

    /// Whose turn each conversation is. A direct conversation is theirs to answer when they wrote last, else yours
    /// waits (`answered`, or `asked` when they never wrote). A mention is answered by your later message in its
    /// thread, which the thread's own item then follows (`finished` here), or, outside a thread, by your later
    /// message in the channel that names them or comes within `answerWindow`.
    static func conversations(mentions: [Match], direct: [Match], sent: [Match], me: String) -> [Head] {
        let yours = sent.filter { $0.user == nil || $0.user == me }
        var heads: [Head] = []
        for mention in mentions where !mention.channel.isDirect {
            let id = "\(mention.channel.id)/\(mention.ts)"
            let later = yours.filter { $0.channel.id == mention.channel.id && $0.date > mention.date }
            let inItsThread = later.contains {
                $0.threadTS != $0.ts && ($0.threadTS == mention.threadTS || $0.threadTS == mention.ts)
            }
            if inItsThread {
                heads.append(Head(message: mention, bundle: .finished, id: id))
                continue
            }
            let answer = later.filter { message in
                message.threadTS == message.ts
                    && (mention.user.map { message.text.contains("<@\($0)>") } == true
                        || message.date.timeIntervalSince(mention.date) <= answerWindow)
            }.max { $0.date < $1.date }
            heads.append(
                answer.map { Head(message: $0, bundle: .answered, id: id) }
                    ?? Head(message: mention, bundle: .mentions, id: id))
        }
        let theirs = Dictionary(uniqueKeysWithValues: latestPerConversation(direct).map { ($0.channel.id, $0) })
        let ours = Dictionary(uniqueKeysWithValues: latestPerConversation(yours).map { ($0.channel.id, $0) })
        for channel in Set(theirs.keys).union(ours.keys).sorted() {
            if let mine = ours[channel], mine.date > (theirs[channel]?.date ?? .distantPast) {
                heads.append(Head(message: mine, bundle: theirs[channel] == nil ? .asked : .answered, id: channel))
            } else if let message = theirs[channel] {
                heads.append(Head(message: message, bundle: .directMessages, id: channel))
            }
        }
        return heads
    }

    static func latestPerConversation(_ direct: [Match]) -> [Match] {
        Dictionary(grouping: direct.filter(\.channel.isDirect), by: \.channel.id).compactMap {
            $0.value.max { $0.date < $1.date }
        }
    }

    /// Slack's reactions as items carry them, one per emoji; nil when there are none.
    static func reactions(_ wire: [WireReaction]?, me: String, custom: [String: String] = [:]) -> [Reaction]? {
        guard let wire, !wire.isEmpty else { return nil }
        return Reaction.merged(
            wire.map {
                resolve(
                    Reaction(
                        name: $0.name, count: $0.count ?? $0.users?.count ?? 0, mine: $0.users?.contains(me) == true),
                    custom: custom)
            }
        )
    }

    /// The threads you wrote in lately, as of your last message there: what search can't find, since nobody
    /// mentioned you. The 10 most recent channel threads, one `conversations.replies` each. A newer reply is to
    /// read or answer (one that mentions you is already a mention); without one, their reactions on your message
    /// speak: ❓ is a question for you, ✅ an answer, and otherwise your message waits (if it asked something, as
    /// `VerbClassifier` decides). Without the history scopes, a remark says how to add them.
    func threadReplies(mine: [Match], me: String, teamID: String?, custom: [String: String] = [:]) async throws -> (
        items: [InboxItem], remarks: [String]
    ) {
        var latest: [String: Match] = [:]
        for message in mine where !message.channel.isDirect {
            let key = "\(message.channel.id)/\(message.threadTS)"
            if (latest[key]?.date ?? .distantPast) < message.date { latest[key] = message }
        }
        let threads = latest.values.sorted { $0.date > $1.date }.prefix(10)
        var items: [InboxItem] = []
        var failed: [String] = []
        for mineLast in threads {
            let replies: Replies
            do {
                replies = try await call(
                    "conversations.replies",
                    [
                        "channel": mineLast.channel.id, "ts": mineLast.threadTS, "oldest": mineLast.ts,
                        "inclusive": "true", "limit": "50",
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
                // A thread that couldn't be read is said, not passed over; a rate limit fails the fetch.
                try BestEffort.note("Replies in your threads", error, remarks: &failed)
                continue
            }
            // Slack always sends the thread's first message too, whatever `oldest` says: only what came after yours.
            let after = replies.messages.filter { $0.date > mineLast.date && $0.user != me && $0.botId == nil }
            let others = after.filter { !$0.text.contains("<@\(me)>") }
            if let reply = others.max(by: { $0.date < $1.date }) {
                var item = reply.item(in: mineLast, accountID: accountID, me: me, teamID: teamID)
                item.reactions = Self.reactions(reply.reactions, me: me, custom: custom)
                items.append(item)
                continue
            }
            guard after.isEmpty else { continue }
            let yours = replies.messages.first { $0.ts == mineLast.ts }
            // Someone else wrote before you (the thread's first message, at least): yours is an answer.
            let answered = replies.messages.contains {
                $0.date < mineLast.date && $0.user != me && $0.user != nil && $0.botId == nil
            }
            items.append(
                Self.unanswered(
                    mineLast, answered: answered, reactions: Self.reactions(yours?.reactions, me: me, custom: custom),
                    accountID: accountID, me: me, teamID: teamID))
        }
        return (items, failed)
    }

    /// Your last message in a thread nobody wrote back in, as the item: `answered` when someone else wrote before
    /// you, else `asked`; their reactions on it are weighed (`weighed`).
    static func unanswered(
        _ yours: Match, answered: Bool = false, reactions: [Reaction]?, accountID: UUID, me: String, teamID: String?
    ) -> InboxItem {
        var item = yours.item(
            accountID: accountID, bundle: .asked, id: "\(yours.channel.id)/thread/\(yours.threadTS)", me: me,
            teamID: teamID)
        item.bundle = answered ? .answered : .asked
        item.expires = nil
        item.context = L("Thread in %@", yours.channel.label)
        item.needsAction = false
        item.reactions = reactions
        return weighed(item, in: yours.channel.label)
    }

    static func snapshot(
        mentions: [Match], direct: [Match], identity: String, me: String, teamID: String? = nil, accountID: UUID,
        reactions: [String: [Reaction]] = [:]
    ) -> SourceSnapshot {
        snapshot(
            heads: conversations(mentions: mentions, direct: direct, sent: [], me: me), identity: identity, me: me,
            teamID: teamID, accountID: accountID, reactions: reactions)
    }

    static func snapshot(
        heads: [Head], identity: String, me: String, teamID: String?, accountID: UUID, reactions: [String: [Reaction]]
    ) -> SourceSnapshot {
        let items = heads.map { head in
            let message = head.message
            var item = message.item(
                accountID: accountID, bundle: head.bundle, id: head.id, me: me, teamID: teamID,
                reactions: reactions["\(message.channel.id)/\(message.ts)"])
            guard [InboxBundle.asked, .answered, .finished].contains(head.bundle) else { return item }
            item.bundle = head.bundle
            item.expires = nil
            item.needsAction = false
            return head.bundle == .finished ? item : weighed(item, in: message.channel.label)
        }
        return SourceSnapshot(identity: identity, items: items)
    }

    /// Your message, last in a conversation: their ✅ finishes it, their ❓ or 🤔 makes it yours to answer, and
    /// otherwise `VerbClassifier` decides whether it waits on them.
    static func weighed(_ item: InboxItem, in label: String) -> InboxItem {
        var item = item
        let theirs = (item.reactions ?? []).filter { $0.others > 0 }
        if theirs.contains(where: { $0.meaning == .done }) {
            item.bundle = .finished
            return item
        }
        let questions = theirs.filter { $0.meaning == .question }
        if !questions.isEmpty {
            item.context = L("%@ on your message in %@", questions.map(\.emoji).joined(separator: " "), label)
            item.bundle = .reply
            item.needsAction = true
        }
        return item
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

    /// A reaction as Slack sends it: on a message from `conversations.replies`, `reactions.list` or `reactions.get`.
    struct WireReaction: Decodable {
        var name: String
        var users: [String]?
        var count: Int?
    }

    /// `reactions.list`: the messages you reacted to, newest first. Files and other kinds have no message.
    struct Reacted: Decodable {
        struct Item: Decodable {
            var channel: String?
            var message: Reactions?

            init(from decoder: Decoder) throws {
                let container = try decoder.container(keyedBy: CodingKeys.self)
                channel = try? container.decodeIfPresent(String.self, forKey: .channel)
                message = try? container.decodeIfPresent(Reactions.self, forKey: .message)
            }

            enum CodingKeys: String, CodingKey { case channel, message }
        }
        var items: [Item]
    }

    /// A message's reactions, from `reactions.list` or `reactions.get`.
    struct Reactions: Decodable {
        var ts: String?
        var reactions: [WireReaction]?
    }

    /// `reactions.get`.
    struct Got: Decodable {
        var message: Reactions?
    }

    /// `emoji.list`: the workspace's own emoji.
    struct CustomEmoji: Decodable {
        var emoji: [String: String]
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
            var reactions: [WireReaction]?

            enum CodingKeys: String, CodingKey { case ts, text, user, botId, userProfile, reactions }

            init(from decoder: Decoder) throws {
                let container = try decoder.container(keyedBy: CodingKeys.self)
                ts = try container.decode(String.self, forKey: .ts)
                text = try container.decodeIfPresent(String.self, forKey: .text) ?? ""
                user = try container.decodeIfPresent(String.self, forKey: .user)
                botId = try container.decodeIfPresent(String.self, forKey: .botId)
                userProfile = try? container.decodeIfPresent(Profile.self, forKey: .userProfile)
                reactions = try? container.decodeIfPresent([WireReaction].self, forKey: .reactions)
            }

            var date: Date { Date(timeIntervalSince1970: Double(ts) ?? 0) }

            /// "Thread in #design": one item per thread, its latest reply, opening the thread where you wrote.
            func item(in thread: Match, accountID: UUID, me: String, teamID: String?) -> InboxItem {
                let lines = SlackText.plain(text, me: me).split(separator: "\n", omittingEmptySubsequences: true)
                    .map { $0.trimmingCharacters(in: .whitespaces) }.filter { !$0.isEmpty }
                let name = userProfile?.displayName?.nilIfEmpty ?? userProfile?.realName?.nilIfEmpty
                return InboxItem(
                    id: InboxItem.makeID(account: accountID, local: "\(thread.channel.id)/thread/\(thread.threadTS)"),
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
        /// Who wrote it, by Slack id.
        var user: String?
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
            user = try? container.decodeIfPresent(String.self, forKey: .user)
            channel = try container.decode(Channel.self, forKey: .channel)
            botId = try container.decodeIfPresent(String.self, forKey: .botId)
            subtype = try container.decodeIfPresent(String.self, forKey: .subtype)
            attachments = try? container.decodeIfPresent([Attachment].self, forKey: .attachments)
            blocks = try? container.decodeIfPresent(TextLeaves.self, forKey: .blocks)
        }

        enum CodingKeys: String, CodingKey {
            case ts, text, permalink, username, user, channel, botId, subtype, attachments, blocks
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

        func item(
            accountID: UUID, bundle: InboxBundle, id: String, me: String, teamID: String? = nil,
            reactions: [Reaction]? = nil
        ) -> InboxItem {
            let lines = SlackText.plain(words, me: me).split(separator: "\n", omittingEmptySubsequences: true)
                .map { $0.trimmingCharacters(in: .whitespaces) }.filter { !$0.isEmpty }
            var item = InboxItem(
                id: InboxItem.makeID(account: accountID, local: "\(id)"),
                accountID: accountID,
                pluginID: SlackPlugin.manifest.id,
                bundle: isFromApp ? .read : bundle,
                title: lines.first ?? L("(no text)"),
                context: channel.label,
                preview: lines.dropFirst().joined(separator: " ").nilIfEmpty,
                url: URL(lenient: permalink),
                author: username.map { Person(name: $0, bot: isFromApp) },
                date: date,
                needsAction: !isFromApp,
                appURL: appURL(teamID: teamID)
            )
            item.expires = expires
            item.reactions = reactions
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
    /// Compiled once, as `dateTag`.
    static let dateTokens = try! NSRegularExpression(pattern: #"\{([a-z_]+)\}"#)

    static func formatted(_ date: Date, _ format: String, fallback: String?, style: SlackDateStyle) -> String {
        var result = ""
        var last = format.startIndex
        for match in dateTokens.matches(in: format, range: NSRange(format.startIndex..., in: format)) {
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

import Foundation
import RemoraCore

/// Outlook mail, Calendar invitations and Teams chats, through Microsoft Graph with a browser sign-in.
public struct MicrosoftPlugin: SourcePlugin {
    /// What Remora asks Microsoft for: read-only, and `offline_access` for the refresh token.
    static let scopes = ["offline_access", "User.Read", "Mail.Read", "Calendars.Read", "Chat.Read"]

    static let signIn = SignIn(
        authorizeURL: "https://login.microsoftonline.com/{tenant}/oauth2/v2.0/authorize",
        tokenURL: "https://login.microsoftonline.com/{tenant}/oauth2/v2.0/token",
        clientIDKey: "clientId",
        scopes: scopes
    )

    public static let manifest = PluginManifest(
        id: "microsoft",
        name: "Microsoft 365",
        symbol: "envelope",
        summary:
            "Preview: flagged mail and unread mail sent to you, invitations you haven’t answered, and unread Teams chats.",
        fields: [
            ConfigField(
                key: "tenant", label: "Tenant", placeholder: "your-company.onmicrosoft.com",
                help: "Your organization’s domain or tenant ID.", managedKey: "MicrosoftTenant"),
            ConfigField(
                key: "clientId", label: "Client ID", placeholder: "00000000-0000-0000-0000-000000000000",
                help: "The ID of the app your IT team registered for Remora in Microsoft Entra.",
                managedKey: "MicrosoftClientID"),
        ],
        setupSteps: [
            "Ask your IT team for the tenant and the client ID of the app they registered for Remora.",
            "Click “Sign in with Microsoft 365” and sign in in your browser.",
        ],
        setupLabel: "",
        egress: Egress(
            hosts: ["login.microsoftonline.com", "graph.microsoft.com"],
            description:
                "Signs in with Microsoft, then reads your flagged and unread mail, unanswered invitations and unread Teams chats from Microsoft Graph."
        ),
        signIn: signIn
    )

    static let graph = URL(string: "https://graph.microsoft.com/v1.0")!

    private let config: PluginConfig
    private let http: HTTPClient

    public init(config: PluginConfig, http: HTTPClient) throws {
        _ = try config.required("tenant")
        _ = try config.required("clientId")
        _ = try config.required("refreshToken")
        self.config = config
        self.http = http
    }

    public func fetch() async throws -> SourceSnapshot {
        let token = try await OAuth.accessToken(Self.signIn, config: config, http: http)
        let me: Me = try await config.cache.value("me", lifetime: CacheLifetime.identity) {
            try await get(token, "me", ["$select": "id,displayName,mail,userPrincipalName"])
        }
        let now = Date.now
        let fields = "id,subject,bodyPreview,receivedDateTime,webLink,importance,from,toRecipients,flag"
        let since = Self.graphTime(now.addingTimeInterval(-7 * .day))
        async let flagged: Page<Message> = get(
            token, "me/messages", ["$filter": "flag/flagStatus eq 'flagged'", "$select": fields, "$top": "25"])
        async let unread: Page<Message> = get(
            token, "me/mailFolders/inbox/messages",
            ["$filter": "isRead eq false and receivedDateTime ge \(since)", "$select": fields, "$top": "25"])
        async let events = BestEffort.attempt { () async throws -> Page<Event> in
            try await get(
                token, "me/calendarView",
                [
                    "startDateTime": Self.graphTime(now),
                    "endDateTime": Self.graphTime(now.addingTimeInterval(14 * .day)),
                    "$select":
                        "id,subject,webLink,isOrganizer,isCancelled,responseRequested,lastModifiedDateTime,start,organizer,responseStatus",
                    "$top": "50",
                ])
        }
        async let chats = BestEffort.attempt { () async throws -> Page<Chat> in
            try await get(
                token, "me/chats",
                [
                    "$expand": "lastMessagePreview", "$orderby": "lastMessagePreview/createdDateTime desc",
                    "$top": "50",
                ])
        }
        // Mail is what the account is for; the calendar and Teams are a bonus (a personal account has no Teams, an
        // organization may not grant every permission): their failure is a remark.
        var remarks: [String] = []
        let invitations = try BestEffort.unwrap(await events, "Calendar invitations", remarks: &remarks) ?? Page()
        let teams = try BestEffort.unwrap(await chats, "Teams chats", remarks: &remarks) ?? Page()
        var snapshot = try await Self.snapshot(
            me: me, flagged: flagged, unread: unread, events: invitations, chats: teams, accountID: config.accountID,
            now: now)
        snapshot.remarks += remarks
        return snapshot
    }

    static func snapshot(
        me: Me, flagged: Page<Message>, unread: Page<Message>, events: Page<Event>, chats: Page<Chat>,
        accountID: UUID, now: Date
    ) -> SourceSnapshot {
        let flaggedIDs = Set(flagged.value.map(\.id))
        let weekAgo = now.addingTimeInterval(-7 * .day)
        let items =
            flagged.value.map { $0.flaggedItem(accountID: accountID) }
            + unread.value.filter { !flaggedIDs.contains($0.id) && $0.isTo(me) }.map {
                $0.unreadItem(accountID: accountID)
            }
            + events.value.filter { $0.awaitsAnswer(now: now) }.map { $0.item(accountID: accountID) }
            + chats.value.compactMap { $0.item(me: me, accountID: accountID, since: weekAgo) }
        let cut = flagged.next != nil || unread.next != nil
        return SourceSnapshot(
            identity: me.address ?? me.displayName, items: items,
            remarks: cut ? [SourceSnapshot.truncated("Microsoft 365")] : [])
    }

    private func get<T: Decodable>(_ token: String, _ path: String, _ query: [String: String]) async throws -> T {
        try await http.decode(
            T.self,
            from: .get(
                Self.graph.appending(path: path, query: query),
                headers: [
                    "Authorization": "Bearer \(token)", "Accept": "application/json",
                    // Times in UTC, whatever the mailbox's time zone.
                    "Prefer": "outlook.timezone=\"UTC\"",
                ]))
    }

    /// "2026-10-13T08:00:00Z", as Graph's filters and calendar views take it.
    static func graphTime(_ date: Date) -> String {
        date.formatted(
            .iso8601.year().month().day().time(includingFractionalSeconds: false).timeZone(separator: .omitted))
    }

    /// Graph's times: "2026-10-12T09:14:03Z", or a `dateTimeTimeZone`'s "2026-10-14T00:00:00.0000000" (in UTC, as
    /// asked with `Prefer`).
    static func graphDate(_ raw: String?) -> Date? {
        guard let raw, !raw.isEmpty else { return nil }
        if let date = Date(iso8601: raw) { return date }
        let seconds = raw.split(separator: ".").first.map(String.init) ?? raw
        return Date(iso8601: seconds + "Z")
    }

    /// A Teams message's HTML as text.
    static func plain(_ html: String) -> String {
        let text = html.replacing(/(?i)<br\s*\/?>|<\/p>|<\/div>/, with: "\n")
            .replacing(/<[^>]*>/, with: "")
            .replacingOccurrences(of: "&nbsp;", with: " ")
            .replacingOccurrences(of: "&lt;", with: "<")
            .replacingOccurrences(of: "&gt;", with: ">")
            .replacingOccurrences(of: "&quot;", with: "\"")
            .replacingOccurrences(of: "&#39;", with: "'")
            .replacingOccurrences(of: "&amp;", with: "&")
        return Readable.text(text.trimmingCharacters(in: .whitespacesAndNewlines))
    }
}

// MARK: - Wire format

extension MicrosoftPlugin {
    struct Me: Decodable, Sendable {
        var id: String
        var displayName: String
        var mail: String?
        var userPrincipalName: String?

        var address: String? { mail ?? userPrincipalName }

        func owns(_ address: String) -> Bool {
            [mail, userPrincipalName].compactMap { $0 }.contains { $0.caseInsensitiveCompare(address) == .orderedSame }
        }
    }

    struct Page<T: Decodable & Sendable>: Decodable, Sendable {
        var value: [T] = []
        var next: String?

        enum CodingKeys: String, CodingKey {
            case value
            case next = "@odata.nextLink"
        }
    }

    struct Recipient: Decodable, Sendable {
        struct Address: Decodable, Sendable {
            var name: String?
            var address: String?
        }
        var emailAddress: Address

        var person: Person? { (emailAddress.name ?? emailAddress.address).map { Person(name: $0) } }
    }

    struct ZonedTime: Decodable, Sendable { var dateTime: String }

    struct Message: Decodable, Sendable {
        struct Flag: Decodable, Sendable { var dueDateTime: ZonedTime? }

        var id: String
        var subject: String?
        var bodyPreview: String?
        var receivedDateTime: String?
        var webLink: String?
        var importance: String?
        var from: Recipient?
        var toRecipients: [Recipient]?
        var flag: Flag?

        /// Sent to you, not only copied or sent to a list you're on.
        func isTo(_ me: Me) -> Bool {
            (toRecipients ?? []).contains { $0.emailAddress.address.map(me.owns) ?? false }
        }

        func base(accountID: UUID, bundle: InboxBundle) -> InboxItem {
            let important = importance == "high"
            let preview = bodyPreview?.split(whereSeparator: \.isWhitespace).joined(separator: " ")
            let subject = subject?.trimmingCharacters(in: .whitespaces).nilIfEmpty ?? L("(no subject)")
            return InboxItem(
                id: InboxItem.makeID(account: accountID, local: "mail/\(id)"),
                accountID: accountID,
                pluginID: MicrosoftPlugin.manifest.id,
                bundle: bundle,
                title: Readable.text(subject),
                context: "Outlook",
                preview: preview?.nilIfEmpty,
                url: URL(lenient: webLink),
                author: from?.person,
                badges: important
                    ? [Badge(id: .priorityHigh, label: L("High"), symbol: "exclamationmark.2", tone: .warning)] : [],
                date: MicrosoftPlugin.graphDate(receivedDateTime) ?? .now,
                needsAction: true,
                priority: important ? .high : nil
            )
        }

        /// Flagged: a task you set yourself, with its due date if it has one.
        func flaggedItem(accountID: UUID) -> InboxItem {
            var item = base(accountID: accountID, bundle: .tasks)
            item.badges.append(Badge(id: .flagged, label: L("Flagged"), symbol: "flag", tone: .accent))
            item.due = MicrosoftPlugin.graphDate(flag?.dueDateTime?.dateTime)
            if let due = item.due { item.badges.append(DueDate.badge(for: due)) }
            return item
        }

        /// Unread and sent to you: sorted by what it asks, as a direct message is.
        func unreadItem(accountID: UUID) -> InboxItem {
            base(accountID: accountID, bundle: .directMessages)
        }
    }

    struct Event: Decodable, Sendable {
        struct ResponseStatus: Decodable, Sendable { var response: String? }

        var id: String
        var subject: String?
        var webLink: String?
        var isOrganizer: Bool?
        var isCancelled: Bool?
        var responseRequested: Bool?
        var lastModifiedDateTime: String?
        var start: ZonedTime?
        var organizer: Recipient?
        var responseStatus: ResponseStatus?

        var startDate: Date? { MicrosoftPlugin.graphDate(start?.dateTime) }

        /// An invitation you haven't answered, for a meeting still to come.
        func awaitsAnswer(now: Date) -> Bool {
            guard responseStatus?.response == "notResponded", isOrganizer != true, isCancelled != true,
                responseRequested != false, let startDate
            else { return false }
            return startDate > now
        }

        func item(accountID: UUID) -> InboxItem {
            var badges = [Badge(id: .invitation, label: L("Invitation"), symbol: "calendar.badge.plus", tone: .accent)]
            if let startDate {
                badges.append(
                    Badge(
                        id: .event,
                        label: startDate.formatted(
                            .dateTime.weekday(.abbreviated).day().month(.abbreviated).hour().minute()),
                        symbol: "calendar", tone: .neutral))
            }
            var item = InboxItem(
                id: InboxItem.makeID(account: accountID, local: "event/\(id)"),
                accountID: accountID,
                pluginID: MicrosoftPlugin.manifest.id,
                bundle: .reply,
                title: Readable.text(subject?.trimmingCharacters(in: .whitespaces).nilIfEmpty ?? L("(no subject)")),
                context: "Outlook",
                url: URL(lenient: webLink),
                author: organizer?.person,
                badges: badges,
                // Moved or renamed: news again.
                date: MicrosoftPlugin.graphDate(lastModifiedDateTime) ?? .now,
                needsAction: true,
                due: startDate
            )
            // Once the meeting starts, answering is moot.
            item.expires = startDate
            return item
        }
    }

    struct Chat: Decodable, Sendable {
        struct Viewpoint: Decodable, Sendable {
            var isHidden: Bool?
            var lastMessageReadDateTime: String?
        }
        struct ChatMessage: Decodable, Sendable {
            struct Body: Decodable, Sendable { var content: String? }
            struct From: Decodable, Sendable {
                struct Sender: Decodable, Sendable {
                    var id: String?
                    var displayName: String?
                }
                var user: Sender?
                /// Set instead of `user` when an app or a bot wrote the message.
                var application: Sender?
            }
            var createdDateTime: String?
            var isDeleted: Bool?
            var body: Body?
            var from: From?
        }

        var id: String
        var topic: String?
        var chatType: String
        var webUrl: String?
        var viewpoint: Viewpoint?
        var lastMessagePreview: ChatMessage?

        /// The last message, when someone else wrote it this week and you haven't read it: one-to-one chats as
        /// direct messages, group chats as mentions. Meeting chats are left out.
        func item(me: Me, accountID: UUID, since: Date) -> InboxItem? {
            // A person, or an app or bot (Teams says which): an app's message is to read, as Slack's apps.
            guard let message = lastMessagePreview, message.isDeleted != true,
                let sender = message.from?.user ?? message.from?.application,
                let date = MicrosoftPlugin.graphDate(message.createdDateTime)
            else { return nil }
            let app = message.from?.user == nil
            let bundle: InboxBundle
            switch chatType {
            case "oneOnOne": bundle = .directMessages
            case "group": bundle = .mentions
            default: return nil
            }
            let read = MicrosoftPlugin.graphDate(viewpoint?.lastMessageReadDateTime)
            guard sender.id != me.id, date > since, read.map({ $0 < date }) ?? true, viewpoint?.isHidden != true
            else { return nil }
            let lines = MicrosoftPlugin.plain(message.body?.content ?? "").components(separatedBy: .newlines)
                .map { $0.trimmingCharacters(in: .whitespaces) }.filter { !$0.isEmpty }
            let preview = lines.dropFirst().joined(separator: " ")
            var item = InboxItem(
                id: InboxItem.makeID(account: accountID, local: "chat/\(id)"),
                accountID: accountID,
                pluginID: MicrosoftPlugin.manifest.id,
                bundle: app ? .read : bundle,
                title: lines.first ?? L("(no text)"),
                context: topic?.trimmingCharacters(in: .whitespaces).nilIfEmpty ?? sender.displayName ?? "Teams",
                preview: preview.nilIfEmpty,
                url: URL(lenient: webUrl),
                author: sender.displayName.map { Person(name: $0, bot: app) },
                date: date,
                needsAction: !app
            )
            item.reach = chatType == "oneOnOne" ? .direct : .group
            return item
        }
    }
}

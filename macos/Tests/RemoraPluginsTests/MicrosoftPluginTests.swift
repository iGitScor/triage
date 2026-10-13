import Foundation
import RemoraCore
import Testing

@testable import RemoraPlugins

/// Same fixtures as Windows (`microsoft.rs`).
struct MicrosoftPluginTests {
    static let me = """
        {"id": "u-me", "displayName": "Alice Martin", "mail": "alice@acme.com",
         "userPrincipalName": "alice@acme.onmicrosoft.com"}
        """

    static let flagged = """
        {"value": [
          {"id": "m1", "subject": "Budget 2027", "bodyPreview": "Can you  check\\r\\nthe numbers?",
           "receivedDateTime": "2026-10-12T16:00:00Z", "webLink": "https://outlook.office365.com/owa/?ItemID=m1",
           "importance": "high", "from": {"emailAddress": {"name": "Bob Stone", "address": "bob@acme.com"}},
           "toRecipients": [{"emailAddress": {"address": "alice@acme.com"}}],
           "flag": {"flagStatus": "flagged", "dueDateTime": {"dateTime": "2026-10-20T00:00:00.0000000", "timeZone": "UTC"}}}
        ]}
        """

    static let unread = """
        {"value": [
          {"id": "m1", "subject": "Budget 2027", "receivedDateTime": "2026-10-12T16:00:00Z",
           "toRecipients": [{"emailAddress": {"address": "alice@acme.com"}}]},
          {"id": "m2", "subject": "Lunch on Friday?", "bodyPreview": "Are you in?", "receivedDateTime": "2026-10-13T08:00:00Z",
           "from": {"emailAddress": {"name": "Carol", "address": "carol@acme.com"}},
           "toRecipients": [{"emailAddress": {"address": "ALICE@acme.onmicrosoft.com"}}]},
          {"id": "m3", "subject": "Newsletter", "receivedDateTime": "2026-10-13T07:00:00Z",
           "toRecipients": [{"emailAddress": {"address": "everyone@acme.com"}}]}
        ], "@odata.nextLink": "https://graph.microsoft.com/v1.0/me/messages?$skip=25"}
        """

    static let events = """
        {"value": [
          {"id": "e1", "subject": "Design review", "webLink": "https://outlook.office365.com/calendar/item/e1",
           "isOrganizer": false, "isCancelled": false, "responseRequested": true,
           "lastModifiedDateTime": "2026-10-12T10:00:00Z", "start": {"dateTime": "2026-10-14T13:00:00.0000000", "timeZone": "UTC"},
           "organizer": {"emailAddress": {"name": "Dan", "address": "dan@acme.com"}}, "responseStatus": {"response": "notResponded"}},
          {"id": "e2", "subject": "Accepted already", "start": {"dateTime": "2026-10-14T13:00:00.0000000"},
           "responseStatus": {"response": "accepted"}},
          {"id": "e3", "subject": "My own meeting", "isOrganizer": true, "start": {"dateTime": "2026-10-14T13:00:00.0000000"},
           "responseStatus": {"response": "notResponded"}},
          {"id": "e4", "subject": "Cancelled", "isCancelled": true, "start": {"dateTime": "2026-10-14T13:00:00.0000000"},
           "responseStatus": {"response": "notResponded"}},
          {"id": "e5", "subject": "Already started", "start": {"dateTime": "2026-10-13T08:30:00.0000000"},
           "responseStatus": {"response": "notResponded"}}
        ]}
        """

    static let chats = """
        {"value": [
          {"id": "c1", "chatType": "oneOnOne", "webUrl": "https://teams.microsoft.com/l/chat/c1",
           "viewpoint": {"isHidden": false, "lastMessageReadDateTime": "2026-10-12T08:00:00Z"},
           "lastMessagePreview": {"createdDateTime": "2026-10-13T08:30:00Z", "isDeleted": false,
            "body": {"contentType": "html", "content": "<p>Could you send the deck?</p><p>Thanks &amp; see you</p>"},
            "from": {"user": {"id": "u-erin", "displayName": "Erin"}}}},
          {"id": "c2", "topic": "Launch", "chatType": "group",
           "viewpoint": {"lastMessageReadDateTime": "2026-10-13T08:00:00Z"},
           "lastMessagePreview": {"createdDateTime": "2026-10-13T08:40:00Z",
            "body": {"content": "Shipped!"}, "from": {"user": {"id": "u-frank", "displayName": "Frank"}}}},
          {"id": "c3", "chatType": "oneOnOne", "viewpoint": {"lastMessageReadDateTime": "2026-10-13T09:00:00Z"},
           "lastMessagePreview": {"createdDateTime": "2026-10-13T08:50:00Z", "body": {"content": "read already"},
            "from": {"user": {"id": "u-erin", "displayName": "Erin"}}}},
          {"id": "c4", "chatType": "oneOnOne",
           "lastMessagePreview": {"createdDateTime": "2026-10-13T08:50:00Z", "body": {"content": "mine"},
            "from": {"user": {"id": "u-me", "displayName": "Alice Martin"}}}},
          {"id": "c5", "chatType": "meeting",
           "lastMessagePreview": {"createdDateTime": "2026-10-13T08:50:00Z", "body": {"content": "meeting chat"},
            "from": {"user": {"id": "u-erin", "displayName": "Erin"}}}},
          {"id": "c6", "chatType": "oneOnOne",
           "lastMessagePreview": {"createdDateTime": "2026-10-01T08:50:00Z", "body": {"content": "old"},
            "from": {"user": {"id": "u-erin", "displayName": "Erin"}}}}
        ]}
        """

    let accountID = UUID()
    let now = Date(iso8601: "2026-10-13T09:00:00Z")!

    func decode<T: Decodable>(_ json: String) throws -> T {
        try JSONDecoder().decode(T.self, from: Data(json.utf8))
    }

    func snapshot() throws -> SourceSnapshot {
        MicrosoftPlugin.snapshot(
            me: try decode(Self.me), flagged: try decode(Self.flagged), unread: try decode(Self.unread),
            events: try decode(Self.events), chats: try decode(Self.chats), accountID: accountID, now: now)
    }

    func item(_ snapshot: SourceSnapshot, _ id: String) throws -> InboxItem {
        try #require(snapshot.items.first { $0.id == "\(accountID.uuidString)/\(id)" })
    }

    @Test func flaggedMailIsATaskWithItsDueDate() throws {
        let snapshot = try snapshot()
        let budget = try item(snapshot, "mail/m1")
        #expect(budget.bundle == .tasks)
        #expect(budget.title == "Budget 2027")
        #expect(budget.preview == "Can you check the numbers?")
        #expect(budget.author?.name == "Bob Stone")
        #expect(budget.priority == .high)
        #expect(budget.badges.map(\.id).contains("flagged") && budget.badges.map(\.id).contains("priority.high"))
        #expect(budget.due == Date(iso8601: "2026-10-20T00:00:00Z"))
        #expect(snapshot.items.filter { $0.id.hasSuffix("/mail/m1") }.count == 1, "flagged and unread: once")
    }

    @Test func unreadMailCountsOnlyWhenSentToYou() throws {
        let snapshot = try snapshot()
        #expect(try item(snapshot, "mail/m2").bundle == .directMessages)
        #expect(!snapshot.items.contains { $0.id.hasSuffix("/mail/m3") }, "a list you're on isn't you")
        #expect(snapshot.identity == "alice@acme.com")
        #expect(snapshot.remarks.count == 1, "more unread mail than Remora reads")
    }

    @Test func invitationsYouHaventAnsweredUntilTheMeetingStarts() throws {
        let snapshot = try snapshot()
        let review = try item(snapshot, "event/e1")
        #expect(review.bundle == .reply)
        #expect(review.author?.name == "Dan")
        let start = Date(iso8601: "2026-10-14T13:00:00Z")
        #expect(review.due == start && review.expires == start)
        #expect(review.badges.map(\.id) == ["invitation", "event"])
        #expect(snapshot.items.filter { $0.id.contains("/event/") }.count == 1, "answered, yours, cancelled or started")
    }

    @Test func unreadTeamsChatsFromOthersThisWeek() throws {
        let snapshot = try snapshot()
        let erin = try item(snapshot, "chat/c1")
        #expect(erin.bundle == .directMessages)
        #expect(erin.title == "Could you send the deck?")
        #expect(erin.preview == "Thanks & see you")
        #expect(erin.context == "Erin")
        let launch = try item(snapshot, "chat/c2")
        #expect(launch.bundle == .mentions && launch.context == "Launch")
        #expect(snapshot.items.filter { $0.id.contains("/chat/") }.count == 2, "read, yours, a meeting's or old")
    }

    @Test func needsTheTenantTheAppAndASignIn() {
        for missing in ["tenant", "clientId", "refreshToken"] {
            var values = ["tenant": "acme.com", "clientId": "app-1", "refreshToken": "r1"]
            values[missing] = nil
            #expect(throws: PluginError.missingField(missing)) {
                try MicrosoftPlugin(config: config(values), http: StubHTTP(routes: [:]))
            }
        }
    }

    @Test func fetchesWithOneTokenAndKeepsMailWhenTeamsFails() async throws {
        let http = StubHTTP(routes: [
            "/oauth2/v2.0/token": #"{"access_token": "a1", "refresh_token": "r2", "expires_in": 3600}"#,
            "/v1.0/me": Self.me, "/me/messages": Self.flagged, "/inbox/messages": Self.unread,
            "/me/calendarView": Self.events,
        ])
        let cache = MemoryAccountCache()
        let plugin = try MicrosoftPlugin(
            config: PluginConfig(
                accountID: accountID, values: ["tenant": "acme.com", "clientId": "app-1", "refreshToken": "r1"],
                cache: cache),
            http: http)
        let snapshot = try await plugin.fetch()
        #expect(snapshot.items.contains { $0.id.hasSuffix("/mail/m2") })
        #expect(snapshot.remarks.contains { $0.hasPrefix(L("%@ couldn’t be read: %@", "Teams chats", "")) })
        #expect(await cache.takeRenewed() == ["refreshToken": "r2"])
    }

    @Test func readsGraphTimesAndTeamsHTML() {
        #expect(MicrosoftPlugin.graphDate("2026-10-14T13:00:00.0000000") == Date(iso8601: "2026-10-14T13:00:00Z"))
        #expect(MicrosoftPlugin.graphDate("2026-10-12T16:00:00Z") == Date(iso8601: "2026-10-12T16:00:00Z"))
        #expect(MicrosoftPlugin.plain("<div>Hi <at id=\"0\">Alice</at>,<br>ok?</div>") == "Hi Alice,\nok?")
        #expect(MicrosoftPlugin.graphTime(Date(iso8601: "2026-10-13T08:00:00Z")!) == "2026-10-13T08:00:00Z")
    }
}

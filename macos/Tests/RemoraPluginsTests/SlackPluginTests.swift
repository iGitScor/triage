import Foundation
import Testing
import RemoraCore
@testable import RemoraPlugins

struct SlackPluginTests {
    @Test func cleansMrkdwn() {
        let text = "<@UME> see <https://x.io|the doc> &amp; ping <@U2|erin> in <#C1|general> <!here>"
        #expect(SlackText.plain(text, me: "UME") == "@you see the doc & ping @erin in #general @here")
    }

    let paris = TimeZone(identifier: "Europe/Paris")!
    /// The event in the screenshot: 16:15 to 17:15 in Paris on 9 October 2026.
    let start = Date(timeIntervalSince1970: 1_791_555_300)

    /// A Google Calendar reminder, as Slack sends it. Same case as Windows.
    @Test func calendarReminderReadsInYourLanguageAndTimeZone() {
        let text = ":loudspeaker: _1 minute until this event:_\n<!date^1791555300^{time}|4:15 PM> - <!date^1791558900^{time}|5:15 PM> "
            + "Staging Movidone *Guests:* You _(organizer)_, <mailto:lou@acme.io|Lou Martin>"
        let style = SlackDateStyle(locale: Locale(identifier: "fr_FR"), timeZone: paris, now: start)
        #expect(SlackText.plain(text, me: "UME", style: style)
            == "📢 1 minute until this event:\n16:15 - 17:15 Staging Movidone Guests: You (organizer), Lou Martin")
    }

    @Test func dateTokens() {
        let style = SlackDateStyle(locale: Locale(identifier: "en_GB"), timeZone: paris, now: start.addingTimeInterval(7_200))
        let format = { (format: String) in SlackText.formatted(self.start, format, fallback: "fallback", style: style) }
        #expect(format("{date_num} {time}") == "2026-10-09 16:15")
        #expect(format("{date_short}") == "9 Oct 2026")
        #expect(format("{date_long}").hasPrefix("Friday"))
        #expect(format("{date_pretty}") == "Today")
        #expect(format("{ago}") == "2 hours ago")
        #expect(format("on {nonsense}") == "fallback", "a token Slack may add later")
        #expect(SlackText.plain("<!date^1791555300^{date_num}^https://x.io|x>", me: "", style: style) == "2026-10-09", "with a link")
        #expect(SlackText.plain("<tel:+33100000000|Call Lou>, <!subteam^S1|@design>, <mailto:a@b.io>", me: "") == "Call Lou, @design, a@b.io")
    }

    /// An app's reminder is over once the event starts; a person's message never expires.
    @Test func appRemindersExpireWhenTheEventStarts() async throws {
        let search = """
        {"ok": true, "messages": {"matches": [
          {"ts": "1791555240.0", "text": "_1 minute until this event:_ <!date^1791555300^{time}|4:15 PM> - <!date^1791558900^{time}|5:15 PM>",
           "username": "Google Calendar", "bot_id": "B1", "channel": {"id": "D7", "is_im": true}},
          {"ts": "1791555000.0", "text": "Shall we meet at <!date^1791555300^{time}|4:15 PM>?", "username": "carol", "channel": {"id": "D1", "is_im": true}}
        ]}}
        """
        let http = StubHTTP(routes: [
            "/auth.test": #"{"ok": true, "user_id": "UME", "user": "alice", "team": "Acme"}"#,
            "/search.messages": search,
        ])
        let items = try await SlackPlugin(config: config(["token": "xoxp"]), http: http).fetch().items
        let calendar = try #require(items.first { $0.author?.name == "Google Calendar" })
        #expect(calendar.expires == start)
        #expect(items.first { $0.author?.name == "carol" }?.expires == nil)

        let assembler = InboxAssembler()
        #expect(assembler.placement(of: calendar, state: nil, now: start.addingTimeInterval(-30)) == .inbox)
        #expect(assembler.placement(of: calendar, state: nil, now: start) == .cleared)
        var pinned = ItemState()
        pinned.pinned = true
        #expect(assembler.placement(of: calendar, state: pinned, now: start) == .inbox, "pinned stays")
    }

    @Test func keepsLatestMessagePerConversation() async throws {
        let search = """
        {"ok": true, "messages": {"matches": [
          {"ts": "1700000100.0", "text": "second", "username": "carol", "channel": {"id": "D1", "is_im": true}},
          {"ts": "1700000000.0", "text": "first", "username": "carol", "channel": {"id": "D1", "is_im": true}},
          {"ts": "1700000050.0", "text": "hey <@UME>\\nmore", "username": "erin", "channel": {"id": "C1", "name": "dev"}}
        ]}}
        """
        let http = StubHTTP(routes: [
            "/auth.test": #"{"ok": true, "user_id": "UME", "user": "alice", "team": "Acme", "team_id": "T42"}"#,
            "/search.messages": search,
        ])
        let snapshot = try await SlackPlugin(config: config(["token": "xoxp"]), http: http).fetch()

        let direct = snapshot.items.filter { $0.bundle == .directMessages }
        #expect(direct.map(\.title) == ["second"])
        let mention = try #require(snapshot.items.first { $0.bundle == .mentions })
        #expect(mention.title == "hey @you")
        #expect(mention.preview == "more")
        #expect(mention.context == "#dev")
        #expect(mention.appURL?.absoluteString == "slack://channel?team=T42&id=C1")
        #expect(direct.first?.appURL?.absoluteString == "slack://channel?team=T42&id=D1")
    }

    /// Google Calendar's app DMs event updates with an empty `text`: the words are in attachments or blocks.
    @Test func appMessagesAreReadableAndToRead() async throws {
        let search = """
        {"ok": true, "messages": {"matches": [
          {"ts": "1700000300.0", "text": "", "username": "Google Calendar", "bot_id": "B1", "channel": {"id": "D7", "is_im": true},
           "attachments": [{"fallback": "Event updated", "pretext": "Event updated", "title": "Design review", "text": "Tomorrow 10:00"}]},
          {"ts": "1700000200.0", "text": "", "username": "Jira Cloud", "channel": {"id": "D8", "is_im": true},
           "blocks": [{"type": "section", "text": {"type": "mrkdwn", "text": "*PAY-12* moved to Done"}},
                      {"type": "context", "elements": [{"type": "mrkdwn", "text": "by Erin"}, {"type": "image", "image_url": "x"}]}]},
          {"ts": "1700000100.0", "text": "", "username": "Mystery", "channel": {"id": "D9", "is_im": true}},
          {"ts": "1700000000.0", "text": "can you look?", "username": "carol", "channel": {"id": "D1", "is_im": true}}
        ]}}
        """
        let http = StubHTTP(routes: [
            "/auth.test": #"{"ok": true, "user_id": "UME", "user": "alice", "team": "Acme"}"#,
            "/search.messages": search,
        ])
        let items = try await SlackPlugin(config: config(["token": "xoxp"]), http: http).fetch().items
            .map(VerbClassifier().classify)
        let calendar = try #require(items.first { $0.author?.name == "Google Calendar" })
        #expect(calendar.title == "Event updated")
        #expect(calendar.preview == "Design review Tomorrow 10:00")
        #expect(calendar.bundle == .read)
        #expect(!calendar.needsAction)
        let jira = try #require(items.first { $0.author?.name == "Jira Cloud" })
        #expect(jira.title == "PAY-12 moved to Done")
        #expect(jira.preview == "by Erin")
        #expect(jira.bundle == .read)
        let empty = try #require(items.first { $0.author?.name == "Mystery" })
        #expect(empty.title == L("(no text)"))
        #expect(empty.bundle == .read)
        let person = try #require(items.first { $0.author?.name == "carol" })
        #expect(person.bundle == .reply)
        #expect(person.needsAction)
    }

    @Test func reportsSlackErrors() async throws {
        let http = StubHTTP(routes: ["/auth.test": #"{"ok": false, "error": "invalid_auth"}"#])
        await #expect(throws: HTTPError.unauthorized) {
            try await SlackPlugin(config: config(["token": "bad"]), http: http).fetch()
        }
    }

    /// A reply in a thread you wrote in, without a mention, reaches the inbox; one that mentions you is
    /// already a mention, and your own or older replies don't count.
    @Test func repliesInYourThreads() async throws {
        let mine = """
        {"ok": true, "messages": {"matches": [
          {"ts": "1700000200.0", "text": "I can take it", "channel": {"id": "C1", "name": "design"},
           "permalink": "https://acme.slack.com/archives/C1/p1700000200?thread_ts=1700000100.0"},
          {"ts": "1700000050.0", "text": "older one", "channel": {"id": "C1", "name": "design"},
           "permalink": "https://acme.slack.com/archives/C1/p1700000050?thread_ts=1700000100.0"},
          {"ts": "1700000300.0", "text": "in a DM", "channel": {"id": "D1", "is_im": true}}
        ]}}
        """
        let replies = """
        {"ok": true, "messages": [
          {"ts": "1700000100.0", "text": "Who can review the mockups?", "user": "U2"},
          {"ts": "1700000200.0", "text": "I can take it", "user": "UME"},
          {"ts": "1700000400.0", "text": "Great, *thanks*! Tomorrow then", "user": "U2", "user_profile": {"display_name": "erin"}},
          {"ts": "1700000500.0", "text": "<@UME> also this", "user": "U3"}
        ]}
        """
        let http = SlackStub(mine: mine, replies: replies)
        let items = try await SlackPlugin(config: config(["token": "xoxp"]), http: http).fetch().items
        let thread = try #require(items.first { $0.context.hasPrefix("Thread") })
        #expect(thread.title == "Great, thanks! Tomorrow then")
        #expect(thread.author?.name == "erin")
        #expect(thread.context == "Thread in #design")
        #expect(thread.id.hasSuffix("/C1/thread/1700000100.0"))
        #expect(thread.bundle == .mentions && thread.needsAction)
        #expect(await http.repliesAsked == ["C1 1700000100.0 1700000200.0"], "one call, from your last message, no DM")
    }

    @Test func threadsWithoutTheHistoryScopeSaySo() async throws {
        let mine = #"{"ok": true, "messages": {"matches": [{"ts": "1.0", "text": "x", "channel": {"id": "C1", "name": "a"}}]}}"#
        let snapshot = try await SlackPlugin(config: config(["token": "xoxp"]), http: SlackStub(mine: mine, replies: #"{"ok": false, "error": "missing_scope"}"#)).fetch()
        #expect(snapshot.remarks.count == 1 && snapshot.remarks[0].contains("channels:history"))
        #expect(!snapshot.items.contains { $0.context.hasPrefix("Thread") })
    }

    @Test func appManifestLinkRequestsSearchScope() throws {
        let url = try #require(SlackPlugin.appManifestURL)
        let items = URLComponents(url: url, resolvingAgainstBaseURL: false)?.queryItems ?? []
        #expect(items.first { $0.name == "new_app" }?.value == "1")
        let manifest = try #require(items.first { $0.name == "manifest_json" }?.value)
        let json = try JSONSerialization.jsonObject(with: Data(manifest.utf8)) as? [String: Any]
        let scopes = ((json?["oauth_config"] as? [String: Any])?["scopes"] as? [String: Any])?["user"] as? [String]
        #expect(scopes == ["search:read", "channels:history", "groups:history"])
    }
}

/// Answers Slack by method, and the searches by their query: your own thread messages, or nothing.
actor SlackStub: HTTPClient {
    let mine: String
    let replies: String
    private(set) var repliesAsked: [String] = []

    init(mine: String, replies: String) {
        self.mine = mine
        self.replies = replies
    }

    func send(_ request: URLRequest) async throws -> (Data, HTTPURLResponse) {
        let url = request.url!
        let query = URLComponents(url: url, resolvingAgainstBaseURL: false)?.queryItems ?? []
        let value = { (name: String) in query.first { $0.name == name }?.value ?? "" }
        let body: String
        switch url.lastPathComponent {
        case "auth.test": body = #"{"ok": true, "user_id": "UME", "user": "alice", "team": "Acme", "team_id": "T1"}"#
        case "conversations.replies":
            repliesAsked.append("\(value("channel")) \(value("ts")) \(value("oldest"))")
            body = replies
        default: body = value("query").contains("is:thread") ? mine : #"{"ok": true, "messages": {"matches": []}}"#
        }
        return (Data(body.utf8), HTTPURLResponse(url: url, statusCode: 200, httpVersion: nil, headerFields: nil)!)
    }
}

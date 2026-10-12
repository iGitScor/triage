import Foundation
import RemoraCore
import Testing

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
        let text =
            ":loudspeaker: _1 minute until this event:_\n<!date^1791555300^{time}|4:15 PM> - <!date^1791558900^{time}|5:15 PM> "
            + "Staging Movidone *Guests:* You _(organizer)_, <mailto:lou@acme.io|Lou Martin>"
        let style = SlackDateStyle(locale: Locale(identifier: "fr_FR"), timeZone: paris, now: start)
        #expect(
            SlackText.plain(text, me: "UME", style: style)
                == "📢 1 minute until this event:\n16:15 - 17:15 Staging Movidone Guests: You (organizer), Lou Martin")
    }

    @Test func dateTokens() {
        let style = SlackDateStyle(
            locale: Locale(identifier: "en_GB"), timeZone: paris, now: start.addingTimeInterval(7_200))
        let format = { (format: String) in SlackText.formatted(self.start, format, fallback: "fallback", style: style) }
        #expect(format("{date_num} {time}") == "2026-10-09 16:15")
        #expect(format("{date_short}") == "9 Oct 2026")
        #expect(format("{date_long}").hasPrefix("Friday"))
        #expect(format("{date_pretty}") == "Today")
        #expect(format("{ago}") == "2 hours ago")
        #expect(format("on {nonsense}") == "fallback", "a token Slack may add later")
        #expect(
            SlackText.plain("<!date^1791555300^{date_num}^https://x.io|x>", me: "", style: style) == "2026-10-09",
            "with a link")
        #expect(
            SlackText.plain("<tel:+33100000000|Call Lou>, <!subteam^S1|@design>, <mailto:a@b.io>", me: "")
                == "Call Lou, @design, a@b.io")
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

    /// Every way Slack says the token no longer works asks for reconnecting; the same list as on Windows.
    @Test(arguments: ["invalid_auth", "not_authed", "token_revoked", "token_expired", "account_inactive"])
    func reportsSlackErrors(error: String) async throws {
        let http = StubHTTP(routes: ["/auth.test": #"{"ok": false, "error": "\#(error)"}"#])
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

    /// FEAT-07: Slack sends the thread's first message whatever `oldest` says; it isn't a reply to you.
    @Test func theFirstMessageOfAThreadIsNotAReply() async throws {
        let mine = """
            {"ok": true, "messages": {"matches": [
              {"ts": "1700000200.0", "text": "I can take it", "channel": {"id": "C1", "name": "design"},
               "permalink": "https://acme.slack.com/archives/C1/p1700000200?thread_ts=1700000100.0"}
            ]}}
            """
        let replies = """
            {"ok": true, "messages": [
              {"ts": "1700000100.0", "text": "Who can review the mockups?", "user": "U2"},
              {"ts": "1700000200.0", "text": "I can take it", "user": "UME"}
            ]}
            """
        let items = try await SlackPlugin(
            config: config(["token": "xoxp"]), http: SlackStub(mine: mine, replies: replies)
        )
        .fetch().items
        let thread = try #require(items.first { $0.context.hasPrefix("Thread") })
        #expect(thread.bundle == .answered && !thread.needsAction, "the ball is in their court: your answer stands")
    }

    /// Your last message in a thread, as Slack returns it with `inclusive`: the reactions on it decide.
    static let yourThread = """
        {"ok": true, "messages": {"matches": [
          {"ts": "1700000200.0", "text": "Can someone review the mockups?", "channel": {"id": "C1", "name": "design"},
           "permalink": "https://acme.slack.com/archives/C1/p1700000200?thread_ts=1700000100.0"}
        ]}}
        """

    static func replies(reactions: String) -> String {
        """
        {"ok": true, "messages": [
          {"ts": "1700000100.0", "text": "Mockups are up", "user": "U2"},
          {"ts": "1700000200.0", "text": "Can someone review the mockups?", "user": "UME", "reactions": \(reactions)}
        ]}
        """
    }

    func thread(reactions: String) async throws -> InboxItem? {
        let http = SlackStub(mine: Self.yourThread, replies: Self.replies(reactions: reactions))
        return try await SlackPlugin(config: config(["token": "xoxp"]), http: http).fetch().items
            .first { $0.id.contains("/thread/") }
    }

    /// Nobody wrote back: their reactions on your message speak for them.
    @Test func theirReactionsOnYourMessage() async throws {
        let waiting = try #require(try await thread(reactions: "[]"))
        #expect(waiting.bundle == .answered && !waiting.needsAction, "someone else started it: yours is an answer")
        #expect(waiting.title == "Can someone review the mockups?")
        #expect(waiting.id.hasSuffix("/C1/thread/1700000100.0"), "the same item as a reply would be")

        let seen = try #require(
            try await thread(reactions: #"[{"name": "+1::skin-tone-2", "users": ["U2"], "count": 1}]"#))
        #expect(seen.bundle == .answered && seen.othersReacted(.ack), "acknowledged, still waiting")

        let question = try #require(
            try await thread(reactions: #"[{"name": "thinking_face", "users": ["U2", "U3"], "count": 2}]"#))
        #expect(question.bundle == .reply && question.needsAction)
        #expect(
            question.context == "🤔 on your message in #design" && question.title == "Can someone review the mockups?")

        let closed = try await thread(reactions: #"[{"name": "white_check_mark", "users": ["U2"], "count": 1}]"#)
        #expect(closed?.bundle == .finished, "their ✅: finished, in Done")
        let own = try await thread(reactions: #"[{"name": "white_check_mark", "users": ["UME"], "count": 1}]"#)
        #expect(own?.bundle == .answered, "your own ✅ answers nothing")
    }

    /// A reply keeps its reactions; your ✅ on it is what moves it to Done.
    @Test func aReplyCarriesItsReactions() async throws {
        let replies = """
            {"ok": true, "messages": [
              {"ts": "1700000200.0", "text": "Can someone review the mockups?", "user": "UME"},
              {"ts": "1700000400.0", "text": "On it tomorrow", "user": "U2",
               "reactions": [{"name": "white_check_mark", "users": ["UME"], "count": 1}]}
            ]}
            """
        let http = SlackStub(mine: Self.yourThread, replies: replies)
        let items = try await SlackPlugin(config: config(["token": "xoxp"]), http: http).fetch().items
        let reply = try #require(items.first { $0.context.hasPrefix("Thread") })
        #expect(reply.title == "On it tomorrow" && reply.yourReaction == .done)
    }

    static func mention(_ ts: String) -> String {
        #"{"ts": "\#(ts)", "text": "<@UME> can you look?", "username": "erin", "channel": {"id": "C9", "name": "ops"}}"#
    }

    /// Search returns no reactions: the messages you reacted to come in one request, and the newest that need you
    /// are asked about one by one, at most 15, each kept 10 minutes.
    @Test func reactionsOnMentions() async throws {
        let mentions =
            #"{"ok": true, "messages": {"matches": ["#
            + (1...20).map { Self.mention("17000000\($0 + 10).0") }.joined(separator: ",") + "]}}"
        let reacted = """
            {"ok": true, "items": [
              {"type": "message", "channel": "C9", "message": {"ts": "1700000011.0",
               "reactions": [{"name": "white_check_mark", "users": ["UME", "U2"], "count": 2}]}},
              {"type": "file", "file": {"id": "F1"}}
            ]}
            """
        let got = [
            "C9/1700000030.0":
                #"{"ok": true, "message": {"reactions": [{"name": "eyes", "users": ["U4"], "count": 1}]}}"#
        ]
        let http = SlackStub(
            mine: #"{"ok": true, "messages": {"matches": []}}"#, replies: "{}", mentions: mentions, reacted: reacted,
            got: got)
        let plugin = try SlackPlugin(
            config: PluginConfig(accountID: UUID(), values: ["token": "xoxp"], cache: MemoryAccountCache()), http: http)
        let items = try await plugin.fetch().items
        let handled = try #require(items.first { $0.id.hasSuffix("C9/1700000011.0") })
        #expect(handled.yourReaction == .done && handled.reactions?.first?.count == 2)
        let looked = try #require(items.first { $0.id.hasSuffix("C9/1700000030.0") })
        #expect(looked.yourReaction == nil && looked.othersReacted(.started))
        #expect(await http.methods.filter { $0 == "reactions.get" }.count == SlackPlugin.reactionsAsked)
        #expect(await http.methods.filter { $0 == "reactions.list" }.count == 1)

        _ = try await plugin.fetch()
        #expect(await http.methods.filter { $0 == "reactions.get" }.count == SlackPlugin.reactionsAsked, "kept")
    }

    @Test func reactionsWithoutTheirPermissionSaySo() async throws {
        let http = SlackStub(
            mine: #"{"ok": true, "messages": {"matches": []}}"#, replies: "{}",
            mentions: #"{"ok": true, "messages": {"matches": [\#(Self.mention("1700000011.0"))]}}"#,
            reacted: #"{"ok": false, "error": "missing_scope"}"#)
        let snapshot = try await SlackPlugin(config: config(["token": "xoxp"]), http: http).fetch()
        #expect(snapshot.remarks.count == 1 && snapshot.remarks[0].contains("reactions:read"))
        #expect(snapshot.items.count == 1, "everything else works")
        #expect(await !http.methods.contains("reactions.get"))
    }

    /// A workspace's own emoji: an alias is the emoji it stands for; another has its picture and a meaning from
    /// its name.
    @Test func workspaceEmoji() async throws {
        let picture = "https://emoji.slack-edge.com/T1/approved-by-design/abc.png"
        let emoji = #"{"ok": true, "emoji": {"yes": "alias:white_check_mark", "approved-by-design": "\#(picture)"}}"#
        let alias = SlackStub(
            mine: Self.yourThread, replies: Self.replies(reactions: #"[{"name": "yes", "users": ["U2"], "count": 1}]"#),
            emoji: emoji)
        let items = try await SlackPlugin(config: config(["token": "xoxp"]), http: alias).fetch().items
        #expect(items.first { $0.id.contains("/thread/") }?.bundle == .finished, "✅ through its alias: finished")

        let custom = SlackPlugin.resolve(
            Reaction(name: "approved-by-design", count: 1, mine: false), custom: ["approved-by-design": picture])
        #expect(custom.image?.host == "emoji.slack-edge.com" && custom.meaning == .done)
        #expect(custom.emoji == ":approved-by-design:")
        #expect(SlackPlugin.resolve(Reaction(name: "yes", count: 1, mine: false), custom: [:]).meaning == .ack)
    }

    /// The assistant reads what reactions mean and how many, never who reacted.
    @Test func theAssistantReadsWhatReactionsMean() {
        var item = InboxItem(
            id: "a", accountID: UUID(), pluginID: "slack", bundle: .reply, title: "Can you look?", context: "#ops",
            date: .now)
        item.reactions = [Reaction(name: "eyes", count: 2, mine: true)]
        let message = BriefPrompt.message(items: [item], now: .now)
        #expect(message.contains("you reacted: started") && message.contains("others reacted: started ×1"))
        #expect(!message.contains("UME"))
    }

    static func matches(_ messages: String...) -> String {
        #"{"ok": true, "messages": {"matches": ["# + messages.joined(separator: ",") + "]}}"
    }

    static func dm(_ ts: String, _ text: String, from user: String, channel: String = "D1") -> String {
        #"{"ts": "\#(ts)", "text": "\#(text)", "user": "\#(user)", "username": "\#(user)", "channel": {"id": "\#(channel)", "is_im": true}}"#
    }

    func items(mentions: String? = nil, direct: String? = nil, sent: String? = nil) async throws -> [InboxItem] {
        let none = #"{"ok": true, "messages": {"matches": []}}"#
        let http = SlackStub(mine: none, replies: "{}", mentions: mentions, direct: direct, sent: sent)
        return try await SlackPlugin(config: config(["token": "xoxp"]), http: http).fetch().items
    }

    /// Whose turn a direct conversation is: theirs to answer when they wrote last, else your message stands.
    @Test func whoWroteLastInADirectConversation() async throws {
        let question = Self.dm("1700000100.0", "Can you send the deck?", from: "U2")
        let answer = Self.dm("1700000200.0", "Here it is", from: "UME")
        let earlier = Self.dm("1700000050.0", "Hi!", from: "UME")

        let theirs = try await items(direct: Self.matches(question), sent: Self.matches(earlier))
        #expect(theirs.count == 1 && theirs[0].bundle == .directMessages && theirs[0].title == "Can you send the deck?")

        let answered = try await items(direct: Self.matches(question), sent: Self.matches(answer, earlier))
        #expect(answered.count == 1 && answered[0].bundle == .answered && answered[0].title == "Here it is")
        #expect(answered[0].id.hasSuffix("/D1") && theirs[0].id.hasSuffix("/D1"), "the same conversation, one item")
        #expect(!answered[0].needsAction && answered[0].expires == nil)

        let asked = try await items(sent: Self.matches(Self.dm("1700000300.0", "Free for lunch?", from: "UME")))
        #expect(asked.count == 1 && asked[0].bundle == .asked, "they never wrote: your question waits")
    }

    static func channel(_ ts: String, _ text: String, from user: String, thread: String? = nil) -> String {
        let link = thread.map { #", "permalink": "https://acme.slack.com/archives/C9/p1?thread_ts=\#($0)""# } ?? ""
        return
            #"{"ts": "\#(ts)", "text": "\#(text)", "user": "\#(user)", "username": "\#(user)", "channel": {"id": "C9", "name": "ops"}\#(link)}"#
    }

    /// A mention is answered by your later message in its thread (which the thread's item then follows), or in the
    /// channel when it names them or comes soon after; a later message of yours about something else doesn't count.
    @Test func whenAMentionIsAnswered() async throws {
        let mention = Self.matches(Self.channel("1700000000.0", "<@UME> can you check the logs?", from: "U2"))
        let named = Self.channel("1700020000.0", "<@U2> logs look fine", from: "UME")
        let soon = Self.channel("1700001000.0", "Looking, give me a sec", from: "UME")
        let unrelated = Self.channel("1700020000.0", "Deploying the web app now", from: "UME")
        let inThread = Self.channel("1700000500.0", "On it", from: "UME", thread: "1700000000.0")

        #expect(try await items(mentions: mention, sent: Self.matches(unrelated))[0].bundle == .mentions)
        let answer = try await items(mentions: mention, sent: Self.matches(named))[0]
        #expect(answer.bundle == .answered && answer.title.hasSuffix("logs look fine"), "your answer stands")
        #expect(try await items(mentions: mention, sent: Self.matches(soon))[0].bundle == .answered)
        #expect(try await items(mentions: mention, sent: Self.matches(inThread))[0].bundle == .finished)
    }

    @Test func threadsWithoutTheHistoryScopeSaySo() async throws {
        let mine =
            #"{"ok": true, "messages": {"matches": [{"ts": "1.0", "text": "x", "channel": {"id": "C1", "name": "a"}}]}}"#
        let snapshot = try await SlackPlugin(
            config: config(["token": "xoxp"]),
            http: SlackStub(mine: mine, replies: #"{"ok": false, "error": "missing_scope"}"#)
        ).fetch()
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
        #expect(scopes == ["search:read", "channels:history", "groups:history", "reactions:read", "emoji:read"])
    }
}

/// Answers Slack by method, and the searches by their query: your own thread messages, mentions of you, or nothing.
/// Reactions: what `reactions.list` and `emoji.list` say, and `reactions.get` by message.
actor SlackStub: HTTPClient {
    let mine: String
    let replies: String
    let mentions: String?
    let reacted: String?
    let got: [String: String]
    let emoji: String?
    let direct: String?
    let sent: String?
    private(set) var repliesAsked: [String] = []
    private(set) var methods: [String] = []

    init(
        mine: String, replies: String, mentions: String? = nil, reacted: String? = nil, got: [String: String] = [:],
        emoji: String? = nil, direct: String? = nil, sent: String? = nil
    ) {
        self.direct = direct
        self.sent = sent
        self.mine = mine
        self.replies = replies
        self.mentions = mentions
        self.reacted = reacted
        self.got = got
        self.emoji = emoji
    }

    func send(_ request: URLRequest) async throws -> (Data, HTTPURLResponse) {
        let url = request.url!
        let query = URLComponents(url: url, resolvingAgainstBaseURL: false)?.queryItems ?? []
        let value = { (name: String) in query.first { $0.name == name }?.value ?? "" }
        let none = #"{"ok": true, "messages": {"matches": []}}"#
        methods.append(url.lastPathComponent)
        let body: String
        switch url.lastPathComponent {
        case "auth.test": body = #"{"ok": true, "user_id": "UME", "user": "alice", "team": "Acme", "team_id": "T1"}"#
        case "conversations.replies":
            repliesAsked.append("\(value("channel")) \(value("ts")) \(value("oldest"))")
            body = replies
        case "reactions.list": body = reacted ?? #"{"ok": true, "items": []}"#
        case "reactions.get": body = got["\(value("channel"))/\(value("timestamp"))"] ?? #"{"ok": true, "message": {}}"#
        case "emoji.list": body = emoji ?? #"{"ok": true, "emoji": {}}"#
        default:
            let search = value("query")
            if search.contains("is:thread") {
                body = mine
            } else if search.hasPrefix("<@UME>") {
                body = mentions ?? none
            } else if search.hasPrefix("to:me") {
                body = direct ?? none
            } else if search.hasPrefix("from:<@UME>") {
                body = sent ?? none
            } else {
                body = none
            }
        }
        return (Data(body.utf8), HTTPURLResponse(url: url, statusCode: 200, httpVersion: nil, headerFields: nil)!)
    }
}

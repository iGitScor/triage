import Foundation
import Testing
import RemoraCore
@testable import RemoraPlugins

struct SlackPluginTests {
    @Test func cleansMrkdwn() {
        let text = "<@UME> see <https://x.io|the doc> &amp; ping <@U2|erin> in <#C1|general> <!here>"
        #expect(SlackText.plain(text, me: "UME") == "@you see the doc & ping @erin in #general @here")
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

    @Test func reportsSlackErrors() async throws {
        let http = StubHTTP(routes: ["/auth.test": #"{"ok": false, "error": "invalid_auth"}"#])
        await #expect(throws: HTTPError.unauthorized) {
            try await SlackPlugin(config: config(["token": "bad"]), http: http).fetch()
        }
    }

    @Test func appManifestLinkRequestsSearchScope() throws {
        let url = try #require(SlackPlugin.appManifestURL)
        let items = URLComponents(url: url, resolvingAgainstBaseURL: false)?.queryItems ?? []
        #expect(items.first { $0.name == "new_app" }?.value == "1")
        let manifest = try #require(items.first { $0.name == "manifest_json" }?.value)
        let json = try JSONSerialization.jsonObject(with: Data(manifest.utf8)) as? [String: Any]
        let scopes = ((json?["oauth_config"] as? [String: Any])?["scopes"] as? [String: Any])?["user"] as? [String]
        #expect(scopes == ["search:read"])
    }
}

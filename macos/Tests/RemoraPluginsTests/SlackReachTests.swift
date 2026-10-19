import Foundation
import RemoraCore
import Testing

@testable import RemoraPlugins

/// How directly a Slack message reached you, and a person who wrote again. As Windows.
struct SlackReachTests {
    let slack = SlackPluginTests()
    typealias S = SlackPluginTests

    @Test func directGroupAndMention() async throws {
        let group =
            #"{"ts": "1700000300.0", "text": "Can you?", "user": "U4", "username": "U4", "channel": {"id": "G1", "is_mpim": true}}"#
        let items = try await slack.items(
            mentions: S.matches(S.channel("1700000000.0", "<@UME> can you check?", from: "U2")),
            direct: S.matches(S.dm("1700000100.0", "Can you send it?", from: "U3"), group))
        #expect(items.first { $0.id.hasSuffix("/D1") }?.reach == .direct)
        #expect(items.first { $0.id.hasSuffix("/G1") }?.reach == .group)
        #expect(items.first { $0.id.contains("/C9/") }?.reach == .mention)
    }

    /// Two messages from them since your last one: followed up. One, or yours in between: not.
    @Test func aSecondMessageSinceYourLastIsAFollowUp() async throws {
        let first = S.dm("1700000100.0", "Can you send the deck?", from: "U2")
        let again = S.dm("1700000900.0", "Any news on the deck?", from: "U2")
        let yours = S.dm("1700000500.0", "Tomorrow", from: "UME")
        #expect(try await slack.items(direct: S.matches(first, again))[0].followedUp == true)
        #expect(try await slack.items(direct: S.matches(again))[0].followedUp == nil)
        #expect(try await slack.items(direct: S.matches(first, again), sent: S.matches(yours))[0].followedUp == nil)
        let mentions = S.matches(
            S.channel("1700000000.0", "<@UME> can you check the logs?", from: "U2"),
            S.channel("1700003000.0", "<@UME> logs, please?", from: "U2"))
        let both = try await slack.items(mentions: mentions)
        #expect(both.filter { $0.followedUp == true }.map(\.title) == ["@you logs, please?"])
    }

    /// The order of To reply, and what its rows say: a follow-up whose last message closes the conversation lifts
    /// nothing, and only a one-to-one message or a mention says it has waited a day.
    @Test func followedUpFirstThenByReach() {
        let now = Date(timeIntervalSince1970: 1_800_000_000)
        func message(_ id: String, _ reach: MessageReach?, followedUp: Bool = false, text: String = "Can you?")
            -> InboxItem
        {
            var item = InboxItem(
                id: id, accountID: UUID(), pluginID: "slack", bundle: .directMessages, title: text, context: "x",
                date: now.addingTimeInterval(-2 * .day))
            item.reach = reach
            item.followedUp = followedUp ? true : nil
            return VerbClassifier().classify(item, now: now)
        }
        let items = [
            message("thread", .thread), message("group", .group), message("own", .ownThread),
            message("other tool", nil), message("direct", .direct), message("again", .thread, followedUp: true),
        ]
        #expect(
            Prioritizer(now: now).sorted(items).map(\.id) == [
                "again", "direct", "other tool", "own", "group", "thread",
            ])
        #expect(items[5].hasBadge(.followedUp) && items[4].hasBadge(.age) && !items[1].hasBadge(.age))
        let closed = message("closed", .direct, followedUp: true, text: "Thanks, all good!")
        #expect(closed.bundle != .reply && closed.followedUp == nil && !closed.hasBadge(.followedUp))
    }
}

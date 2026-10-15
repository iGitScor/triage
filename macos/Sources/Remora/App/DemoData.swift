import Foundation
import RemoraCore

/// Sample items for `--demo`, so the app can be explored without connecting anything. In the Mac’s language.
enum DemoData {
    /// In the Mac's language, as Claude writes a real brief (the French screenshots show it).
    static let brief = Brief(
        summary: L(
            "Two reviews are blocking teammates and one of your PRs needs changes. Bob is waiting on you for Friday's rollout."
        ),
        focus: [
            .init(id: "demo/1", reason: L("Erin is blocked; checks are green, small diff.")),
            .init(id: "demo/3", reason: L("Direct question with a Friday deadline.")),
            .init(id: "demo/6", reason: L("Frank requested changes on your PR.")),
        ]
    )

    static func items() -> [UUID: [InboxItem]] {
        let account = UUID()
        func person(_ name: String, _ tone: Tone? = nil) -> Person {
            Person(name: name, avatarURL: URL(string: "https://i.pravatar.cc/64?u=\(name)"), tone: tone)
        }
        func item(
            _ n: Int, _ bundle: InboxBundle, _ title: String, _ context: String, minutes: Double,
            author: String, badges: [Badge] = [], people: [Person] = [], preview: String? = nil,
            plugin: String = "github"
        ) -> InboxItem {
            InboxItem(
                id: "demo/\(n)", accountID: account, pluginID: plugin, bundle: bundle, title: title,
                context: context, preview: preview, url: URL(string: "https://github.com"),
                author: person(author), participants: people, badges: badges,
                date: .now.addingTimeInterval(-minutes * 60),
                needsAction: bundle != .authored || badges.contains { ["approved", "changes"].contains($0.id) }
            )
        }
        let approved = Badge(id: "approved", label: L("Approved"), symbol: "checkmark", tone: .accent)
        let passing = Badge(id: "checks.passing", label: L("Checks"), symbol: "checkmark.circle.fill", tone: .positive)
        let failing = Badge(
            id: "checks.failing", label: L("Checks failed"), symbol: "xmark.octagon.fill", tone: .negative)
        let changes = Badge(
            id: "changes", label: L("Changes requested"), symbol: "exclamationmark.bubble", tone: .negative)
        let draft = Badge(id: "draft", label: L("Draft"), symbol: "pencil", tone: .neutral)
        func diff(_ a: Int, _ d: Int) -> Badge { Badge(id: "diff", label: "+\(a) −\(d)", tone: .neutral) }

        let passkeyFiles = ChangeSet(
            files: [
                ChangedFile(path: "src/auth/passkeys/register.ts", additions: 120, deletions: 12),
                ChangedFile(path: "src/auth/passkeys/register.test.ts", additions: 70, deletions: 0),
                ChangedFile(path: "db/migrations/20261008_passkeys.sql", additions: 24, deletions: 0),
            ], fileCount: 4)
        let queueFiles = ChangeSet(
            files: ["workers/images/consumer.rb", "workers/images/queue.rb", "config/sqs.yml"].map {
                ChangedFile(path: $0)
            })
        var items: [InboxItem] = [
            item(
                1, .reviews, L("Add passkeys to the sign-in page"), "acme/web #482", minutes: 12,
                author: "erin", badges: [passing, diff(214, 37)], people: [person("you"), person("frank")]),
            item(
                2, .reviews, L("Move the image resizer to the new queue"), "acme/platform !1290", minutes: 95,
                author: "dave", badges: [failing, diff(88, 120)], people: [person("you")], plugin: "gitlab"),
            item(
                3, .mentions, L("Can you double check the rollout plan before Friday?"), "#releases", minutes: 30,
                author: "Bob", preview: L("@you the flag is at 20% in prod, metrics look good so far…"), plugin: "slack"
            ),
            item(
                4, .directMessages, L("Lunch tomorrow? 🍜"), L("Direct message"), minutes: 140,
                author: "Carol", preview: L("There’s a new ramen place near the office"), plugin: "slack"),
            item(
                5, .authored, L("Snooze reminders for review requests"), "acme/inbox #77", minutes: 8,
                author: "you", badges: [approved, passing, diff(320, 12)],
                people: [person("erin", .accent), person("dave", .accent)]),
            item(
                6, .authored, L("Cache GraphQL responses per account"), "acme/inbox #75", minutes: 400,
                author: "you", badges: [changes, passing], people: [person("frank", .negative)]),
            item(
                7, .authored, L("WIP: Notion tasks bundle"), "acme/inbox #79", minutes: 1_500,
                author: "you", badges: [draft]),
            item(
                8, .tasks, L("Write Q4 hiring scorecard"), L("Team tasks · Due Fri"), minutes: 2_000,
                author: "Notion", plugin: "notion"),
            item(
                9, .reviews, L("WEB-42 retry failed uploads"), "acme/web #490", minutes: 300,
                author: "erin", badges: [passing, diff(64, 10)]),
            item(10, .reviews, L("WEB-42 alert on upload failures"), "acme/web #491", minutes: 280, author: "frank"),
            item(
                11, .reviews, L("Refactor the search service"), "acme/search #77", minutes: 900, author: "dave",
                badges: [diff(820, 410)]),
            item(
                12, .tasks, L("Update the onboarding checklist"), L("Team tasks"), minutes: 40_000,
                author: "Notion", plugin: "notion"),
            item(
                13, .authored, L("Show snooze reasons in notifications"), "acme/inbox #81", minutes: 3_000,
                author: "you", badges: [passing, diff(96, 14)], people: [person("erin"), person("bob")]),
            item(
                14, .authored, L("Group reminders by day"), "acme/inbox #82", minutes: 90,
                author: "you", badges: [passing, diff(140, 22)]),
        ]
        items[13].suggestedPeople = [person("dave")]
        // Reactions as Slack shows them, and a question of yours waiting, acknowledged with a 👍.
        items[2].reactions = [Reaction(name: "eyes", count: 2, mine: false)]
        items[3].reactions = [Reaction(name: "+1", count: 2, mine: true)]
        var question = item(
            15, .awaiting, L("Can someone proofread the new onboarding copy?"), L("Thread in %@", "#design"),
            minutes: 55, author: "you", plugin: "slack")
        question.needsAction = false
        question.reactions = [Reaction(name: "+1", count: 1, mine: false)]
        items.append(question)
        items[0].changes = passkeyFiles
        items[1].changes = queueFiles
        return [account: items]
    }

    /// Snoozed items 9–12 all come back Monday 9:00 (a pile-up); #11 was snoozed three times (a loop).
    static func snoozes(items: [InboxItem], now: Date = .now) -> (states: [String: ItemState], history: [SnoozeRecord])
    {
        let clock = SnoozeClock()
        let monday =
            clock.presets(now: now).first { $0.symbol == "calendar" }?.date ?? now.addingTimeInterval(3 * .day)
        let snoozedIDs = ["demo/9", "demo/10", "demo/11", "demo/12"]
        var states: [String: ItemState] = [:]
        var history: [SnoozeRecord] = []
        for item in items where snoozedIDs.contains(item.id) {
            let reason: SnoozeReason = item.id == "demo/11" ? .motivation : .noTime
            states[item.id] = ItemState(
                snooze: Snooze(until: monday, mode: .hide, fingerprint: item.fingerprint, reason: reason))
            let times = item.id == "demo/11" ? 3 : 1
            for day in 0..<times {
                history.append(
                    SnoozeRecord(
                        item: item, reason: reason, at: now.addingTimeInterval(Double(-day) * .day), until: monday))
            }
        }
        return (states, history)
    }
}

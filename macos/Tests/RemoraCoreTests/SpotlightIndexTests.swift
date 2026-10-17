import Foundation
import Testing

@testable import RemoraCore

/// What goes to Spotlight, and what leaves it.
struct SpotlightIndexTests {
    let account = UUID()
    let names = ["github": "GitHub", "slack": "Slack", "reminders": "Reminder"]

    func item(
        _ id: String, _ pluginID: String = "github", title: String = "Fix the export", context: String = "acme/api"
    )
        -> InboxItem
    {
        var item = InboxItem(
            id: id, accountID: account, pluginID: pluginID, bundle: .reviews, title: title, context: context,
            date: .now)
        item.preview = "What was written in the message"
        item.author = Person(name: "Erin")
        return item
    }

    func entries(_ items: [InboxItem], enabled: Bool = true, hidden: Set<String> = []) -> [SpotlightEntry] {
        SpotlightIndex.entries(items: items, enabled: enabled, hiddenContent: hidden, sourceNames: names)
    }

    @Test func onlyTheTitleAndWhereItComesFrom() {
        let entry = entries([item("1")])
        #expect(
            entry == [
                SpotlightEntry(
                    id: "1", domain: account.uuidString, title: "Fix the export", context: "GitHub · acme/api")
            ])
        #expect(entries([item("r", "reminders", title: "Call Erin", context: "Reminder")]).first?.context == "Reminder")
        #expect(entries([item("r", "reminders")]).first?.domain == SpotlightIndex.remindersDomain)
    }

    @Test func nothingWhenOffNorFromAToolWhoseContentIsHidden() {
        #expect(entries([item("1")], enabled: false).isEmpty)
        let shown = entries([item("1"), item("2", "slack")], hidden: ["slack"])
        #expect(shown.map(\.id) == ["1"])
        #expect(entries([item("1", title: "  ")]).isEmpty, "no empty title")
        #expect(entries([item("1"), item("1")]).count == 1, "each item once")
    }

    @Test func aLaunchStartsTheIndexOver() {
        let wanted = entries([item("1")])
        #expect(
            SpotlightIndex.changes(indexed: nil, wanted: wanted)
                == SpotlightIndex.Changes(deleteAll: true, index: wanted))
        #expect(SpotlightIndex.changes(indexed: nil, wanted: []) == SpotlightIndex.Changes(deleteAll: true))
    }

    @Test func onlyWhatChangedIsSent() {
        let before = entries([item("1"), item("2"), item("3")])
        let indexed = Dictionary(uniqueKeysWithValues: before.map { ($0.id, $0) })
        let after = entries([item("1"), item("2", title: "Fix the import"), item("4")])
        let changes = SpotlightIndex.changes(indexed: indexed, wanted: after)
        #expect(changes.delete == ["3"])
        #expect(changes.index.map(\.id) == ["2", "4"])
        #expect(!changes.deleteAll)
        #expect(SpotlightIndex.changes(indexed: indexed, wanted: before).isEmpty)
    }

    @Test func turnedOffEverythingGoes() {
        let indexed = Dictionary(uniqueKeysWithValues: entries([item("1")]).map { ($0.id, $0) })
        #expect(SpotlightIndex.changes(indexed: indexed, wanted: []) == SpotlightIndex.Changes(deleteAll: true))
        #expect(SpotlightIndex.changes(indexed: [:], wanted: []).isEmpty)
    }
}

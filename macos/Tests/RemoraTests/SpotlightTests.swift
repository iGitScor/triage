import Foundation
import RemoraCore
import Testing

@testable import Remora

/// Inbox titles in Spotlight only while the user wants them, and out again on every way out.
@MainActor
struct SpotlightTests {
    @Test func nothingGoesToSpotlightUntilTurnedOn() async throws {
        let harness = Harness()
        let model = try await harness.connected()
        model.start()
        #expect(harness.spotlight.entries.isEmpty)

        model.preferences.spotlight = true
        let titles = harness.spotlight.entries.values.map(\.title).sorted()
        #expect(titles == ["Fix CSV export", "Polish onboarding"])
        #expect(harness.spotlight.entries.values.allSatisfy { $0.context.hasPrefix("Linear") })

        model.preferences.spotlight = false
        #expect(harness.spotlight.entries.isEmpty)
    }

    @Test func remindersFollow() async throws {
        let harness = Harness()
        let model = try await harness.connected()
        model.preferences.spotlight = true
        model.addReminder("Call Erin", at: .now.addingTimeInterval(3_600))
        #expect(harness.spotlight.entries.values.contains { $0.title == "Call Erin" })
        let reminder = try #require(model.reminders.first)
        model.toggleDone(reminder)
        #expect(!harness.spotlight.entries.values.contains { $0.title == "Call Erin" }, "a reminder done is gone")
    }

    @Test func hiddenContentStaysOut() async throws {
        var harness = Harness()
        let model = try await harness.connected()
        model.preferences.spotlight = true
        model.preferences.hiddenContentPlugins = ["linear"]
        #expect(harness.spotlight.entries.isEmpty)
        model.preferences.hiddenContentPlugins = []
        #expect(harness.spotlight.entries.count == 2)

        // The organization hides Linear's content: the next launch indexes none of it.
        harness.managed = ManagedDictionary(values: ["HiddenContentSources": ["linear"]])
        let managed = harness.model()
        managed.start()
        #expect(harness.spotlight.entries.isEmpty)
    }

    @Test func disconnectingBlockingOrErasingRemovesIt() async throws {
        var harness = Harness()
        let model = try await harness.connected()
        model.preferences.spotlight = true
        let account = try #require(model.accounts.first)
        model.disconnect(account)
        #expect(harness.spotlight.deletedDomains == [account.id.uuidString])
        #expect(harness.spotlight.entries.isEmpty)

        let again = try await harness.connected()
        again.start()
        #expect(harness.spotlight.entries.count == 2)
        harness.managed = ManagedDictionary(values: ["AllowedPlugins": ["github"]])
        let blocked = harness.model()
        blocked.start()
        #expect(harness.spotlight.entries.isEmpty, "a blocked tool leaves Spotlight")

        let erased = harness.spotlight.deletedAll
        try again.eraseLocalData()
        #expect(harness.spotlight.deletedAll > erased)
        #expect(!again.preferences.spotlight, "off again after Erase")
    }

    @Test func aSpotlightHitOpensTheItem() async throws {
        let harness = Harness()
        let model = try await harness.connected()
        let item = try #require(model.allItems.first)
        #expect(model.openFromNotification(item.id))
        #expect(harness.outside.opened.count == 1)
        #expect(!model.openFromNotification("gone"))
    }

    @Test func theDemoIndexesNothing() {
        var harness = Harness()
        harness.demo = true
        let model = harness.model()
        model.preferences.spotlight = true
        model.start()
        #expect(harness.spotlight.entries.isEmpty)
        #expect(harness.spotlight.deletedAll == 0)
    }
}

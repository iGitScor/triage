import Foundation
import Testing
import RemoraCore
@testable import Remora

/// The layout is computed once per change, and the clock moves it only when that changes something.
@MainActor
struct LayoutCacheTests {
    @Test func timeMovesTheLayoutOnlyAtBoundaries() {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = TimeZone(identifier: "Europe/Paris")!
        let morning = Date(timeIntervalSince1970: 1_791_615_600) // 2026-10-10 09:00, Paris
        var event = InboxItem(id: "e", accountID: UUID(), pluginID: "slack", bundle: .reminders, title: "Standup", context: "Slack", date: morning)
        event.expires = morning.addingTimeInterval(3_600)
        #expect(!InboxModel.layoutTimeChanged(from: morning, to: morning.addingTimeInterval(600), items: [event], calendar: calendar))
        #expect(InboxModel.layoutTimeChanged(from: morning, to: morning.addingTimeInterval(3_700), items: [event], calendar: calendar), "the event started")
        #expect(InboxModel.layoutTimeChanged(from: morning, to: morning.addingTimeInterval(86_400), items: [], calendar: calendar), "a new day")
    }

    @Test func aQuietTickLeavesTheLayoutAlone() async throws {
        let harness = Harness()
        let model = try await harness.connected()
        _ = model.layout
        let version = model.layoutVersion
        model.tick(at: model.now.addingTimeInterval(1))
        #expect(model.layoutVersion == version, "nothing to recompute three times a minute")

        let item = try #require(model.allItems.first { $0.context == "ENG-42" })
        model.toggleDone(item)
        #expect(model.layoutVersion != version)
        #expect(model.layout.done.map(\.id) == [item.id], "a change shows at once")
    }

    @Test func aSnoozeEndingBringsTheItemBack() async throws {
        let harness = Harness()
        let model = try await harness.connected()
        let item = try #require(model.allItems.first { $0.context == "ENG-42" })
        model.snooze(item, until: .now.addingTimeInterval(60), mode: .hide, note: nil)
        #expect(model.layout.snoozed.map(\.id) == [item.id])
        model.tick(at: .now.addingTimeInterval(120))
        #expect(model.layout.snoozed.isEmpty)
    }
}

/// Snooze insights are worked out off the main thread, and still arrive.
@MainActor
struct InsightsTests {
    @Test func aLoopIsFoundInTheBackground() async throws {
        let harness = Harness()
        let model = try await harness.connected()
        let item = try #require(model.allItems.first { $0.context == "ENG-42" })
        for _ in 0..<3 { model.snooze(item, until: .now.addingTimeInterval(3_600), mode: .hide, note: nil) }
        model.refreshInsights()
        await model.insightsTask?.value
        #expect(model.insights.contains { if case .loop(let looped, _) = $0 { looped.id == item.id } else { false } })
    }
}

/// What the privacy policy refuses leaves the screen and the disk at once.
@MainActor
struct PolicyEnforcementTests {
    @Test func aRefusedToolsContentGoesAtOnce() async throws {
        let harness = Harness()
        let model = try await harness.connected()
        let account = try #require(model.accounts.first)
        #expect(model.allItems.count == 2)

        model.preferences.allowedPlugins = ["github"]
        #expect(model.allItems.isEmpty, "gone from the screen")
        #expect(!harness.writtenText().contains("Fix CSV export"), "and from the cache on disk")
        #expect(harness.notifier.removedPrefixes.contains("\(account.id.uuidString)/"))
        #expect(model.errors[account.id]?.message.contains("Remove the account") == true)
        #expect(model.accounts.map(\.id) == [account.id], "the account stays, to remove its token")

        let before = harness.server.requests
        await model.refresh(manual: true)
        #expect(harness.server.requests == before, "a refused tool isn't asked")

        model.preferences.allowedPlugins = nil
        await model.refresh(manual: true)
        #expect(model.allItems.count == 2, "back once allowed")
        #expect(model.errors[account.id] == nil)
    }
}


/// A review started then done records how long it really took.
@MainActor
struct ReviewTimingTests {
    @Test func aTimedReviewIsRemembered() async throws {
        let harness = Harness()
        let model = try await harness.connected()
        var review = InboxItem(id: "r", accountID: try #require(model.accounts.first).id, pluginID: "github", bundle: .reviews,
                               title: "Retry", context: "acme/app #1", date: .now)
        review.changes = ChangeSet(files: [ChangedFile(path: "src/retry.ts", additions: 40, deletions: 0)])
        model.start(review)
        model.toggleDone(review)
        #expect(model.reviewTimings.count == 1)
        #expect(harness.model().reviewTimings.count == 1, "saved")
        #expect(model.reviewPace == 1, "not before five")
    }
}

/// A long task title doesn't widen the menu.
@MainActor
struct MenuTitleTests {
    @Test func longTitlesAreCutAtAWord() {
        #expect(StatusItemController.menuTitle("Fix CSV export") == "Fix CSV export")
        let long = "Can you look at the deploy that failed last night on staging, the migration step timed out again"
        let title = StatusItemController.menuTitle(long)
        #expect(title.count <= 61 && title.hasSuffix("…"))
        #expect(title == "Can you look at the deploy that failed last night on…")
        #expect(StatusItemController.menuTitle("First line\nsecond line") == "First line second line", "one line")
    }
}


/// A reminder's context is stored in English and translated when shown.
@MainActor
struct ShownContextTests {
    @Test func aReminderSaysReminderInTheAppsLanguage() async throws {
        let harness = Harness()
        let model = harness.model()
        model.addReminder("Call the bank", at: .now.addingTimeInterval(60))
        let reminder = try #require(model.reminders.first)
        #expect(reminder.context == "Reminder", "stored as written, whatever the language")
        #expect(reminder.shownContext == L("Reminder"))
        let github = InboxItem(id: "g", accountID: UUID(), pluginID: "github", bundle: .reviews, title: "x", context: "Reminder", date: .now)
        #expect(github.shownContext == "Reminder", "only a reminder's is translated: a repo may be called that")
    }
}

/// No text under 11 pt, and the chosen text size applies at once and after a relaunch.
@MainActor
struct TextSizeTests {
    @Test func theTextSizeIsAppliedAndKept() {
        let harness = Harness()
        let model = harness.model()
        #expect(Myna.textScale == 1 && Myna.smallestText == 11)
        model.preferences.textSize = .larger
        #expect(Myna.textScale == 1.3)
        _ = harness.model()
        #expect(Myna.textScale == 1.3, "read back at launch")
        model.preferences.textSize = .standard
        #expect(Myna.textScale == 1)
    }
}

/// A press without a mouse-down (VoiceOver, Switch Control) opens the inbox instead of waiting for a drag.
@MainActor
struct AssistivePressTests {
    @Test func onlyAMouseDownStartsADrag() {
        #expect(StatusItemController.canStartDrag(.leftMouseDown))
        #expect(!StatusItemController.canStartDrag(nil), "an accessibility press has no event")
        #expect(!StatusItemController.canStartDrag(.keyDown))
        #expect(!StatusItemController.canStartDrag(.leftMouseUp))
    }
}

/// The rows' lookups come from caches that follow every change.
@MainActor
struct RowLookupTests {
    @Test func theIndexFollowsTheInbox() async throws {
        let harness = Harness()
        let model = try await harness.connected()
        let item = try #require(model.allItems.first { $0.context == "ENG-42" })
        #expect(model.item(item.id)?.title == item.title)
        model.addReminder("Call the bank", at: .now.addingTimeInterval(60))
        let reminder = try #require(model.reminders.first)
        #expect(model.item(reminder.id) != nil, "a new item is found at once")
        model.disconnect(try #require(model.accounts.first))
        #expect(model.item(item.id) == nil, "a removed one is gone")
    }

    @Test func snoozeCountsFollowTheHistory() async throws {
        let harness = Harness()
        let model = try await harness.connected()
        let item = try #require(model.allItems.first { $0.context == "ENG-42" })
        #expect(model.snoozeCount(item) == 0)
        model.snooze(item, until: .now.addingTimeInterval(3_600), mode: .hide, note: nil)
        #expect(model.snoozeCount(item) == 1)
        model.snooze(item, until: .now.addingTimeInterval(7_200), mode: .hide, note: nil)
        #expect(model.snoozeCount(item) == 2)
    }
}

/// A corrupt accounts file is kept aside and named in Settings, not replaced by "no accounts".
@MainActor
struct CorruptStorageTests {
    @Test func aCorruptAccountsFileIsSaidAndKept() async throws {
        let harness = Harness()
        _ = try await harness.connected()
        let accounts = harness.folder.appending(path: "accounts.json")
        try Data("{not json".utf8).write(to: accounts)
        let model = harness.model()
        #expect(model.accounts.isEmpty)
        #expect(model.storageIssues.map(\.file) == ["accounts.json"])
        #expect(model.storageIssues.first?.message.contains("accounts.json.corrupt") == true)
        #expect(FileManager.default.fileExists(atPath: harness.folder.appending(path: "accounts.json.corrupt").path), "kept, for support or by hand")
    }
}

import Foundation
import RemoraCore
import RemoraPlugins
import Testing

@testable import Remora

/// The layout is computed once per change, and the clock moves it only when that changes something.
@MainActor
struct LayoutCacheTests {
    @Test func timeMovesTheLayoutOnlyAtBoundaries() {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = TimeZone(identifier: "Europe/Paris")!
        let morning = Date(timeIntervalSince1970: 1_791_615_600)  // 2026-10-10 09:00, Paris
        var event = InboxItem(
            id: "e", accountID: UUID(), pluginID: "slack", bundle: .reminders, title: "Standup", context: "Slack",
            date: morning)
        event.expires = morning.addingTimeInterval(3_600)
        #expect(
            !InboxModel.layoutTimeChanged(
                from: morning, to: morning.addingTimeInterval(600), items: [event], calendar: calendar))
        #expect(
            InboxModel.layoutTimeChanged(
                from: morning, to: morning.addingTimeInterval(3_700), items: [event], calendar: calendar),
            "the event started")
        #expect(
            InboxModel.layoutTimeChanged(
                from: morning, to: morning.addingTimeInterval(86_400), items: [], calendar: calendar), "a new day")
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
        #expect(
            model.snoozes.insights.contains {
                if case .loop(let looped, _) = $0 { looped.id == item.id } else { false }
            })
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
        #expect(model.sync.errors[account.id]?.message.contains("Remove the account") == true)
        #expect(model.accounts.map(\.id) == [account.id], "the account stays, to remove its token")

        let before = harness.server.requests
        await model.refresh(manual: true)
        #expect(harness.server.requests == before, "a refused tool isn't asked")

        model.preferences.allowedPlugins = nil
        await model.refresh(manual: true)
        #expect(model.allItems.count == 2, "back once allowed")
        #expect(model.sync.errors[account.id] == nil)
    }
}

/// The organization's AI server replaces the user's, and decides whether the assistant runs.
@MainActor
struct AIServerTests {
    @Test func theOrganizationsServerWins() {
        var harness = Harness()
        harness.managed.values = [
            "AIServer": "https://acme.openai.azure.com/openai/v1",
            "AllowedAIServers": ["https://acme.openai.azure.com/openai/"], "AllowExternalAI": true,
        ]
        let model = harness.model()
        let account = Account(pluginID: "openai", name: nil, settings: ["host": "https://api.openai.com/v1"])
        #expect(model.privacy.effective(account).settings["host"] == "https://acme.openai.azure.com/openai/v1")
        #expect(model.privacy.refusal(for: account) == nil)
        let manifest = OpenAIPlugin.manifest
        #expect(model.privacy.allowedHosts(for: account, manifest: manifest) == ["acme.openai.azure.com"])

        harness.managed.values["AIServer"] = "https://api.openai.com/v1"
        #expect(
            harness.model().privacy.refusal(for: account) == L("Your organization doesn’t allow this AI server."),
            "a forced server outside the list")
    }

    @Test func aLocalServerWorksWithExternalAIOff() {
        let harness = Harness()
        let model = harness.model()
        model.preferences.allowExternalAI = false
        let local = Account(pluginID: "openai", name: nil, settings: ["host": "http://localhost:11434/v1"])
        let cloud = Account(pluginID: "openai", name: nil, settings: ["host": "https://api.openai.com/v1"])
        #expect(model.privacy.refusal(for: local) == nil)
        #expect(model.privacy.refusal(for: cloud) != nil)
    }
}

/// Avatars come only from an allowed tool's hosts, over https, while images are allowed; GitLab's include Gravatar.
@MainActor
struct AvatarHostTests {
    @Test func gitLabAvatarsMayComeFromGravatar() throws {
        let harness = Harness()
        let accounts = [
            Account(pluginID: "gitlab", name: nil, settings: ["host": "https://gitlab.acme.io"]),
            Account(pluginID: "linear", name: nil, settings: [:]),
        ]
        try JSONEncoder().encode(accounts).write(to: harness.folder.appending(path: "accounts.json"))
        let model = harness.model()
        #expect(model.allowsImage(URL(string: "https://gitlab.acme.io/uploads/-/system/user/avatar/1/a.png")))
        #expect(model.allowsImage(URL(string: "https://secure.gravatar.com/avatar/abc?s=80")))
        #expect(!model.allowsImage(URL(string: "https://evil.example.com/a.png")))
        #expect(!model.allowsImage(URL(string: "http://gitlab.acme.io/a.png")), "https only")

        let linearOnly = Harness()
        try JSONEncoder().encode([accounts[1]]).write(to: linearOnly.folder.appending(path: "accounts.json"))
        #expect(!linearOnly.model().allowsImage(URL(string: "https://secure.gravatar.com/avatar/abc")))

        model.preferences.allowRemoteImages = false
        #expect(
            !model.allowsImage(URL(string: "https://secure.gravatar.com/avatar/abc")), "nothing while images are off")
    }
}

/// A review started then done records how long it really took.
@MainActor
struct ReviewTimingTests {
    @Test func aTimedReviewIsRemembered() async throws {
        let harness = Harness()
        let model = try await harness.connected()
        var review = InboxItem(
            id: "r", accountID: try #require(model.accounts.first).id, pluginID: "github", bundle: .reviews,
            title: "Retry", context: "acme/app #1", date: .now)
        review.changes = ChangeSet(files: [ChangedFile(path: "src/retry.ts", additions: 40, deletions: 0)])
        model.start(review)
        model.toggleDone(review)
        #expect(model.learning.reviewTimings.count == 1)
        #expect(harness.model().learning.reviewTimings.count == 1, "saved")
        #expect(model.learning.reviewPace == 1, "not before five")
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

    /// A tool in "hide message content" shows where the task is from, not what it says.
    @Test func aHiddenToolsTaskShowsWhereItIsFrom() {
        var task = InboxItem(
            id: "s", accountID: UUID(), pluginID: "slack", bundle: .mentions, title: "Salaries", context: "#payroll",
            date: .now)
        #expect(StatusItemController.taskLabel(task, hidden: false) == "Salaries")
        #expect(StatusItemController.taskLabel(task, hidden: true) == "Slack · #payroll")
        task.context = ""
        #expect(StatusItemController.taskLabel(task, hidden: true) == "Slack")
        let reminder = InboxItem(
            id: "r", accountID: UUID(), pluginID: "reminders", bundle: .reminders, title: "Call the bank",
            context: "Reminder", date: .now)
        #expect(StatusItemController.taskLabel(reminder, hidden: true) == L("Reminder"))
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
        let github = InboxItem(
            id: "g", accountID: UUID(), pluginID: "github", bundle: .reviews, title: "x", context: "Reminder",
            date: .now)
        #expect(github.shownContext == "Reminder", "only a reminder's is translated: a repo may be called that")
    }
}

/// No text under 11 pt, and the chosen text size applies at once and after a relaunch.
@MainActor
struct TextSizeTests {
    @Test func theTextSizeIsAppliedAndKept() {
        let harness = Harness()
        let model = harness.model()
        #expect(RemoraDesign.textScale == 1 && RemoraDesign.smallestText == 11)
        model.preferences.textSize = .larger
        #expect(RemoraDesign.textScale == 1.3)
        _ = harness.model()
        #expect(RemoraDesign.textScale == 1.3, "read back at launch")
        model.preferences.textSize = .standard
        #expect(RemoraDesign.textScale == 1)
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
        harness.outside.quit()
        let accounts = harness.folder.appending(path: "accounts.json")
        try Data("{not json".utf8).write(to: accounts)
        let model = harness.model()
        #expect(model.accounts.isEmpty)
        #expect(model.storageIssues.map(\.file) == ["accounts.json"])
        #expect(model.storageIssues.first?.message.contains("accounts.json.corrupt") == true)
        #expect(
            FileManager.default.fileExists(atPath: harness.folder.appending(path: "accounts.json.corrupt").path),
            "kept, for support or by hand")
    }
}

/// The assistant can take a minute or more; what it answers after Erase or a disconnect isn't kept.
@MainActor
struct AssistantInFlightTests {
    let item = InboxItem(
        id: "r", accountID: UUID(), pluginID: "github", bundle: .reviews, title: "Retry", context: "acme/app #1",
        date: .now)

    func connected(_ harness: Harness) throws -> (InboxModel, Account) {
        let account = Account(
            pluginID: "openai", name: nil, settings: ["host": "https://api.openai.com/v1", "model": "gpt-x"])
        try JSONEncoder().encode([account]).write(to: harness.folder.appending(path: "accounts.json"))
        harness.secrets.stored[account.id] = ["token": "sk"]
        let model = harness.model()
        model.preferences.allowExternalAI = true
        #expect(model.assistantAccount?.id == account.id)
        return (model, account)
    }

    @Test func anAnswerStillWantedIsKept() async throws {
        let harness = Harness()
        let (model, _) = try connected(harness)
        await model.makeBrief()
        await model.summarize(key: "k", topic: "Reviews", items: [item])
        #expect(model.assistant.brief?.summary == "Two reviews wait.")
        #expect(model.assistant.bundleSummaries["k"]?.text == "Two reviews wait.")
    }

    @Test func aBriefThatComesBackAfterEraseIsDropped() async throws {
        let harness = Harness()
        let (model, _) = try connected(harness)
        harness.server.whileAnswering = { await MainActor.run { try? model.eraseLocalData() } }
        await model.makeBrief()
        #expect(model.assistant.brief == nil)
        #expect(!harness.writtenText().contains("Two reviews wait."), "nothing written back")
    }

    @Test func aSummaryThatComesBackAfterADisconnectIsDropped() async throws {
        let harness = Harness()
        let (model, account) = try connected(harness)
        harness.server.whileAnswering = { await MainActor.run { model.disconnect(account) } }
        await model.summarize(key: "k", topic: "Reviews", items: [item])
        #expect(model.assistant.bundleSummaries.isEmpty)
        #expect(!harness.writtenText().contains("Two reviews wait."))
    }

    /// Every request is counted with its tokens, and the daily limit refuses the next one without sending it.
    @Test func usageIsRecordedAndLimited() async throws {
        let harness = Harness()
        let (model, _) = try connected(harness)
        await model.makeBrief()
        let today = model.assistant.usage.today()
        #expect(today.requests == 1 && today.inputTokens == 120 && today.outputTokens == 30)

        model.preferences.assistantDailyLimit = 1
        let requests = harness.server.requests
        await model.summarize(key: "k", topic: "Reviews", items: [item])
        #expect(model.assistant.bundleSummaryErrors["k"] == AssistantCoordinator.limitReached)
        #expect(harness.server.requests == requests, "nothing sent")
        model.preferences.assistantDailyLimit = 2
        await model.summarize(key: "k", topic: "Reviews", items: [item])
        #expect(model.assistant.bundleSummaries["k"] != nil)
        #expect(model.assistant.usage.thisMonth().requests == 2)
        #expect(harness.writtenText().contains("inputTokens"), "saved")
    }

    @Test func eraseForgetsTheUsage() async throws {
        let harness = Harness()
        let (model, _) = try connected(harness)
        await model.makeBrief()
        try model.eraseLocalData()
        #expect(model.assistant.usage == AssistantUsage())
        #expect(!harness.writtenText().contains("inputTokens"))
    }

    @Test func anAnswerThatComesBackOnceAIIsTurnedOffIsDropped() async throws {
        let harness = Harness()
        let (model, _) = try connected(harness)
        harness.server.whileAnswering = { await MainActor.run { model.preferences.allowExternalAI = false } }
        await model.makeBrief()
        #expect(model.assistant.brief == nil)
    }
}

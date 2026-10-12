import Foundation
import RemoraCore
import Testing

@testable import Remora

/// The inbox model on a temporary folder: never your data, tokens or notifications.
@MainActor
struct InboxModelTests {
    func issue(_ model: InboxModel, _ key: String = "ENG-42") throws -> InboxItem {
        try #require(model.allItems.first { $0.context == key })
    }

    @Test func connectingKeepsTheTokenInTheVaultOnly() async throws {
        let harness = Harness()
        let model = try await harness.connected()
        let account = try #require(model.accounts.first)
        #expect(account.identity == "Alice")
        #expect(model.allItems.count == 2)
        #expect(harness.secrets.secrets(for: account.id) == ["token": "lin_api_key"])
        #expect(!harness.writtenText().contains("lin_api_key"), "no token in the data folder")
    }

    /// Connecting the same Linear account again updates its token and keeps its states.
    @Test func connectingTheSameAccountAgainUpdatesIt() async throws {
        let harness = Harness()
        let model = try await harness.connected()
        let account = try #require(model.accounts.first)
        let item = try issue(model)
        model.togglePin(item)

        try await model.connect(pluginID: "linear", name: "Work", settings: [:], secrets: ["token": "lin_new_key"])
        #expect(model.accounts.map(\.id) == [account.id], "no second account")
        #expect(model.accounts.first?.name == "Work")
        #expect(model.allItems.count == 2, "no duplicate items")
        #expect(model.state(of: try issue(model)).pinned, "states kept")
        #expect(harness.secrets.secrets(for: account.id) == ["token": "lin_new_key"])
        #expect(harness.secrets.stored.count == 1)
    }

    @Test func sameToolOnAnotherHostIsAnotherAccount() {
        let one = Account(pluginID: "gitlab", name: nil, settings: ["host": "https://gitlab.acme.io/"])
        var saved = Account(pluginID: "gitlab", name: nil, settings: ["host": "gitlab.acme.io"])
        saved.identity = "alice"
        #expect(saved.isSame(as: one, identity: "Alice"))
        #expect(
            !saved.isSame(
                as: Account(pluginID: "gitlab", name: nil, settings: ["host": "gitlab.other.io"]), identity: "alice"))
        #expect(!saved.isSame(as: one, identity: "bob"))
        #expect(!saved.isSame(as: one, identity: ""))
    }

    @Test func aNewLaunchFindsAccountsItemsAndStates() async throws {
        let harness = Harness()
        let model = try await harness.connected()
        model.togglePin(try issue(model))
        model.addReminder("Call the bank", at: .now.addingTimeInterval(3_600))

        let relaunched = harness.model()
        #expect(relaunched.accounts.map(\.id) == model.accounts.map(\.id))
        #expect(relaunched.allItems.count == 3)
        let pinned = try issue(relaunched)
        #expect(relaunched.state(of: pinned).pinned)
        #expect(relaunched.reminders.map(\.title) == ["Call the bank"])
    }

    @Test func demoModeSavesNothing() async throws {
        var harness = Harness()
        harness.demo = true
        let model = harness.model()
        model.addReminder("Not saved", at: .now.addingTimeInterval(60))
        #expect(!harness.writtenText().contains("Not saved"))
    }

    /// From the diff: starting moves an item to In progress and cancels its pending notification; stopping puts it back.
    @Test func startAndStop() async throws {
        let harness = Harness()
        let model = try await harness.connected()
        let item = try issue(model)
        model.start(item)
        #expect(model.layout.inProgress.map(\.id) == [item.id])
        #expect(harness.notifier.cancelled.contains(item.id))
        model.stop(item)
        #expect(model.layout.inProgress.isEmpty)
        #expect(model.layout.groups.flatMap(\.items).contains { $0.id == item.id })
    }

    /// The search narrows what the inbox shows, never the count or what the assistant reads.
    @Test func searchingOnlyNarrowsWhatIsShown() async throws {
        let harness = Harness()
        let model = try await harness.connected()
        let everything = model.layout.groups.flatMap(\.items).count
        model.query = "csv"
        #expect(model.visibleLayout.groups.flatMap(\.items).map(\.context) == ["ENG-42"])
        #expect(model.layout.groups.flatMap(\.items).count == everything)
    }

    @Test func snoozingSchedulesTheReturnAndDoneCancelsIt() async throws {
        let harness = Harness()
        let model = try await harness.connected()
        let item = try issue(model)
        let until = Date.now.addingTimeInterval(3_600)
        model.snooze(item, until: until, mode: .hide, note: "after lunch")
        let scheduled = try #require(harness.notifier.scheduled.last)
        #expect(scheduled.notice.itemID == item.id && scheduled.at == until)
        #expect(scheduled.notice.body.contains("Fix CSV export"))
        #expect(model.layout.snoozed.map(\.id) == [item.id])

        model.toggleDone(item)
        #expect(harness.notifier.cancelled.last == item.id)
        #expect(model.state(of: item).snooze == nil && model.state(of: item).done != nil)
    }

    /// Hide message content: the scheduled return says where, not what.
    @Test func hiddenContentReachesNoNotification() async throws {
        let harness = Harness()
        let model = try await harness.connected()
        model.preferences.hiddenContentPlugins = ["linear"]
        let item = try issue(model)
        model.snooze(item, until: .now.addingTimeInterval(60), mode: .hide, note: "secret plan")
        let notice = try #require(harness.notifier.scheduled.last?.notice)
        #expect(!notice.body.contains("Fix CSV export") && !notice.body.contains("secret plan"))
    }

    /// The inbox acts on what the organization forced, whatever the user chose.
    @Test func forcedSettingsApplyToNotificationsAndLinks() async throws {
        var harness = Harness()
        harness.managed = ManagedDictionary(values: ["HiddenContentSources": ["linear"], "OpenInApps": false])
        let model = try await harness.connected()
        let item = try issue(model)
        model.preferences.openInApps = true
        model.open(item)
        #expect(harness.outside.opened.last?.scheme == "https", "the browser, as the organization set")
        model.snooze(item, until: .now.addingTimeInterval(60), mode: .hide, note: nil)
        #expect(harness.notifier.scheduled.last?.notice.body.contains("Fix CSV export") == false)
    }

    @Test func opensTheAppThenTheWebPage() async throws {
        let harness = Harness()
        let model = try await harness.connected()
        let item = try issue(model)
        model.open(item)
        #expect(harness.outside.opened.last?.scheme == "linear", "the installed app first")
        model.open(item, inBrowser: true)
        #expect(harness.outside.opened.last?.absoluteString == "https://linear.app/x/issue/ENG-42")
        harness.outside.installedApps = false
        model.open(item)
        #expect(harness.outside.opened.last?.scheme == "https")
    }

    /// A link that isn't a web page is never opened.
    @Test func neverOpensOtherSchemes() async throws {
        let harness = Harness()
        let model = harness.model()
        let item = InboxItem(
            id: "x", accountID: UUID(), pluginID: "linear", bundle: .tasks, title: "Bad link", context: "X",
            url: URL(string: "file:///etc/passwd"), date: .now
        )
        model.open(item, inBrowser: true)
        #expect(harness.outside.opened.isEmpty)
    }

    /// The organization's policy applies to connecting and to every refresh.
    @Test func aBlockedToolCantConnectOrRefresh() async throws {
        var harness = Harness()
        let connected = try await harness.connected()
        harness.managed = ManagedDictionary(values: ["AllowedPlugins": ["github"]])
        let blocked = harness.model()
        let before = harness.server.requests
        await #expect(throws: (any Error).self) {
            try await blocked.connect(pluginID: "linear", name: nil, settings: [:], secrets: ["token": "lin_api_key"])
        }
        await blocked.refresh(manual: true)
        #expect(harness.server.requests == before, "nothing reaches a blocked tool")
        let account = try #require(connected.accounts.first)
        #expect(blocked.errors[account.id] != nil)
    }

    @Test func aFailedRefreshKeepsTheItems() async throws {
        let harness = Harness()
        let model = try await harness.connected()
        harness.server.failing = true
        await model.refresh(manual: true)
        #expect(model.allItems.count == 2)
        #expect(model.errors.count == 1)
    }

    /// Offline, Remora doesn't ask the tools, says so once, and refreshes as soon as the network is back.
    @Test func offlineWaitsForTheNetwork() async throws {
        let harness = Harness()
        let model = try await harness.connected()
        model.start()
        harness.network.set(online: false)
        let before = harness.server.requests
        await model.refresh(manual: true)
        #expect(harness.server.requests == before, "no request while offline")
        #expect(model.sourcesHealth == .offline && model.errors.isEmpty)
        #expect(model.allItems.count == 2, "the inbox as it was")

        harness.network.set(online: true)
        try await Task.sleep(for: .milliseconds(50))
        #expect(harness.server.requests > before, "refreshed when back online")
        #expect(model.sourcesHealth == .fine)
    }

    /// A source that fails for lack of a network isn't slowed down for it, and isn't "failing".
    @Test func aNetworkFailureIsOfflineNotFailing() async throws {
        let harness = Harness()
        let model = try await harness.connected()
        harness.server.offline = true
        await model.refresh(manual: true)
        #expect(model.sourcesHealth == .offline)
        harness.server.offline = false
        let before = harness.server.requests
        await model.refresh()
        #expect(harness.server.requests > before, "no slow-down after an offline failure")
        #expect(model.sourcesHealth == .fine)
    }

    /// A rejected token asks for reconnecting, and reconnecting keeps the account.
    @Test func aRejectedTokenAsksToReconnect() async throws {
        let harness = Harness()
        let model = try await harness.connected()
        let account = try #require(model.accounts.first)
        harness.server.status = 401
        await model.refresh(manual: true)
        #expect(model.sourcesHealth == .reconnect([account.id]))
        #expect(model.errors[account.id]?.kind == .auth)

        harness.server.status = nil
        try await model.connect(
            pluginID: "linear", name: nil, settings: account.settings, secrets: ["token": "lin_new_key"])
        #expect(model.accounts.map(\.id) == [account.id], "the same account")
        #expect(model.sourcesHealth == .fine)
    }

    /// A new token any time, not only after a rejection: the account and its states stay.
    @Test func updatingTheTokenKeepsTheAccountAndItsStates() async throws {
        let harness = Harness()
        let model = try await harness.connected()
        let account = try #require(model.accounts.first)
        let item = try #require(model.allItems.first)
        model.togglePin(item)

        try await model.connect(
            pluginID: "linear", name: nil, settings: account.settings, secrets: ["token": "lin_new_key"])
        #expect(model.accounts.map(\.id) == [account.id], "the same account")
        #expect(model.state(of: item).pinned, "its pin stays")
        #expect(harness.secrets.stored[account.id]?["token"] == "lin_new_key")
    }

    /// The first sync after launch compares with the cache: what arrived while Remora was closed is announced.
    @Test func whatArrivedWhileClosedIsAnnounced() async throws {
        let harness = Harness()
        let model = try await harness.connected()
        let account = try #require(model.accounts.first)
        let arrived = try #require(model.allItems.last { $0.needsAction })
        _ = harness.model()  // Quits the first one: its files are written.
        let cache = JSONStore<[UUID: [InboxItem]]>("cache", in: harness.folder, backedUp: false)
        var items = try #require(cache.load())
        items[account.id]?.removeAll { $0.id == arrived.id }
        try cache.save(items)

        let relaunched = harness.model()
        await relaunched.refresh(manual: true)
        #expect(harness.notifier.posted.map(\.itemID) == [arrived.id])

        await relaunched.refresh(manual: true)
        #expect(harness.notifier.posted.count == 1, "later syncs compare with the last one")
    }

    /// Without a cache (a new account, or none saved), the first sync stays silent.
    @Test func aFirstSyncWithoutCacheIsSilent() async throws {
        let harness = Harness()
        _ = try await harness.connected()
        _ = harness.model()
        try FileManager.default.removeItem(at: harness.folder.appending(path: "cache.json"))
        let relaunched = harness.model()
        await relaunched.refresh(manual: true)
        #expect(harness.notifier.posted.isEmpty)
    }

    @Test func disconnectingRemovesTheTokenItemsAndNotifications() async throws {
        let harness = Harness()
        let model = try await harness.connected()
        let account = try #require(model.accounts.first)
        model.disconnect(account)
        #expect(model.accounts.isEmpty && model.allItems.isEmpty)
        #expect(harness.secrets.stored[account.id] == nil)
        #expect(harness.notifier.removedPrefixes == ["\(account.id.uuidString)/"])
        #expect(harness.model().accounts.isEmpty, "saved")
    }

    /// SEC-26: a refresh started before Erase finishes after it, and used to write the inbox back.
    @Test func aRefreshThatFinishesAfterEraseWritesNothing() async throws {
        let harness = Harness()
        let model = try await harness.connected()
        let posted = harness.notifier.posted.count
        harness.server.whileFetching = { await MainActor.run { try? model.eraseLocalData() } }
        await model.refresh(manual: true)
        #expect(model.allItems.isEmpty && model.errors.isEmpty)
        #expect(harness.writtenText().isEmpty, "nothing back on disk")
        #expect(harness.notifier.posted.count == posted)
    }

    @Test func resultsForAnAccountDisconnectedDuringARefreshAreDropped() async throws {
        let harness = Harness()
        let model = try await harness.connected()
        let account = try #require(model.accounts.first)
        harness.server.whileFetching = { await MainActor.run { model.disconnect(account) } }
        await model.refresh(manual: true)
        #expect(model.allItems.isEmpty)
        #expect(!harness.writtenText().contains("Fix CSV export"))
    }

    @Test func aToolBlockedDuringARefreshKeepsNothing() async throws {
        let harness = Harness()
        let model = try await harness.connected()
        harness.server.whileFetching = { await MainActor.run { model.preferences.allowedPlugins = ["github"] } }
        await model.refresh(manual: true)
        #expect(model.allItems.isEmpty)
        #expect(!harness.writtenText().contains("Fix CSV export"))
    }

    /// When the Keychain refuses (Deny), connecting fails and no account is added.
    @Test func aRefusedTokenFailsTheConnection() async throws {
        let harness = Harness()
        harness.secrets.refusing = true
        let model = harness.model()
        await #expect(throws: KeychainError.self) {
            try await model.connect(pluginID: "linear", name: nil, settings: [:], secrets: ["token": "lin_api_key"])
        }
        #expect(model.accounts.isEmpty && model.allItems.isEmpty)
        #expect(harness.model().accounts.isEmpty, "nothing saved")
    }

    /// A token the Keychain won't delete keeps its account, with the reason under it.
    @Test func aRefusedRemovalKeepsTheAccount() async throws {
        let harness = Harness()
        let model = try await harness.connected()
        let account = try #require(model.accounts.first)
        harness.secrets.refusing = true
        model.disconnect(account)
        #expect(model.accounts.map(\.id) == [account.id])
        #expect(model.errors[account.id] != nil)
        #expect(harness.secrets.stored[account.id] != nil)
    }

    /// Done, Mark all as done and Clear can be undone once, and a snooze's return comes back with them.
    @Test func doneMarkAllAndClearCanBeUndone() async throws {
        let harness = Harness()
        let model = try await harness.connected()
        let item = try issue(model)
        let until = Date.now.addingTimeInterval(3_600)
        model.snooze(item, until: until, mode: .hide, note: nil)
        let scheduled = harness.notifier.scheduled.count

        model.toggleDone(item)
        #expect(model.undoPoint?.message == "Marked as done")
        model.undo()
        #expect(model.state(of: item).done == nil && model.state(of: item).snooze?.until == until)
        #expect(harness.notifier.scheduled.count == scheduled + 1, "its return is scheduled again")
        #expect(model.undoPoint == nil)
        #expect(harness.model().state(of: item).snooze != nil, "saved")

        let all = model.allItems.filter { $0.bundle != .reminders }
        model.sweep(all)
        #expect(model.layout.done.count == 2)
        model.undo()
        #expect(model.layout.done.isEmpty)

        model.sweep(all)
        model.clear(model.layout.done)
        #expect(model.layout.done.isEmpty)
        model.undo()
        #expect(model.layout.done.count == 2, "back in Done, not cleared")
    }

    /// Clear all also clears what the tool put in Done (your ✅ in Slack, a finished conversation).
    @Test func clearAllClearsWhatTheToolPutInDone() throws {
        let harness = Harness()
        let account = Account(pluginID: "slack", name: nil, settings: [:])
        var handled = InboxItem(
            id: "\(account.id)/D1", accountID: account.id, pluginID: "slack", bundle: .reply, title: "Deck?",
            context: "Direct message", date: .now, needsAction: true)
        handled.reactions = [Reaction(name: "white_check_mark", count: 1, mine: true)]
        var finished = handled
        finished.id = "\(account.id)/D2"
        finished.bundle = .finished
        finished.reactions = nil
        try JSONEncoder().encode([account]).write(to: harness.folder.appending(path: "accounts.json"))
        try JSONEncoder().encode([account.id: [handled, finished]]).write(
            to: harness.folder.appending(path: "cache.json"))
        // Marked done days ago, on an older message: a stale mark doesn't keep it from clearing.
        let stale = ItemState(done: .init(at: .now, fingerprint: "0|", clearedAt: .now))
        try JSONEncoder().encode([finished.id: stale]).write(to: harness.folder.appending(path: "states.json"))
        let model = harness.model()
        #expect(model.layout.done.count == 2)

        model.clear(model.layout.done)
        #expect(model.layout.done.isEmpty)
        model.undo()
        #expect(model.layout.done.count == 2, "back in Done")
    }

    @Test func aReminderMarkedDoneComesBack() async throws {
        let harness = Harness()
        let model = harness.model()
        model.addReminder("Call the bank", at: .now.addingTimeInterval(60))
        let reminder = try #require(model.reminders.first)
        model.unsnooze(reminder)
        model.toggleDone(reminder)
        #expect(model.reminders.isEmpty)
        model.undo()
        #expect(model.reminders.map(\.title) == ["Call the bank"])
        #expect(harness.model().reminders.count == 1, "saved")
    }

    /// Done on a repeating reminder hides it until its next time instead of removing it; Undo puts it back.
    @Test func aRepeatingReminderComesBackAfterDone() async throws {
        let harness = Harness()
        let model = harness.model()
        let first = Date.now.addingTimeInterval(-60)
        model.addReminder("Stand-up", at: first, repeating: .daily)
        let reminder = try #require(model.reminders.first)
        #expect(reminder.recurrence == Recurrence(rule: .daily, anchor: first))
        model.unsnooze(reminder)
        model.toggleDone(reminder)

        let next = try #require(Recurrence(rule: .daily, anchor: first).next(after: .now))
        #expect(model.reminders.map(\.title) == ["Stand-up"], "kept")
        #expect(model.state(of: reminder).snooze?.until == next)
        #expect(model.state(of: reminder).done == nil)
        #expect(harness.notifier.scheduled.last?.at == next)
        #expect(model.undoPoint?.message.hasPrefix("Next reminder") == true)
        #expect(model.snoozeCount(reminder) == 0, "not counted as a snooze")

        model.undo()
        #expect(model.state(of: reminder).snooze == nil)
        #expect(harness.notifier.cancelled.last == reminder.id, "the next time's notification goes")
        #expect(harness.model().reminders.first?.recurrence?.rule == .daily, "saved")
    }

    /// Past its last day, or once it stops repeating, Done removes the reminder as before.
    @Test func aRepeatingReminderEnds() async throws {
        let harness = Harness()
        let model = harness.model()
        let first = Date.now.addingTimeInterval(-60)
        model.addReminder("Today only", at: first, repeating: .weekly, until: first)
        model.addReminder("Daily", at: first, repeating: .daily)
        #expect(model.reminders.count == 2)
        model.toggleDone(try #require(model.reminders.first { $0.title == "Today only" }))
        #expect(model.reminders.map(\.title) == ["Daily"])

        let daily = try #require(model.reminders.first)
        model.stopRepeating(daily)
        #expect(model.reminders.first?.recurrence == nil)
        model.toggleDone(try #require(model.reminders.first))
        #expect(model.reminders.isEmpty)
    }

    @Test func anEndBeforeTheFirstReminderIsRefused() {
        let model = Harness().model()
        model.addReminder("Never", at: .now.addingTimeInterval(86_400 * 3), repeating: .daily, until: .now)
        #expect(model.reminders.isEmpty)
    }

    @Test func anUndoOfferExpiresAndIsReplaced() async throws {
        let harness = Harness()
        let model = try await harness.connected()
        model.toggleDone(try issue(model))
        let first = try #require(model.undoPoint)
        model.toggleDone(try issue(model, "ENG-43"))
        model.forgetUndo(first.id)
        #expect(model.undoPoint != nil, "a later offer stays")
        model.forgetUndo(try #require(model.undoPoint).id)
        #expect(model.undoPoint == nil)
        model.disconnect(try #require(model.accounts.first))
        model.undo()
        #expect(model.allItems.isEmpty)
    }

    /// Erase still clears everything else when the Keychain refuses, then says so.
    @Test func eraseSaysWhenTheKeychainRefuses() async throws {
        let harness = Harness()
        let model = try await harness.connected()
        harness.secrets.refusing = true
        #expect(throws: KeychainError.self) { try model.eraseLocalData() }
        #expect(model.accounts.isEmpty && model.allItems.isEmpty)
        #expect(harness.notifier.removedAll)
    }

    /// A file Erase can't delete is named, not passed over as erased; the rest is still cleared.
    @Test func eraseNamesTheFilesItCouldNotDelete() async throws {
        let harness = Harness()
        let model = try await harness.connected()
        let locked = harness.folder.appending(path: "locked")
        try FileManager.default.createDirectory(at: locked, withIntermediateDirectories: true)
        try Data("message content".utf8).write(to: locked.appending(path: "kept.json"))
        try FileManager.default.setAttributes([.posixPermissions: 0o500], ofItemAtPath: locked.path)
        defer { try? FileManager.default.setAttributes([.posixPermissions: 0o700], ofItemAtPath: locked.path) }
        #expect(throws: EraseError(files: ["locked"], folder: harness.folder.path, keychain: nil)) {
            try model.eraseLocalData()
        }
        #expect(model.accounts.isEmpty && model.allItems.isEmpty)
        #expect(harness.secrets.erased)
        let left = try FileManager.default.contentsOfDirectory(atPath: harness.folder.path)
        #expect(left == ["locked"], "everything else is gone")
    }

    /// Erase leaves no file, token, notification or HTTP cache behind.
    @Test func eraseLeavesNothing() async throws {
        let harness = Harness()
        let model = try await harness.connected()
        model.addReminder("Call the bank", at: .now.addingTimeInterval(60))
        let cachesBefore = harness.outside.cachesRemoved
        try model.eraseLocalData()
        #expect(model.accounts.isEmpty && model.allItems.isEmpty && model.states.isEmpty)
        #expect(harness.secrets.erased && harness.secrets.stored.isEmpty)
        #expect(harness.notifier.removedAll)
        #expect(harness.outside.cachesRemoved == cachesBefore + 1)
        let left = try FileManager.default.contentsOfDirectory(atPath: harness.folder.path)
        #expect(left.isEmpty)
        let relaunched = harness.model()
        #expect(relaunched.accounts.isEmpty && relaunched.allItems.isEmpty)
    }
}

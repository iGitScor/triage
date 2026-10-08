import Foundation
import Testing
@testable import RemoraCore

struct InboxAssemblerTests {
    let assembler = InboxAssembler()

    @Test func groupsItemsByBundleRank() {
        let layout = assembler.layout(items: [makeItem("1", bundle: .authored), makeItem("2", bundle: .reviews)], states: [:], now: now)
        #expect(layout.groups.map(\.bundle) == [.reviews, .authored])
    }

    @Test func doneItemStaysDoneUntilItChanges() {
        let item = makeItem("1")
        let states = ["1": ItemState(done: .init(at: now, fingerprint: item.fingerprint))]
        #expect(assembler.placement(of: item, state: states["1"], now: now) == .done)

        var updated = item
        updated.badges = [approved]
        #expect(assembler.placement(of: updated, state: states["1"], now: now) == .inbox)
    }

    @Test func hiddenSnoozeLastsUntilItsTime() {
        let item = makeItem("1")
        let state = ItemState(snooze: Snooze(until: now + 60, mode: .hide, fingerprint: item.fingerprint))
        #expect(assembler.placement(of: item, state: state, now: now) == .snoozed)
        #expect(assembler.placement(of: item, state: state, now: now + 61) == .inbox)
    }

    @Test func activityWakesSnoozedItemOnlyWhenEnabled() {
        let item = makeItem("1")
        let state = ItemState(snooze: Snooze(until: now + 60, mode: .hide, fingerprint: "stale"))
        #expect(assembler.placement(of: item, state: state, now: now) == .inbox)
        #expect(InboxAssembler(wakeOnActivity: false).placement(of: item, state: state, now: now) == .snoozed)
    }

    @Test func remindModeKeepsItemVisible() {
        let item = makeItem("1")
        let state = ItemState(snooze: Snooze(until: now + 60, mode: .remind, fingerprint: item.fingerprint))
        #expect(assembler.placement(of: item, state: state, now: now) == .inbox)
    }

    @Test func pinnedItemsGetTheirOwnSection() {
        let layout = assembler.layout(items: [makeItem("1"), makeItem("2")], states: ["2": ItemState(pinned: true)], now: now)
        #expect(layout.pinned.map(\.id) == ["2"])
        #expect(layout.groups.flatMap(\.items).map(\.id) == ["1"])
    }

    @Test func queryFiltersOnTitleAndContext() {
        let layout = assembler.layout(items: [makeItem("1"), makeItem("22")], states: [:], now: now, query: "#22")
        #expect(layout.inboxCount == 1)
    }

    @Test func countsPerSource() {
        var slack = makeItem("3", bundle: .mentions, needsAction: true)
        slack.pluginID = "slack"
        let items = [makeItem("1", needsAction: true), makeItem("2", bundle: .authored), slack]
        let layout = assembler.layout(items: items, states: [:], now: now)
        #expect(layout.countsBySource(actionableOnly: true).map(\.count) == [1, 1])
        #expect(layout.countsBySource(actionableOnly: false).first { $0.pluginID == "test" }?.count == 2)
    }

    @Test func clearedDoneItemsLeaveTheListUntilTheyChange() {
        let item = makeItem("1")
        let state = ItemState(done: .init(at: now, fingerprint: item.fingerprint, clearedAt: now))
        #expect(assembler.layout(items: [item], states: ["1": state], now: now) == InboxLayout())

        var updated = item
        updated.badges = [approved]
        #expect(assembler.placement(of: updated, state: state, now: now) == .inbox)
    }

    @Test func splitsByWhoseTurnItIs() {
        let review = makeItem("1", bundle: .reviews, needsAction: true)
        let waitingOnOthers = makeItem("2", bundle: .authored)
        let reminder = makeItem("3", bundle: .reminders)
        let backFromSnooze = makeItem("4", bundle: .authored)
        let layout = assembler.layout(
            items: [review, waitingOnOthers, reminder, backFromSnooze],
            states: ["4": ItemState(remindedAt: now)],
            now: now
        )
        #expect(Set(layout.myTurnItems.map(\.id)) == ["1", "3", "4"])
        #expect(layout.waitingItems.map(\.id) == ["2"])
        #expect(layout.actionCount == 3)
    }

    @Test func sourcesAreOrderedByTheirMostPressingVerb() {
        var review = makeItem("1", bundle: .reviews, needsAction: true)
        review.pluginID = "github"
        var secondReview = makeItem("2", bundle: .reviews, needsAction: true)
        secondReview.pluginID = "github"
        var question = makeItem("3", bundle: .reply, needsAction: true)
        question.pluginID = "slack"
        let layout = assembler.layout(items: [review, secondReview, question], states: [:], now: now)
        #expect(layout.countsBySource(actionableOnly: true).map(\.pluginID) == ["slack", "github"], "to reply comes before to review")
    }
}

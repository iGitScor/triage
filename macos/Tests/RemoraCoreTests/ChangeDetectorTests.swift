import Testing

@testable import RemoraCore

struct ChangeDetectorTests {
    @Test func firstSyncIsSilent() {
        #expect(ChangeDetector.notices(previous: nil, current: [makeItem("1", needsAction: true)]).isEmpty)
    }

    @Test func newActionableItemIsAnnounced() {
        let notices = ChangeDetector.notices(previous: [], current: [makeItem("1", needsAction: true), makeItem("2")])
        #expect(notices.map(\.itemID) == ["1"])
        #expect(notices.first?.kind == .arrival)
    }

    @Test func newNotifyingBadgeIsAnnouncedOnce() {
        let before = [makeItem("1", bundle: .authored)]
        let after = [makeItem("1", bundle: .authored, badges: [approved])]
        #expect(ChangeDetector.notices(previous: before, current: after).map(\.title) == ["Approved"])
        #expect(ChangeDetector.notices(previous: after, current: after).isEmpty)
    }

    @Test func silentBadgesAreIgnored() {
        let quiet = Badge(id: "draft", label: "Draft", tone: .neutral)
        #expect(ChangeDetector.notices(previous: [makeItem("1")], current: [makeItem("1", badges: [quiet])]).isEmpty)
    }

    @Test func aFewArrivalsWhileClosedComeOneByOne() {
        let notices = ChangeDetector.notices(
            previous: [], current: (1...3).map { makeItem("\($0)", needsAction: true) })
        #expect(ChangeDetector.whileClosed(notices, accountID: "a", accountName: "Linear") == notices)
    }

    @Test func manyArrivalsWhileClosedBecomeOneSummary() {
        let notices = ChangeDetector.notices(
            previous: [], current: (1...5).map { makeItem("\($0)", needsAction: true) })
        let summary = ChangeDetector.whileClosed(notices, accountID: "a", accountName: "Linear")
        #expect(summary.count == 1)
        #expect(summary.first?.itemID == "a/while-closed")
        #expect(summary.first?.isSummary == true)
        #expect(summary.first?.title == "5 new while Remora was closed")
        #expect(summary.first?.subtitle == "Linear")
        #expect(summary.first?.url == nil)
        #expect(notices.first?.isSummary == false)
    }

    @Test func redactedNoticeKeepsWhereButNotWhat() {
        let notice = Notice(
            kind: .arrival, itemID: "1", title: "Mentions", subtitle: "#general", body: "Salary review moved", url: nil)
        let redacted = notice.redacted()
        #expect(redacted.title == "Mentions")
        #expect(redacted.subtitle == "#general")
        #expect(redacted.itemID == "1")
        #expect(!redacted.body.contains("Salary"))
    }
}

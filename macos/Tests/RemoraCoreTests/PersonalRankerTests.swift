import Foundation
import Testing
@testable import RemoraCore

struct PersonalRankerTests {
    func item(_ id: String, author: String, context: String = "acme/app #1") -> InboxItem {
        var item = makeItem(id, bundle: .reviews)
        item.author = Person(name: author)
        item.context = context
        return item
    }

    /// Erin's reviews get done quickly, Frank's get snoozed.
    var records: [ActionRecord] {
        (0..<12).map { ActionRecord(item: item("l\($0)", author: "erin"), outcome: .quick) }
            + (0..<12).map { ActionRecord(item: item("t\($0)", author: "frank"), outcome: .deferred) }
    }

    @Test func silentUntilEnoughData() {
        let ranker = PersonalRanker(records: Array(records.prefix(10)))
        #expect(!ranker.isTrained)
        #expect(ranker.score(item("x", author: "erin")) == 0)
    }

    @Test func learnsWhoYouHandleQuickly() {
        let ranker = PersonalRanker(records: records)
        #expect(ranker.isTrained)
        #expect(ranker.score(item("x", author: "erin")) > ranker.score(item("y", author: "frank")))
        #expect(ranker.reason(item("x", author: "erin")) == "You usually handle items from erin quickly.")
        #expect(ranker.reason(item("y", author: "frank")) == nil)
    }

    @Test func habitsBreakTiesButNeverBeatPriority() {
        let ranker = PersonalRanker(records: records)
        var urgentFromFrank = item("u", author: "frank")
        urgentFromFrank.priority = .urgent
        let fromErin = item("a", author: "erin"), fromFrank = item("b", author: "frank")
        let sorted = Prioritizer(now: now, personal: { ranker.score($0) }).sorted([fromFrank, fromErin, urgentFromFrank])
        #expect(sorted.map(\.id) == ["u", "a", "b"])
    }

    @Test func usualReasonNeedsAClearHabit() {
        let advisor = SnoozeAdvisor()
        let target = item("x", author: "erin", context: "acme/app #9")
        let mostlyWaiting = [SnoozeReason.waiting, .waiting, .waiting, .noTime].map {
            SnoozeRecord(item: item("h", author: "erin"), reason: $0, at: now, until: now + 3_600)
        }
        #expect(advisor.usualReason(for: target, history: mostlyWaiting) == .waiting)
        let mixed = [SnoozeReason.waiting, .noTime, .focus].map {
            SnoozeRecord(item: item("h", author: "erin"), reason: $0, at: now, until: now + 3_600)
        }
        #expect(advisor.usualReason(for: target, history: mixed) == nil)
    }
}

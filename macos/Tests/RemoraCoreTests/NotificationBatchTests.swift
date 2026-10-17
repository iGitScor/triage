import Foundation
import Testing

@testable import RemoraCore

/// The same cases as Windows (`batch.rs`).
struct NotificationBatchTests {
    let calendar = Calendar.current

    func at(_ day: Int, _ hour: Int, _ minute: Int) -> Date {
        calendar.date(from: DateComponents(year: 2026, month: 10, day: day, hour: hour, minute: minute))!
    }

    @Test func readsTheTimesHoweverTheyAreTyped() {
        #expect(NotificationBatch.times("17:00, 10:00;14:00 10:00 noon 9:5") == [600, 840, 1020])
        #expect(NotificationBatch.times("").isEmpty)
        #expect(NotificationBatch.times(NotificationBatch.defaultTimes).count == 3)
    }

    @Test func aSetTimeIsDueOnceItHasPassed() {
        let times = NotificationBatch.times("10:00, 17:00")
        #expect(!NotificationBatch.isDue(times, since: at(13, 9, 0), now: at(13, 9, 59)), "not yet")
        #expect(NotificationBatch.isDue(times, since: at(13, 9, 0), now: at(13, 10, 0)), "10:00 passed")
        #expect(!NotificationBatch.isDue(times, since: at(13, 10, 0), now: at(13, 16, 0)), "17:00 still to come")
        #expect(NotificationBatch.isDue(times, since: at(13, 18, 0), now: at(14, 10, 5)), "the next morning's")
        #expect(NotificationBatch.isDue(times, since: at(10, 18, 0), now: at(13, 9, 0)), "days away")
        #expect(!NotificationBatch.isDue([], since: at(13, 9, 0), now: at(13, 18, 0)), "no times: never")
    }

    @Test func severalWaitingComeAsOne() {
        let notice = { (id: String, kind: Notice.Kind) in
            Notice(kind: kind, itemID: id, title: "To review", subtitle: "", body: id, url: nil)
        }
        let few = [notice("a", .arrival), notice("b", .statusChange)]
        #expect(NotificationBatch.grouped(few) == few)
        let one = NotificationBatch.grouped(["a", "b", "c", "d"].map { notice($0, .arrival) })
        #expect(one.count == 1 && one[0].isSummary && one[0].title == L("%d new in Remora", 4))
        #expect(NotificationBatch.waits(notice("a", .statusChange)) && !NotificationBatch.waits(notice("r", .reminder)))
    }
}

import Foundation
import Testing

@testable import RemoraCore

/// The same cases as the Windows app's `recurrence` tests.
struct RecurrenceTests {
    let calendar: Calendar = {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = TimeZone(identifier: "Europe/Paris")!
        return calendar
    }()

    func at(_ day: Int, _ hour: Int, month: Int = 10, minute: Int = 0) -> Date {
        calendar.date(from: DateComponents(year: 2026, month: month, day: day, hour: hour, minute: minute))!
    }

    // 2026-10-09 is a Friday.

    @Test func dailyComesBackTheNextDayAtTheSameTime() {
        let recurrence = Recurrence(rule: .daily, anchor: at(5, 9))
        #expect(recurrence.next(after: at(8, 10), calendar: calendar) == at(9, 9))
    }

    @Test func dailyIsTodayWhenTodaysTimeIsAhead() {
        let recurrence = Recurrence(rule: .daily, anchor: at(5, 9, minute: 30))
        #expect(recurrence.next(after: at(8, 8), calendar: calendar) == at(8, 9, minute: 30))
    }

    @Test func weekdaysSkipTheWeekend() {
        let recurrence = Recurrence(rule: .weekdays, anchor: at(5, 9))
        #expect(recurrence.next(after: at(9, 10), calendar: calendar) == at(12, 9))
        #expect(recurrence.next(after: at(10, 8), calendar: calendar) == at(12, 9))
    }

    @Test func weeklyKeepsTheWeekday() {
        let recurrence = Recurrence(rule: .weekly, anchor: at(5, 9))
        #expect(recurrence.next(after: at(5, 10), calendar: calendar) == at(12, 9))
        #expect(recurrence.next(after: at(8, 10), calendar: calendar) == at(12, 9))
    }

    @Test func neverBeforeTheFirstOccurrence() {
        let recurrence = Recurrence(rule: .daily, anchor: at(12, 9))
        #expect(recurrence.next(after: at(8, 10), calendar: calendar) == at(12, 9))
    }

    @Test func theLastDayIsIncluded() {
        let recurrence = Recurrence(rule: .daily, anchor: at(5, 9), until: at(9, 0))
        #expect(recurrence.next(after: at(8, 10), calendar: calendar) == at(9, 9))
        #expect(recurrence.next(after: at(9, 10), calendar: calendar) == nil)
    }

    @Test func daylightSavingKeepsTheTimeOfDay() {
        // Clocks change in Paris on 2026-03-29.
        let recurrence = Recurrence(rule: .daily, anchor: at(27, 9, month: 3))
        #expect(recurrence.next(after: at(29, 12, month: 3), calendar: calendar) == at(30, 9, month: 3))
        #expect(calendar.component(.hour, from: at(30, 9, month: 3)) == 9)
    }

    @Test func anEndBeforeTheStartIsRefused() {
        #expect(Recurrence(rule: .daily, anchor: at(9, 9), until: at(8, 12)).endsBeforeStart(calendar: calendar))
        #expect(!Recurrence(rule: .daily, anchor: at(9, 9), until: at(9, 0)).endsBeforeStart(calendar: calendar))
    }

    @Test func roundTripsAndOldItemsHaveNone() throws {
        var item = InboxItem(
            id: "reminder/1", accountID: UUID(), pluginID: "reminders", bundle: .reminders, title: "Stand-up",
            context: "Reminder", date: at(5, 8))
        item.recurrence = Recurrence(rule: .weekdays, anchor: at(5, 9), until: at(30, 0))
        let data = try JSONEncoder().encode(item)
        #expect(try JSONDecoder().decode(InboxItem.self, from: data) == item)
        #expect(String(data: data, encoding: .utf8)!.contains("\"weekdays\""))

        var old = try JSONSerialization.jsonObject(with: data) as! [String: Any]
        old["recurrence"] = nil
        let decoded = try JSONDecoder().decode(InboxItem.self, from: JSONSerialization.data(withJSONObject: old))
        #expect(decoded.recurrence == nil)
        #expect(decoded.fingerprint == item.fingerprint)
    }
}

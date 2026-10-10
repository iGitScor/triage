import Foundation
import Testing

@testable import RemoraCore

struct SnoozeClockTests {
    let clock: SnoozeClock = {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = TimeZone(identifier: "Europe/Paris")!
        return SnoozeClock(calendar: calendar)
    }()

    @Test func scrubberGrowsFromMinutesToDays() {
        #expect(clock.duration(at: 0) == 5 * 60)
        #expect(clock.duration(at: 1) == 7 * 86_400)
        let samples = stride(from: 0.0, through: 1.0, by: 0.05).map(clock.duration(at:))
        #expect(samples == samples.sorted())
    }

    @Test func presetsAreInTheFuture() {
        let morning = clock.calendar.date(from: DateComponents(year: 2026, month: 10, day: 8, hour: 10))!
        let presets = clock.presets(now: morning)
        #expect(presets.map(\.label) == ["Later today", "This evening", "Tomorrow", "Next week"])
        #expect(presets.allSatisfy { $0.date > morning })
    }

    @Test func lateEveningSkipsSameDayPresets() {
        let night = clock.calendar.date(from: DateComponents(year: 2026, month: 10, day: 8, hour: 23))!
        #expect(clock.presets(now: night).map(\.label) == ["Tomorrow", "Next week"])
    }

    @Test func progressFollowsAnyDate() {
        #expect(clock.progress(for: 60) == 0)
        #expect(clock.progress(for: 30 * 86_400) == 1)
        for step in SnoozeClock.steps {
            #expect(abs(clock.duration(at: clock.progress(for: step)) - step) < 1, "round trip at \(step)")
        }
        let tomorrowMorning: TimeInterval = 20 * 3_600
        let progress = clock.progress(for: tomorrowMorning)
        #expect(progress > clock.progress(for: 12 * 3_600) && progress < clock.progress(for: 24 * 3_600))
    }
}

import Foundation
import Testing

@testable import RemoraCore

/// What the assistant used, per local day, and the daily limit. Same cases on Windows.
struct AssistantUsageTests {
    let calendar: Calendar = {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = TimeZone(identifier: "Europe/Paris")!
        return calendar
    }()

    func date(_ text: String) -> Date {
        ISO8601DateFormatter().date(from: text)!
    }

    @Test func recordsRequestsAndTokensPerDay() {
        var usage = AssistantUsage()
        let morning = date("2026-10-15T08:00:00+02:00")
        usage.record(TokenUsage(input: 1_000, output: 200), now: morning, calendar: calendar)
        usage.record(nil, now: morning, calendar: calendar)
        let today = usage.today(now: morning, calendar: calendar)
        #expect(today == AssistantUsage.Day(day: "2026-10-15", requests: 2, inputTokens: 1_000, outputTokens: 200))
        #expect(today.tokens == 1_200)
    }

    /// Midnight in the user's time zone starts a new day, and with it the limit.
    @Test func aNewDayStartsAtLocalMidnight() {
        var usage = AssistantUsage()
        let late = date("2026-10-15T23:59:00+02:00")
        usage.record(TokenUsage(input: 10, output: 5), now: late, calendar: calendar)
        #expect(!usage.allows(limit: 1, now: late, calendar: calendar))
        let early = date("2026-10-16T00:01:00+02:00")
        #expect(usage.today(now: early, calendar: calendar).requests == 0)
        #expect(usage.allows(limit: 1, now: early, calendar: calendar))
        usage.record(TokenUsage(input: 1, output: 1), now: early, calendar: calendar)
        #expect(usage.days.map(\.day) == ["2026-10-15", "2026-10-16"])
    }

    @Test func theMonthAddsUpItsDaysOnly() {
        var usage = AssistantUsage()
        usage.record(TokenUsage(input: 100, output: 0), now: date("2026-09-30T12:00:00+02:00"), calendar: calendar)
        usage.record(TokenUsage(input: 10, output: 1), now: date("2026-10-01T12:00:00+02:00"), calendar: calendar)
        usage.record(TokenUsage(input: 20, output: 2), now: date("2026-10-15T12:00:00+02:00"), calendar: calendar)
        let month = usage.thisMonth(now: date("2026-10-15T18:00:00+02:00"), calendar: calendar)
        #expect(month.requests == 2 && month.tokens == 33)
    }

    @Test func keepsTheLastDaysOnly() {
        var usage = AssistantUsage()
        let first = date("2026-01-01T12:00:00+01:00")
        for day in 0..<40 {
            usage.record(TokenUsage(input: 1, output: 0), now: first + Double(day) * 86_400, calendar: calendar)
        }
        #expect(usage.days.count == AssistantUsage.keptDays)
        #expect(usage.days.last?.day == "2026-02-09")
    }

    @Test func noLimitByDefault() {
        var usage = AssistantUsage()
        let noon = date("2026-10-15T12:00:00+02:00")
        for _ in 0..<100 { usage.record(nil, now: noon, calendar: calendar) }
        #expect(usage.allows(limit: 0, now: noon, calendar: calendar))
        #expect(!usage.allows(limit: 100, now: noon, calendar: calendar))
    }

    @Test func survivesASaveAndALoad() throws {
        var usage = AssistantUsage()
        usage.record(TokenUsage(input: 3, output: 4), now: date("2026-10-15T12:00:00+02:00"), calendar: calendar)
        let loaded = try JSONDecoder().decode(AssistantUsage.self, from: JSONEncoder().encode(usage))
        #expect(loaded == usage)
    }

    /// The tokens of every answer within one request add up; outside of a request they're dropped.
    @Test func theMeterAddsUpWhatItsRequestReports() {
        let meter = AssistantMeter()
        AssistantMeter.$current.withValue(meter) {
            AssistantMeter.report(TokenUsage(input: 10, output: 2))
            AssistantMeter.report(nil)
            AssistantMeter.report(TokenUsage(input: 5, output: 1))
        }
        AssistantMeter.report(TokenUsage(input: 99, output: 99))
        #expect(meter.tokens == TokenUsage(input: 15, output: 3))
        #expect(AssistantMeter().tokens == nil, "nothing reported")
    }
}

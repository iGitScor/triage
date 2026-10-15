import Foundation

/// The tokens one assistant answer used, as its provider reported them. Input counts the cached prompt too.
public struct TokenUsage: Codable, Equatable, Sendable {
    public var input: Int
    public var output: Int

    public init(input: Int, output: Int) {
        self.input = input
        self.output = output
    }

    public var total: Int { input + output }
}

/// What the assistant used on this Mac, per day (AI-03): requests and tokens, for the last `keptDays` days with any.
/// Tokens only, no price: Remora doesn't know what each plan charges. Same on Windows (`AssistantUsage`).
public struct AssistantUsage: Codable, Equatable, Sendable {
    public struct Day: Codable, Equatable, Sendable {
        /// "2026-10-15", in the local calendar.
        public var day: String
        public var requests = 0
        public var inputTokens = 0
        public var outputTokens = 0

        public init(day: String, requests: Int = 0, inputTokens: Int = 0, outputTokens: Int = 0) {
            self.day = day
            self.requests = requests
            self.inputTokens = inputTokens
            self.outputTokens = outputTokens
        }

        public var tokens: Int { inputTokens + outputTokens }
    }

    /// Enough for this month's total, and small.
    public static let keptDays = 31

    /// Oldest first.
    public private(set) var days: [Day]

    public init(days: [Day] = []) {
        self.days = days
    }

    /// One request sent, with the tokens its answers reported (nil: the provider said nothing).
    public mutating func record(_ tokens: TokenUsage?, now: Date = .now, calendar: Calendar = .current) {
        let key = Self.key(now, calendar: calendar)
        if days.last?.day != key { days.append(Day(day: key)) }
        days[days.count - 1].requests += 1
        days[days.count - 1].inputTokens += tokens?.input ?? 0
        days[days.count - 1].outputTokens += tokens?.output ?? 0
        days = Array(days.suffix(Self.keptDays))
    }

    public func today(now: Date = .now, calendar: Calendar = .current) -> Day {
        let key = Self.key(now, calendar: calendar)
        return days.last { $0.day == key } ?? Day(day: key)
    }

    /// This calendar month's days added up.
    public func thisMonth(now: Date = .now, calendar: Calendar = .current) -> Day {
        let month = String(Self.key(now, calendar: calendar).prefix(7))
        return days.filter { $0.day.hasPrefix(month) }.reduce(into: Day(day: month)) { total, day in
            total.requests += day.requests
            total.inputTokens += day.inputTokens
            total.outputTokens += day.outputTokens
        }
    }

    /// Whether another request fits today's limit (`limit` requests a day; 0 is no limit).
    public func allows(limit: Int, now: Date = .now, calendar: Calendar = .current) -> Bool {
        limit <= 0 || today(now: now, calendar: calendar).requests < limit
    }

    static func key(_ date: Date, calendar: Calendar) -> String {
        let parts = calendar.dateComponents([.year, .month, .day], from: date)
        return String(format: "%04d-%02d-%02d", parts.year ?? 0, parts.month ?? 0, parts.day ?? 0)
    }
}

/// Adds up the tokens the answers within one request report (a retry, Claude Code's older-version rerun). The
/// coordinator binds one around each request; a plugin reports through `report`, outside of one it's dropped.
public final class AssistantMeter: @unchecked Sendable {
    @TaskLocal public static var current: AssistantMeter?

    private let lock = NSLock()
    private var counted: TokenUsage?

    public init() {}

    /// Nil when no answer said what it used.
    public var tokens: TokenUsage? { lock.withLock { counted } }

    public static func report(_ usage: TokenUsage?) {
        guard let usage, let meter = current else { return }
        meter.lock.withLock {
            meter.counted = TokenUsage(
                input: (meter.counted?.input ?? 0) + usage.input, output: (meter.counted?.output ?? 0) + usage.output)
        }
    }
}

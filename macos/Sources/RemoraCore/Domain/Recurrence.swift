import Foundation

/// How a reminder repeats: on a grid of local days at the first reminder's time of day, until an optional last day.
public struct Recurrence: Hashable, Codable, Sendable {
    public enum Rule: String, Hashable, Codable, Sendable, CaseIterable {
        case daily, weekdays, weekly
    }

    public var rule: Rule
    /// The first occurrence: its time of day, and its weekday for `weekly`, set the grid.
    public var anchor: Date
    /// The last day with an occurrence (inclusive), if any.
    public var until: Date?

    public init(rule: Rule, anchor: Date, until: Date? = nil) {
        self.rule = rule
        self.anchor = anchor
        self.until = until
    }

    /// Whether `until` falls on a day before the first occurrence.
    public func endsBeforeStart(calendar: Calendar = .current) -> Bool {
        guard let until else { return false }
        return calendar.startOfDay(for: until) < calendar.startOfDay(for: anchor)
    }

    /// The first occurrence strictly after `now` (never before `anchor`), or nil once past `until`. Missed
    /// occurrences are skipped.
    public func next(after now: Date, calendar: Calendar = .current) -> Date? {
        let time = calendar.dateComponents([.hour, .minute, .second], from: anchor)
        let weekday = calendar.component(.weekday, from: anchor)
        var day = calendar.startOfDay(for: max(now, anchor))
        // A week always holds the next match, whatever the rule.
        for _ in 0..<8 {
            if let candidate = calendar.date(
                bySettingHour: time.hour ?? 0, minute: time.minute ?? 0, second: time.second ?? 0, of: day),
                candidate > now, candidate >= anchor, matches(day, weekday: weekday, calendar: calendar)
            {
                guard let until else { return candidate }
                let end = calendar.date(byAdding: .day, value: 1, to: calendar.startOfDay(for: until)) ?? until
                return candidate < end ? candidate : nil
            }
            guard let following = calendar.date(byAdding: .day, value: 1, to: day) else { return nil }
            day = following
        }
        return nil
    }

    private func matches(_ day: Date, weekday: Int, calendar: Calendar) -> Bool {
        switch rule {
        case .daily: true
        // Monday to Friday (Sunday is 1, Saturday 7), whatever the locale's weekend.
        case .weekdays: ![1, 7].contains(calendar.component(.weekday, from: day))
        case .weekly: calendar.component(.weekday, from: day) == weekday
        }
    }
}

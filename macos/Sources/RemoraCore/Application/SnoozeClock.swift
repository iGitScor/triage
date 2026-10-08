import Foundation

/// Time picking for snoozes and reminders: a Gestimer-like scrubber plus Google Inbox-like presets.
public struct SnoozeClock: Sendable {
    public struct Preset: Identifiable, Equatable, Sendable {
        public var label: String
        public var symbol: String
        public var date: Date
        public var id: String { label }

        public init(label: String, symbol: String, date: Date) {
            self.label = label
            self.symbol = symbol
            self.date = date
        }
    }

    public static let steps: [TimeInterval] = [
        5, 10, 15, 20, 30, 45,
        60, 90, 120, 180, 240, 360, 480, 720,
        1_440, 2_880, 4_320, 7_200, 10_080,
    ].map { $0 * 60 }

    public var calendar: Calendar

    public init(calendar: Calendar = .current) {
        self.calendar = calendar
    }

    /// Maps a drag position (0…1) to a duration. Small moves give minutes, long ones give days.
    public func duration(at progress: Double) -> TimeInterval {
        let clamped = min(max(progress, 0), 1)
        let index = Int((clamped * Double(Self.steps.count - 1)).rounded())
        return Self.steps[index]
    }

    /// The drag position (0…1) for a duration, interpolated between steps, so the knob can follow a date
    /// picked another way (a preset, a suggestion).
    public func progress(for duration: TimeInterval) -> Double {
        let steps = Self.steps
        guard duration > steps[0] else { return 0 }
        guard duration < steps[steps.count - 1] else { return 1 }
        let upper = steps.firstIndex { $0 >= duration } ?? steps.count - 1
        let lower = upper - 1
        let fraction = (duration - steps[lower]) / (steps[upper] - steps[lower])
        return (Double(lower) + fraction) / Double(steps.count - 1)
    }

    public func presets(now: Date = .now) -> [Preset] {
        var presets: [Preset] = []
        let laterToday = roundedUp(now.addingTimeInterval(3 * 3_600))
        if calendar.isDate(laterToday, inSameDayAs: now) {
            presets.append(Preset(label: L("Later today"), symbol: "clock", date: laterToday))
        }
        if let evening = at(hour: 18, on: now), evening.timeIntervalSince(now) > 3_600 {
            presets.append(Preset(label: L("This evening"), symbol: "moon", date: evening))
        }
        presets.append(Preset(label: L("Tomorrow"), symbol: "sunrise", date: tomorrowMorning(after: now)))
        if let monday = nextWeekday(2, after: now), let morning = at(hour: 9, on: monday) {
            presets.append(Preset(label: L("Next week"), symbol: "calendar", date: morning))
        }
        return presets
    }

    public func tomorrowMorning(after now: Date = .now) -> Date {
        let tomorrow = calendar.date(byAdding: .day, value: 1, to: now) ?? now.addingTimeInterval(86_400)
        return at(hour: 9, on: tomorrow) ?? tomorrow
    }

    public func describe(_ date: Date, now: Date = .now) -> String {
        let seconds = max(0, date.timeIntervalSince(now))
        let relative: String
        switch seconds {
        case ..<3_600: relative = L("in %d min", Int((seconds / 60).rounded()))
        case ..<86_400:
            let hours = Int(seconds / 3_600)
            let minutes = Int(seconds.truncatingRemainder(dividingBy: 3_600) / 60)
            relative = minutes == 0 ? L("in %d h", hours) : L("in %d h %02d", hours, minutes)
        default: relative = L("in %d d", Int((seconds / 86_400).rounded()))
        }
        let style: Date.FormatStyle = calendar.isDate(date, inSameDayAs: now)
            ? .dateTime.hour().minute()
            : .dateTime.weekday(.abbreviated).hour().minute()
        return "\(relative) · \(date.formatted(style))"
    }

    private func at(hour: Int, on day: Date) -> Date? {
        calendar.date(bySettingHour: hour, minute: 0, second: 0, of: day)
    }

    private func nextWeekday(_ weekday: Int, after date: Date) -> Date? {
        calendar.nextDate(after: date, matching: DateComponents(weekday: weekday), matchingPolicy: .nextTime)
    }

    /// Next quarter hour.
    public func roundedUp(_ date: Date) -> Date {
        let quarter: TimeInterval = 15 * 60
        return Date(timeIntervalSinceReferenceDate: (date.timeIntervalSinceReferenceDate / quarter).rounded(.up) * quarter)
    }
}

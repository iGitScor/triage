import Foundation

/// Orders a bundle by importance rather than recency: overdue or due today first, then priority,
/// then your personal habits, then the nearest due date, then the most recent activity.
public struct Prioritizer: Sendable {
    public var now: Date
    public var calendar: Calendar
    /// How likely you are to handle an item quickly (log-odds); breaks ties after explicit priority.
    public var personal: (@Sendable (InboxItem) -> Double)?

    public init(now: Date = .now, calendar: Calendar = .current, personal: (@Sendable (InboxItem) -> Double)? = nil) {
        self.now = now
        self.calendar = calendar
        self.personal = personal
    }

    public func sorted(_ items: [InboxItem]) -> [InboxItem] {
        items.sorted { a, b in
            let pressingA = isPressing(a), pressingB = isPressing(b)
            if pressingA != pressingB { return pressingA }
            let priorityA = a.priority ?? .normal, priorityB = b.priority ?? .normal
            if priorityA != priorityB { return priorityA > priorityB }
            if let personal {
                let scoreA = personal(a), scoreB = personal(b)
                if abs(scoreA - scoreB) > 0.5 { return scoreA > scoreB }
            }
            switch (a.due, b.due) {
            case let (dueA?, dueB?) where dueA != dueB: return dueA < dueB
            case (.some, nil): return true
            case (nil, .some): return false
            default: return a.date > b.date
            }
        }
    }

    /// Overdue or due today.
    public func isPressing(_ item: InboxItem) -> Bool {
        guard let due = item.due else { return false }
        return due < calendar.startOfDay(for: now) || calendar.isDate(due, inSameDayAs: now)
    }
}

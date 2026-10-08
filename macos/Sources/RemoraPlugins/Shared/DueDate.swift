import Foundation
import RemoraCore

enum DueDate {
    /// Overdue and due-today badges notify once; later dates are a quiet chip.
    static func badge(for due: Date, now: Date = .now, calendar: Calendar = .current) -> Badge {
        if due < calendar.startOfDay(for: now) {
            return Badge(id: "overdue", label: L("Overdue"), symbol: "exclamationmark.circle", tone: .negative,
                         notify: .init(title: L("Task overdue")))
        }
        if calendar.isDate(due, inSameDayAs: now) {
            return Badge(id: "due.today", label: L("Due today"), symbol: "calendar", tone: .warning,
                         notify: .init(title: L("Due today")))
        }
        return Badge(id: "due", label: due.formatted(.dateTime.day().month(.abbreviated)), symbol: "calendar", tone: .neutral)
    }

    /// "2026-10-09" or a full ISO-8601 timestamp.
    static func parse(_ raw: String?) -> Date? {
        guard let raw, !raw.isEmpty else { return nil }
        return Date(iso8601: raw) ?? (try? Date(raw, strategy: .iso8601.year().month().day()))
    }
}

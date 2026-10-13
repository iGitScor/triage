import Foundation

/// Notifications grouped at set times (IDEA-26): what arrives and what changes waits for the next time the user
/// chose, then comes as it is, or as one notification when there is more. Reminders and snoozes coming back keep
/// their own time. Same rule on Windows (`batch.rs`).
public enum NotificationBatch {
    /// The times offered at first: mid-morning, after lunch, before leaving.
    public static let defaultTimes = "10:00, 14:00, 17:00"

    /// "10:00, 14:00 17:00" → minutes since midnight, in order and once each. Anything that isn't a time is left out.
    public static func times(_ text: String) -> [Int] {
        let parts = text.split { ",; ".contains($0) }
        let minutes = parts.compactMap { part -> Int? in
            let pieces = part.split(separator: ":")
            guard pieces.count == 2, let hour = Int(pieces[0]), let minute = Int(pieces[1]),
                (0..<24).contains(hour), (0..<60).contains(minute), pieces[1].count == 2
            else { return nil }
            return hour * 60 + minute
        }
        return Array(Set(minutes)).sorted()
    }

    /// Whether one of `times` (on this Mac's clock) came after `since` and by `now`. After two days away, one has.
    public static func isDue(_ times: [Int], since: Date, now: Date, calendar: Calendar = .current) -> Bool {
        guard !times.isEmpty, now > since else { return false }
        if now.timeIntervalSince(since) > 2 * 86_400 { return true }
        var day = calendar.startOfDay(for: since)
        while day <= now {
            for minutes in times {
                guard
                    let at = calendar.date(
                        bySettingHour: minutes / 60, minute: minutes % 60, second: 0, of: day)
                else { continue }
                if since < at, at <= now { return true }
            }
            guard let next = calendar.date(byAdding: .day, value: 1, to: day) else { break }
            day = next
        }
        return false
    }

    /// Whether a notice waits for the next set time: what arrived or changed, not a reminder.
    public static func waits(_ notice: Notice) -> Bool { notice.kind != .reminder }

    /// What waited, as notifications: up to three as they are, more as one that says how many and opens Remora.
    public static func grouped(_ waiting: [Notice], limit: Int = 3) -> [Notice] {
        guard waiting.count > limit else { return waiting }
        return [
            Notice(
                kind: .arrival,
                itemID: "batch" + ChangeDetector.whileClosedSuffix,
                title: L("%d new in Remora", waiting.count),
                subtitle: "",
                body: L("Open Remora to see them."),
                url: nil
            )
        ]
    }
}

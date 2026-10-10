import Foundation

/// When to ask a tool again after it failed. One failure changes nothing: the next refresh tries again.
/// Then less and less often (every 2, then 4 refreshes…, at most every 30 minutes), so a broken server or token
/// isn't hammered. A rate limit is waited out until the server's reset time, even by a manual refresh.
public struct Backoff: Equatable, Sendable {
    public private(set) var failures = 0
    public private(set) var notBefore: Date?
    public private(set) var rateLimitedUntil: Date?

    public static let longestWait: TimeInterval = 30 * 60

    public init() {}

    public mutating func failed(at now: Date, interval: TimeInterval, rateLimitedUntil resume: Date? = nil) {
        failures += 1
        let wait = min(interval * pow(2, Double(failures - 1)), max(interval, Self.longestWait))
        // A little early: the next scheduled refresh comes back about one interval later, not to the second.
        notBefore = now.addingTimeInterval(wait - min(10, interval / 10))
        rateLimitedUntil = resume.map { max($0, now) }
        if let resume, resume > (notBefore ?? now) { notBefore = resume }
    }

    public mutating func succeeded() {
        self = Backoff()
    }

    /// Whether this account sits this refresh out. A manual refresh skips the slow-down, never a rate limit.
    public func waits(at now: Date, manual: Bool) -> Bool {
        if let rateLimitedUntil, rateLimitedUntil > now { return true }
        if manual { return false }
        return (notBefore ?? .distantPast) > now
    }
}

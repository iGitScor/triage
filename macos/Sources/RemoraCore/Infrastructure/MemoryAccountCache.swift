import Foundation

/// `AccountCache` in memory: nothing outlives the app, and the app drops it with the account.
public actor MemoryAccountCache: AccountCache {
    private var entries: [String: (value: any Sendable, at: Date)] = [:]
    private let now: @Sendable () -> Date

    public init(now: @escaping @Sendable () -> Date = { .now }) {
        self.now = now
    }

    public func value<T: Sendable>(
        _ key: String, lifetime: TimeInterval, load: @Sendable () async throws -> T
    ) async throws -> T {
        if let entry = entries[key], let value = entry.value as? T, now().timeIntervalSince(entry.at) < lifetime {
            return value
        }
        let value = try await load()
        entries[key] = (value, now())
        return value
    }
}

import Foundation

/// What a plugin may keep between refreshes of one account, such as who the token belongs to: asking for it every
/// few minutes costs a request and counts against the tool's rate limit. In memory only, and dropped by the app when
/// the account is reconnected, changed, disconnected or erased.
public protocol AccountCache: Sendable {
    /// The value kept under `key` if younger than `lifetime`, else what `load` returns, kept for next time. A failed
    /// `load` keeps nothing.
    func value<T: Sendable>(
        _ key: String, lifetime: TimeInterval, load: @Sendable () async throws -> T
    ) async throws -> T
}

public enum CacheLifetime {
    /// Who the token belongs to changes rarely; an hour keeps a renamed user from going unnoticed for long.
    public static let identity: TimeInterval = 3600
}

/// Keeps nothing: every call loads. For plugins built outside a refresh (tests, the connection check).
public struct NoAccountCache: AccountCache {
    public init() {}

    public func value<T: Sendable>(
        _ key: String, lifetime: TimeInterval, load: @Sendable () async throws -> T
    ) async throws -> T {
        try await load()
    }
}

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

    /// What was kept under `key` with `keep`, however old: for a value that knows when it expires (an access token).
    func kept<T: Sendable>(_ key: String) async -> T?

    func keep<T: Sendable>(_ key: String, _ value: T) async

    /// A secret the plugin renewed during a fetch (an OAuth refresh token the server rotated), for the app to save in
    /// the account's Keychain entry. Only keys in `renewedSecrets`.
    func renew(_ key: String, _ value: String) async

    /// What was renewed since the last call, for the app to save.
    func takeRenewed() async -> [String: String]
}

/// The secrets a plugin renews on its own, left out of the session's fingerprint so a renewal doesn't start the
/// account over: a new sign-in changes `signIn` instead. Same list on Windows (`RENEWED_SECRETS`).
public let renewedSecrets: Set<String> = ["refreshToken"]

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

    public func kept<T: Sendable>(_ key: String) async -> T? { nil }
    public func keep<T: Sendable>(_ key: String, _ value: T) async {}
    public func renew(_ key: String, _ value: String) async {}
    public func takeRenewed() async -> [String: String] { [:] }
}

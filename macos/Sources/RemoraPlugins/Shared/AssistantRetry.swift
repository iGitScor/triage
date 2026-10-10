import Foundation
import RemoraCore

/// Retries for the assistants that call an API (Claude, OpenAI-compatible). Same rule on Windows (`assistant_retry`).
enum AssistantRetry {
    /// Retries an overloaded or rate-limited answer up to twice (AI-04): 429, 408, 409 and 5xx (529 is "overloaded").
    /// A rate limit waits as long as `retry-after` says, up to a minute; beyond that, or without it, 2 s then 4 s.
    /// Nil: give up and show the error.
    static func delay(after error: Error, attempt: Int, now: Date = .now) -> TimeInterval? {
        guard attempt < 2 else { return nil }
        let backoff = TimeInterval(2 << attempt)
        switch error as? HTTPError {
        case .rateLimited(let until?):
            let wait = until.timeIntervalSince(now)
            return wait > 60 ? nil : max(wait, 0)
        case .rateLimited(nil): return backoff
        case .status(let code) where [408, 409].contains(code) || code >= 500: return backoff
        default: return nil
        }
    }

    /// Sends `request` until it succeeds, or fails in a way a retry can't fix. A timeout says `tooSlow`.
    static func decode<T: Decodable>(
        _ type: T.Type, from request: URLRequest, http: HTTPClient, tooSlow: String,
        sleep: @Sendable (TimeInterval) async throws -> Void
    ) async throws -> T {
        var attempt = 0
        while true {
            do {
                return try await http.decode(T.self, from: request, using: JSONDecoder())
            } catch let error as URLError where error.code == .timedOut {
                throw HTTPError.api(tooSlow)
            } catch {
                guard let delay = delay(after: error, attempt: attempt) else { throw error }
                attempt += 1
                try await sleep(delay)
            }
        }
    }
}

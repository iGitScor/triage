import Foundation
import Testing

@testable import RemoraCore

/// A failing or rate-limited tool is asked less often, and never before its reset time.
struct BackoffTests {
    let now = Date(timeIntervalSince1970: 1_800_000_000)
    let interval: TimeInterval = 300

    @Test func oneFailureWaitsOnlyForTheNextRefresh() {
        var backoff = Backoff()
        backoff.failed(at: now, interval: interval)
        #expect(!backoff.waits(at: now.addingTimeInterval(interval), manual: false))
    }

    @Test func repeatedFailuresWaitLongerUpToHalfAnHour() {
        var backoff = Backoff()
        for _ in 0..<2 { backoff.failed(at: now, interval: interval) }
        #expect(backoff.waits(at: now.addingTimeInterval(interval), manual: false), "skips one refresh")
        #expect(!backoff.waits(at: now.addingTimeInterval(2 * interval), manual: false))
        for _ in 0..<10 { backoff.failed(at: now, interval: interval) }
        #expect(
            !backoff.waits(at: now.addingTimeInterval(Backoff.longestWait), manual: false),
            "never longer than 30 minutes")
        #expect(!backoff.waits(at: now, manual: true), "a manual refresh doesn't wait for a slow-down")
    }

    @Test func aRateLimitIsWaitedOutEvenByAManualRefresh() {
        var backoff = Backoff()
        let reset = now.addingTimeInterval(20 * 60)
        backoff.failed(at: now, interval: interval, rateLimitedUntil: reset)
        #expect(backoff.waits(at: now.addingTimeInterval(interval), manual: true))
        #expect(!backoff.waits(at: reset.addingTimeInterval(1), manual: false))
        backoff.succeeded()
        #expect(backoff == Backoff())
    }

    @Test func rateLimitsAreRecognisedWithTheirResetTime() async throws {
        // GitHub's primary limit: a 403 with nothing left, and the reset in seconds since 1970.
        #expect(
            await failure(403, ["X-RateLimit-Remaining": "0", "X-RateLimit-Reset": "1800000600"]) as? HTTPError
                == .rateLimited(Date(timeIntervalSince1970: 1_800_000_600)))
        // GitLab's RateLimit-Reset, and a plain 429.
        #expect(
            await failure(429, ["RateLimit-Reset": "1800000900"]) as? HTTPError
                == .rateLimited(Date(timeIntervalSince1970: 1_800_000_900)))
        #expect(await failure(429, [:]) as? HTTPError == .rateLimited(nil))
        // GitHub's secondary limit: a 403 with requests left, and when to come back.
        if case .rateLimited(let until?) = await failure(403, ["X-RateLimit-Remaining": "4000", "Retry-After": "60"])
            as? HTTPError
        {
            #expect(abs(until.timeIntervalSinceNow - 60) < 5)
        } else {
            Issue.record("GitHub's secondary limit read as a refused token")
        }
        // A 403 with requests left is still a refused token.
        #expect(await failure(403, ["X-RateLimit-Remaining": "12"]) as? HTTPError == .unauthorized)
        // Retry-After in seconds.
        if case .rateLimited(let until?) = await failure(429, ["Retry-After": "120"]) as? HTTPError {
            #expect(abs(until.timeIntervalSinceNow - 120) < 5)
        } else {
            Issue.record("Retry-After was ignored")
        }
    }
}

/// An HTTP answer with a status and headers, and the error `decode` makes of it.
private struct Answer: HTTPClient {
    let status: Int
    let headers: [String: String]
    func send(_ request: URLRequest) async throws -> (Data, HTTPURLResponse) {
        (Data(), HTTPURLResponse(url: request.url!, statusCode: status, httpVersion: nil, headerFields: headers)!)
    }
}

private func failure(_ status: Int, _ headers: [String: String]) async -> Error? {
    let request = URLRequest.get(URL(string: "https://api.github.com/user")!)
    do {
        _ = try await Answer(status: status, headers: headers).decode([String: String].self, from: request)
        return nil
    } catch { return error }
}

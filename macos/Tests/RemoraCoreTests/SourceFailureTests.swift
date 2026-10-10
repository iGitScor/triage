import Foundation
import Testing

@testable import RemoraCore

/// Offline, a rejected token and a rate limit don't look the same.
struct SourceFailureTests {
    @Test func failuresAreSortedByWhatFixesThem() {
        #expect(FailureKind(URLError(.notConnectedToInternet)) == .offline)
        #expect(FailureKind(URLError(.cannotFindHost)) == .unreachable)
        #expect(FailureKind(URLError(.timedOut)) == .unreachable)
        #expect(FailureKind(HTTPError.unauthorized) == .auth)
        #expect(FailureKind(HTTPError.rateLimited(nil)) == .rateLimited)
        #expect(FailureKind(HTTPError.status(500)) == .other)
    }

    @Test func networkFailuresSayWhatTheyMean() {
        #expect(SourceFailure(URLError(.notConnectedToInternet)).message.hasPrefix("Offline"))
        #expect(SourceFailure(HTTPError.unauthorized).message == HTTPError.unauthorized.localizedDescription)
    }

    @Test func theFooterSaysTheMostUsefulThing() {
        let (a, b, c) = (UUID(), UUID(), UUID())
        let offline = SourceFailure(kind: .offline, message: "")
        let auth = SourceFailure(kind: .auth, message: "")
        let limited = SourceFailure(kind: .rateLimited, message: "")
        let broken = SourceFailure(kind: .other, message: "")
        #expect(SourcesHealth(failures: [:], offline: false) == .fine)
        #expect(SourcesHealth(failures: [:], offline: true) == .offline)
        #expect(
            SourcesHealth(failures: [a: offline, b: offline], offline: false) == .offline,
            "every source failed for lack of a network")
        #expect(SourcesHealth(failures: [a: auth, b: broken], offline: false) == .reconnect([a]))
        #expect(SourcesHealth(failures: [a: limited], offline: false) == .fine, "a rate limit is only waited out")
        #expect(SourcesHealth(failures: [a: limited, b: broken, c: broken], offline: false) == .failing(2))
    }
}

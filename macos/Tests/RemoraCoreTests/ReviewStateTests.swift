import Foundation
import Testing

@testable import RemoraCore

/// A reviewer's stand is a state, not a colour. Same cases as Windows.
struct ReviewStateTests {
    func read(_ json: String) throws -> ReviewState? {
        try JSONDecoder().decode(Person.self, from: Data(json.utf8)).review
    }

    /// An inbox saved before 0.10 gave a reviewer's stand only as the colour of their avatar.
    @Test func aSavedReviewerColourBecomesTheirStand() throws {
        #expect(try read(#"{"name":"a","tone":"accent"}"#) == .approved)
        #expect(try read(#"{"name":"a","tone":"positive"}"#) == .approved)
        #expect(try read(#"{"name":"a","tone":"negative"}"#) == .changesRequested)
        #expect(try read(#"{"name":"a"}"#) == nil)
        #expect(try read(#"{"name":"a","review":"changesRequested"}"#) == .changesRequested)
        let encoder = JSONEncoder()
        encoder.outputFormatting = .sortedKeys  // key order isn't guaranteed otherwise: the test was flaky
        let saved = try encoder.encode(Person(name: "a", review: .approved))
        #expect(String(decoding: saved, as: UTF8.self) == #"{"name":"a","review":"approved"}"#)
    }

    /// A badge id this version doesn't know still loads, and ids are saved as plain strings.
    @Test func badgeIDsAreOpenAndSavedAsStrings() throws {
        let badge = try JSONDecoder().decode(
            Badge.self, from: Data(#"{"id":"future.thing","label":"x","tone":"neutral"}"#.utf8))
        #expect(badge.id.rawValue == "future.thing")
        let saved = try JSONEncoder().encode(Badge(id: .checksFailing, label: "x", tone: .neutral))
        #expect(String(decoding: saved, as: UTF8.self).contains(#""id":"checks.failing""#))
    }
}

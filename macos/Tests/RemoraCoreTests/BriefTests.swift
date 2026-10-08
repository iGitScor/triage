import Foundation
import Testing
@testable import RemoraCore

struct BriefTests {
    let brief = Brief(summary: "s", focus: [], createdAt: now)

    @Test func staysFreshForTheCacheDuration() {
        #expect(brief.isFresh(cacheMinutes: 30, now: now + 29 * 60))
        #expect(!brief.isFresh(cacheMinutes: 30, now: now + 30 * 60))
    }

    @Test func zeroMinutesMeansNoCache() {
        #expect(!brief.isFresh(cacheMinutes: 0, now: now))
    }

    @Test func bundleSummaryNeedsTheSameItems() {
        let items = [makeItem("1"), makeItem("2")]
        let summary = BundleSummary(text: "t", items: items, createdAt: now)
        #expect(summary.isFresh(for: items.reversed(), cacheMinutes: 30, now: now))
        #expect(!summary.isFresh(for: items + [makeItem("3")], cacheMinutes: 30, now: now))
        #expect(!summary.isFresh(for: items, cacheMinutes: 30, now: now + 31 * 60))
    }
}

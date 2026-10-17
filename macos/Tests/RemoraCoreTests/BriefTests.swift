import Foundation
import Testing

@testable import RemoraCore

struct BriefTests {
    let brief = Brief(summary: "s", focus: [], createdAt: now)

    @Test func staysFreshForTheCacheDuration() {
        #expect(brief.isFresh(for: [], cacheMinutes: 30, now: now + 29 * 60))
        #expect(!brief.isFresh(for: [], cacheMinutes: 30, now: now + 30 * 60))
    }

    @Test func zeroMinutesMeansNoCache() {
        #expect(!brief.isFresh(for: [], cacheMinutes: 0, now: now))
    }

    @Test func briefNeedsTheSameItems() {
        let items = [makeItem("1"), makeItem("2")]
        let written = brief.writtenFrom(items)
        #expect(written.isFresh(for: items.reversed(), cacheMinutes: 30, now: now))
        #expect(!written.isFresh(for: [items[0]], cacheMinutes: 30, now: now))
        #expect(!written.isFresh(for: items + [makeItem("3")], cacheMinutes: 30, now: now))
        // Saved before the items were kept: time alone.
        #expect(brief.isFresh(for: items, cacheMinutes: 30, now: now))
    }

    /// An item that changed (approved, a failing check, a new reply) makes a brief or summary stale too.
    @Test func aChangedItemMakesThemStale() {
        let items = [makeItem("1"), makeItem("2")]
        var changed = items
        changed[0].badges.append(Badge(id: .approved, label: "Approved", tone: .positive))
        #expect(!brief.writtenFrom(items).isFresh(for: changed, cacheMinutes: 30, now: now))
        #expect(
            !BundleSummary(text: "t", items: items, createdAt: now).isFresh(for: changed, cacheMinutes: 30, now: now))
        var replied = items
        replied[1].date = replied[1].date.addingTimeInterval(60)
        #expect(!brief.writtenFrom(items).isFresh(for: replied, cacheMinutes: 30, now: now))
    }

    @Test func bundleSummaryNeedsTheSameItems() {
        let items = [makeItem("1"), makeItem("2")]
        let summary = BundleSummary(text: "t", items: items, createdAt: now)
        #expect(summary.isFresh(for: items.reversed(), cacheMinutes: 30, now: now))
        #expect(!summary.isFresh(for: items + [makeItem("3")], cacheMinutes: 30, now: now))
        #expect(!summary.isFresh(for: items, cacheMinutes: 30, now: now + 31 * 60))
    }
}

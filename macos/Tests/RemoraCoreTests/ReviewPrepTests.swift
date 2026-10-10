import Foundation
import Testing
@testable import RemoraCore

struct ReviewPrepTests {
    func review(_ id: String, _ files: [(String, Int)], hoursAgo: Double = 1) -> InboxItem {
        var item = makeItem(id, bundle: .reviews, date: now.addingTimeInterval(-hoursAgo * 3_600))
        item.changes = ChangeSet(files: files.map { ChangedFile(path: $0.0, additions: $0.1, deletions: 0) })
        return item
    }

    @Test func sizesEstimatesAndTests() {
        let prep = ReviewPrep(review("1", [("src/api/retry.ts", 40), ("src/api/retry.test.ts", 30)]))!
        #expect(prep.size == .small && prep.lines == 70 && prep.fileCount == 2)
        #expect(prep.estimatedMinutes == 3)
        #expect(prep.testsTouched)
        #expect(prep.summary == "~3 min · 2 files")
        #expect(ReviewPrep(review("2", [("a.ts", 900)]))!.size == .large)
        #expect(ReviewPrep(makeItem("3")) == nil, "no files, no prep")
    }

    @Test func flagsComeFromPaths() {
        let prep = ReviewPrep(review("1", [
            ("db/migrations/2026_add_avatars.sql", 20), ("src/privacy/consent.ts", 50),
            ("src/auth/session.ts", 5), (".github/workflows/ci.yml", 3), ("package.json", 1),
        ]))!
        #expect(prep.flags == [.migrations, .auth, .personalData, .infra, .dependencies])
        #expect(!prep.testsTouched)
        #expect(prep.topFiles.first?.path == "src/privacy/consent.ts")
    }

    @Test func lockfileOnlyIsItsOwnFlag() {
        let prep = ReviewPrep(review("1", [("package-lock.json", 800), ("yarn.lock", 40)]))!
        #expect(prep.flags == [.lockfileOnly])
    }

    @Test func sessionDoesQuickWinsFirstThenOldest() {
        let big = review("big", [("a.ts", 600)], hoursAgo: 48)
        let quickOld = review("quickOld", [("b.ts", 10)], hoursAgo: 30)
        let quickNew = review("quickNew", [("c.ts", 10)], hoursAgo: 2)
        var overdue = review("overdue", [("d.ts", 400)])
        overdue.due = now - 86_400
        #expect(ReviewQueue.order([big, quickNew, overdue, quickOld], now: now).map(\.id) == ["overdue", "quickOld", "quickNew", "big"])
    }

    @Test func pathsOnlyEstimateFromFileCount() {
        var item = makeItem("1", bundle: .reviews)
        item.changes = ChangeSet(files: (1...5).map { ChangedFile(path: "src/f\($0).ts") })
        let prep = ReviewPrep(item)!
        #expect(prep.lines == nil && prep.size == .medium && prep.estimatedMinutes == 12)
    }

    /// Lockfiles and generated files don't make a review longer.
    @Test func generatedFilesDontCount() {
        let lockfileOnly = ReviewPrep(review("1", [("package-lock.json", 2_400)]))!
        #expect(lockfileOnly.estimatedMinutes == 2 && lockfileOnly.size == .tiny, "not ~60 min")
        let mixed = ReviewPrep(review("2", [("src/api.ts", 40), ("yarn.lock", 900), ("dist/app.min.js", 3_000),
                                            ("src/__snapshots__/api.test.ts.snap", 200)]))!
        #expect(mixed.lines == 40 && mixed.fileCount == 4, "every file listed, only the reviewable ones counted")
        #expect(mixed.estimatedMinutes == 3)
        #expect(mixed.topFiles.map(\.path) == ["src/api.ts"])
    }

    /// Flags and tests match words of the path, not fragments.
    @Test func flagsMatchWordsNotFragments() {
        let lookalikes = ReviewPrep(review("1", [("src/author/latest.ts", 10), ("src/personality.ts", 10), ("docs/environment.md", 5)]))!
        #expect(lookalikes.flags.isEmpty && !lookalikes.testsTouched)
        let real = ReviewPrep(review("2", [("Sources/OAuthClient.swift", 10), ("Tests/RetryTests.swift", 10), (".env.example", 1),
                                           ("Package.swift", 1)]))!
        #expect(real.flags == [.auth, .infra, .dependencies])
        #expect(real.testsTouched)
    }

    /// One default for a review without a file list.
    @Test func unknownReviewsShareOneDefault() {
        let unknown = makeItem("u", bundle: .reviews)
        #expect(ReviewQueue.remainingMinutes([unknown]) == ReviewPrep.unknownMinutes)
        let quick = review("q", [("a.ts", 10)])
        #expect(ReviewQueue.order([unknown, quick], now: now).map(\.id) == ["q", "u"], "ordered with the same default")
    }

    /// From five timed reviews on, estimates follow your pace.
    @Test func estimatesLearnYourPace() {
        #expect(ReviewPace.factor(Array(repeating: ReviewTiming(estimated: 10, actual: 20), count: 4)) == 1, "not before five")
        let slower = Array(repeating: ReviewTiming(estimated: 10, actual: 20), count: 5)
        #expect(ReviewPace.factor(slower) == 2)
        let forgotten = slower + Array(repeating: ReviewTiming(estimated: 10, actual: 600), count: 10)
        #expect(ReviewPace.factor(forgotten) == 2, "a timer left running for hours says nothing")
        #expect(ReviewPace.factor(Array(repeating: ReviewTiming(estimated: 10, actual: 100), count: 5)) == 3, "at most three times")
        let prep = ReviewPrep(review("1", [("a.ts", 400)]), pace: 2)!
        #expect(prep.estimatedMinutes == 24)
    }

}

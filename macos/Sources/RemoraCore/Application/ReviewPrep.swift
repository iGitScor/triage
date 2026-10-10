import Foundation

/// What a review will take, computed on this Mac from file paths and line counts (never code).
public struct ReviewPrep: Equatable, Sendable {
    public enum Size: String, Sendable { case tiny, small, medium, large }

    /// Areas that deserve extra attention, read from the paths.
    public enum Flag: String, CaseIterable, Sendable {
        case migrations, auth, personalData, infra, dependencies, lockfileOnly

        public var title: String {
            switch self {
            case .migrations: L("Migrations")
            case .auth: L("Auth")
            case .personalData: L("Personal data")
            case .infra: L("Infra/CI")
            case .dependencies: L("Dependencies")
            case .lockfileOnly: L("Lockfile only")
            }
        }

        public var symbol: String {
            switch self {
            case .migrations: "cylinder.split.1x2"
            case .auth: "key"
            case .personalData: "person.text.rectangle"
            case .infra: "server.rack"
            case .dependencies: "shippingbox"
            case .lockfileOnly: "lock.doc"
            }
        }
    }

    /// Nil when the source only gives file paths (GitLab): the estimate then uses the file count.
    public var lines: Int?
    public var fileCount: Int
    public var size: Size
    public var estimatedMinutes: Int
    public var testsTouched: Bool
    public var flags: [Flag]
    public var topFiles: [ChangedFile]

    /// An estimate for a review Remora knows nothing about (no file list).
    public static let unknownMinutes = 10

    /// Words in a path that flag an area: whole words of the path, so `auth` is in `src/auth/` and
    /// `OAuthClient`, not in "author", and `test` is in `RetryTests.swift`, not in "latest".
    static let words: [Flag: Set<String>] = [
        .migrations: ["migration", "migrations", "migrate", "db", "schema"],
        .auth: ["auth", "oauth", "authn", "authz", "authentication", "authorization", "login", "logout", "session",
                "sessions", "permission", "permissions", "rbac", "sso", "saml", "jwt"],
        .personalData: ["privacy", "gdpr", "consent", "pii", "personal"],
        .infra: ["docker", "dockerfile", "terraform", "helm", "k8s", "kubernetes", "workflows", "env"],
    ]
    /// Manifests, by file name.
    static let manifests: Set<String> = ["package.json", "go.mod", "gemfile", "requirements.txt", "podfile", "cargo.toml",
                                         "pyproject.toml", "package.swift", "build.gradle", "pom.xml"]
    static let testWords: Set<String> = ["test", "tests", "spec", "specs", "testing"]
    static let lockfiles = ["package-lock.json", "yarn.lock", "pnpm-lock.yaml", "go.sum", "gemfile.lock", "podfile.lock",
                            "cargo.lock", "poetry.lock", "package.resolved", "composer.lock", "flake.lock"]
    /// Folders and suffixes of files nobody reviews line by line: they don't count in the estimate.
    static let generatedFolders: Set<String> = ["dist", "build", "vendor", "node_modules", "pods", "__generated__", "generated", "__snapshots__"]
    static let generatedSuffixes = [".min.js", ".min.css", ".map", ".snap", ".pb.go", ".g.dart", ".generated.ts", ".pbxproj", "_pb2.py"]

    /// A lockfile or a generated file: shown in the count, left out of the estimate.
    static func isGenerated(_ path: String) -> Bool {
        let path = path.lowercased()
        let segments = path.split(separator: "/").map(String.init)
        return lockfiles.contains { path.hasSuffix($0) }
            || generatedSuffixes.contains { path.hasSuffix($0) }
            || segments.dropLast().contains { generatedFolders.contains($0) }
    }

    /// The words of a path: split at anything but letters and digits, and at camelCase humps.
    static func words(of path: String) -> Set<String> {
        let spaced = path.replacingOccurrences(of: "([a-z0-9])([A-Z])", with: "$1 $2", options: .regularExpression)
        return Set(spaced.lowercased().split { !$0.isLetter && !$0.isNumber }.map(String.init))
    }

    /// `pace`: how your reviews compare with the estimate, learned from timed ones (`ReviewPace`); 1 until then.
    public init?(_ item: InboxItem, pace: Double = 1) {
        guard let changes = item.changes, !changes.files.isEmpty else { return nil }
        let paths = changes.files.map { $0.path.lowercased() }
        // Lockfiles and generated files are listed but not reviewed: they don't make a review longer.
        let reviewable = changes.files.filter { !Self.isGenerated($0.path) }
        let generated = changes.files.count - reviewable.count
        let counted = changes.files.contains { $0.additions != nil || $0.deletions != nil }
        // The total from the tool, when there are no per-file counts, can't leave generated lines out.
        lines = counted ? reviewable.reduce(0) { $0 + $1.lines } : item.diffSize
        fileCount = changes.fileCount
        let reviewableCount = max(0, changes.fileCount - generated)
        let estimate: Int
        if reviewableCount == 0 {
            size = .tiny
            estimate = 2
        } else if let lines {
            size = switch lines {
            case ..<20: .tiny
            case ..<150: .small
            case ..<500: .medium
            default: .large
            }
            estimate = min(60, max(2 + lines / 40, Int((Double(reviewableCount) / 3).rounded(.up))))
        } else {
            size = switch reviewableCount {
            case ...2: .small
            case ...8: .medium
            default: .large
            }
            estimate = min(60, 2 + reviewableCount * 2)
        }
        estimatedMinutes = max(1, Int((Double(estimate) * pace).rounded()))
        testsTouched = changes.files.contains { !Self.words(of: $0.path).isDisjoint(with: Self.testWords) }

        let onlyLockfiles = paths.allSatisfy { path in Self.lockfiles.contains { path.hasSuffix($0) } }
        if onlyLockfiles {
            flags = [.lockfileOnly]
        } else {
            let pathWords = changes.files.map { Self.words(of: $0.path) }
            let names = paths.map { $0.split(separator: "/").last.map(String.init) ?? $0 }
            flags = Flag.allCases.filter { flag in
                switch flag {
                case .dependencies: names.contains { Self.manifests.contains($0) }
                case .lockfileOnly: false
                default: Self.words[flag].map { words in pathWords.contains { !$0.isDisjoint(with: words) } } ?? false
                }
            }
        }
        topFiles = Array(reviewable.sorted { $0.lines > $1.lines }.prefix(3))
    }

    /// "~6 min · 4 files", the rest is shown as chips.
    public var summary: String {
        L("~%d min · %@", estimatedMinutes, L("%d file", plural: "%d files", fileCount))
    }
}

/// The order of a review session: pressing first, then quick wins, then whoever waited longest.
public enum ReviewQueue {
    public static func order(_ items: [InboxItem], now: Date = .now) -> [InboxItem] {
        let prioritizer = Prioritizer(now: now)
        return items.sorted { a, b in
            let pressingA = prioritizer.isPressing(a), pressingB = prioritizer.isPressing(b)
            if pressingA != pressingB { return pressingA }
            let minutesA = minutes(a), minutesB = minutes(b)
            if minutesA != minutesB { return minutesA < minutesB }
            return a.date < b.date
        }
    }

    public static func remainingMinutes(_ items: [InboxItem], pace: Double = 1) -> Int {
        items.reduce(0) { $0 + minutes($1, pace: pace) }
    }

    /// One default for a review without a file list, wherever it is counted.
    static func minutes(_ item: InboxItem, pace: Double = 1) -> Int {
        ReviewPrep(item, pace: pace)?.estimatedMinutes ?? max(1, Int((Double(ReviewPrep.unknownMinutes) * pace).rounded()))
    }
}

/// How long a review you timed really took, against its estimate. Kept on this Mac only.
public struct ReviewTiming: Codable, Equatable, Sendable {
    public var estimated: Int
    public var actual: Int

    public init(estimated: Int, actual: Int) {
        self.estimated = estimated
        self.actual = actual
    }
}

/// Your pace from timed reviews: the median of actual over estimated, from 5 of them, between half and three times.
/// A review left running for hours, or done in under a minute, says nothing and is left out.
public enum ReviewPace {
    public static let minimumTimings = 5

    public static func factor(_ timings: [ReviewTiming]) -> Double {
        let ratios = timings.suffix(20)
            .filter { $0.estimated > 0 && (1...240).contains($0.actual) }
            .map { Double($0.actual) / Double($0.estimated) }
            .sorted()
        guard ratios.count >= minimumTimings else { return 1 }
        return min(3, max(0.5, ratios[ratios.count / 2]))
    }
}

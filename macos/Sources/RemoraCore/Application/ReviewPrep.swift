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

    static let patterns: [Flag: [String]] = [
        .migrations: ["migrat", "/db/", "schema"],
        .auth: ["auth", "login", "session", "permission", "rbac"],
        .personalData: ["privacy", "gdpr", "consent", "pii", "personal"],
        .infra: [".github/workflows", "docker", "terraform", "helm", "k8s", ".env"],
        .dependencies: ["package.json", "go.mod", "gemfile", "requirements", "podfile", "cargo.toml", "pyproject.toml"],
    ]
    static let lockfiles = ["package-lock.json", "yarn.lock", "pnpm-lock.yaml", "go.sum", "gemfile.lock", "podfile.lock", "cargo.lock", "poetry.lock"]

    public init?(_ item: InboxItem) {
        guard let changes = item.changes, !changes.files.isEmpty else { return nil }
        let paths = changes.files.map { $0.path.lowercased() }
        let counted = changes.files.contains { $0.additions != nil || $0.deletions != nil }
        lines = counted || item.diffSize != nil ? max(changes.files.reduce(0) { $0 + $1.lines }, item.diffSize ?? 0) : nil
        fileCount = changes.fileCount
        if let lines {
            size = switch lines {
            case ..<20: .tiny
            case ..<150: .small
            case ..<500: .medium
            default: .large
            }
            estimatedMinutes = min(60, max(2 + lines / 40, Int((Double(fileCount) / 3).rounded(.up))))
        } else {
            size = switch fileCount {
            case ...2: .small
            case ...8: .medium
            default: .large
            }
            estimatedMinutes = min(60, 2 + fileCount * 2)
        }
        testsTouched = paths.contains { $0.contains("test") || $0.contains("spec") || $0.contains("__tests__") }

        let onlyLockfiles = paths.allSatisfy { path in Self.lockfiles.contains { path.hasSuffix($0) } }
        if onlyLockfiles {
            flags = [.lockfileOnly]
        } else {
            flags = Flag.allCases.filter { flag in
                Self.patterns[flag].map { patterns in paths.contains { path in patterns.contains { path.contains($0) } } } ?? false
            }
        }
        topFiles = Array(changes.files.sorted { $0.lines > $1.lines }.prefix(3))
    }

    /// "~6 min · 4 files", the rest is shown as chips.
    public var summary: String {
        L("~%d min · %d files", estimatedMinutes, fileCount)
    }
}

/// The order of a review session: pressing first, then quick wins, then whoever waited longest.
public enum ReviewQueue {
    public static func order(_ items: [InboxItem], now: Date = .now) -> [InboxItem] {
        let prioritizer = Prioritizer(now: now)
        return items.sorted { a, b in
            let pressingA = prioritizer.isPressing(a), pressingB = prioritizer.isPressing(b)
            if pressingA != pressingB { return pressingA }
            let minutesA = ReviewPrep(a)?.estimatedMinutes ?? 30, minutesB = ReviewPrep(b)?.estimatedMinutes ?? 30
            if minutesA != minutesB { return minutesA < minutesB }
            return a.date < b.date
        }
    }

    public static func remainingMinutes(_ items: [InboxItem]) -> Int {
        items.reduce(0) { $0 + (ReviewPrep($1)?.estimatedMinutes ?? 10) }
    }
}

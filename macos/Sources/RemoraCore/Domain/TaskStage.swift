import Foundation

/// Where a ticket stands in its tracker (Linear, Jira), to order To do within one priority, first to last: someone
/// wrote to you on it, you started it, the current cycle or sprint plans it, the rest, it waits to be triaged,
/// another ticket blocks it, the backlog. Only the order: nothing is hidden, and an unknown stage stands with the
/// rest, so nothing moves on a guess. Its badge or status on the row says why. As Windows (`TaskStage`).
public enum TaskStage: String, Codable, Sendable, CaseIterable, Comparable {
    case discussed, started, planned, todo, triage, blocked, backlog

    public static func < (a: TaskStage, b: TaskStage) -> Bool {
        allCases.firstIndex(of: a)! < allCases.firstIndex(of: b)!
    }
}

extension TaskStage {
    /// A status that puts started work aside: "Paused", "On hold", "En pause". It ranks as not started.
    public static func paused(_ name: String?) -> Bool {
        guard let name = name?.lowercased() else { return false }
        return name.contains("pause") || name.contains("hold")
    }

    /// What a ticket that ages without being started counts as: open for weeks.
    public static let longOpen: TimeInterval = 14 * .day

    /// The badges that say why a ticket stands where it does, beyond its status.
    public static func badges(discussed: Bool, blocked: Bool, planned: Bool, longOpen: Bool, sprint: Bool) -> [Badge] {
        var badges: [Badge] = []
        if discussed {
            badges.append(Badge(id: .discussed, label: L("New comment"), symbol: "bubble.left", tone: .accent))
        }
        if blocked { badges.append(Badge(id: .blocked, label: L("Blocked"), symbol: "hand.raised", tone: .warning)) }
        if planned {
            badges.append(
                Badge(
                    id: .cycle, label: L(sprint ? "Current sprint" : "Current cycle"),
                    symbol: "arrow.triangle.2.circlepath", tone: .neutral))
        }
        if longOpen { badges.append(Badge(id: .age, label: L("Open for weeks"), symbol: "calendar", tone: .warning)) }
        return badges
    }
}

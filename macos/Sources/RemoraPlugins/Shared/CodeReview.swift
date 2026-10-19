import Foundation
import RemoraCore

/// Provider-neutral view of a pull/merge request, turned into inbox badges the same way everywhere.
struct CodeReview {
    enum Checks { case passing, failing, running, none }

    var isDraft = false
    var isApproved = false
    var changesRequested = false
    var hasConflicts = false
    var checks: Checks = .none
    var comments = 0
    var additions: Int?
    var deletions: Int?

    /// Review requests always need you; your own MR only when it's blocked or ready to merge.
    func needsAction(authored: Bool) -> Bool {
        guard !isDraft else { return false }
        guard authored else { return true }
        return isApproved || changesRequested || hasConflicts || checks == .failing
    }

    /// Someone else's request that is already approved: still yours to look at, last in its tab, but neither counted
    /// nor announced. Back to normal as soon as the approval goes (new commits, changes requested).
    func priority(authored: Bool) -> Priority? {
        !authored && isApproved && !changesRequested ? .low : nil
    }

    func badges(authored: Bool) -> [Badge] {
        var badges: [Badge] = []
        if isDraft {
            badges.append(Badge(id: .draft, label: L("Draft"), symbol: "pencil", tone: .neutral))
        }
        if isApproved {
            badges.append(
                Badge(
                    id: .approved, label: L("Approved"), symbol: "checkmark", tone: .accent,
                    notify: authored ? .init(title: L("Approved")) : nil
                ))
        }
        if changesRequested {
            badges.append(
                Badge(
                    id: .changes, label: L("Changes requested"), symbol: "exclamationmark.bubble", tone: .negative,
                    notify: authored ? .init(title: L("Changes requested")) : nil
                ))
        }
        if hasConflicts {
            badges.append(Badge(id: .conflicts, label: L("Conflicts"), symbol: "arrow.triangle.merge", tone: .warning))
        }
        switch checks {
        case .passing:
            badges.append(
                Badge(id: .checksPassing, label: L("Checks"), symbol: "checkmark.circle.fill", tone: .positive))
        case .failing:
            badges.append(
                Badge(
                    id: .checksFailing, label: L("Checks failed"), symbol: "xmark.octagon.fill", tone: .negative,
                    notify: authored ? .init(title: L("Checks failed")) : nil
                ))
        case .running:
            badges.append(Badge(id: .checksRunning, label: L("Running"), symbol: "clock", tone: .warning))
        case .none:
            break
        }
        if comments > 0 {
            badges.append(Badge(id: .comments, label: "\(comments)", symbol: "bubble.left", tone: .neutral))
        }
        if let additions, let deletions {
            badges.append(Badge(id: .diff, label: "+\(additions) −\(deletions)", tone: .neutral))
        }
        return badges
    }
}

import Foundation

/// A ticket whose work is up for review (Linear, Jira, GitHub issues, Azure DevOps, Notion): a pull or merge request
/// is open for it, or its status says so. It then waits on others, like your own pull request under review: the pull
/// request's row carries what it needs from you, and a comment on the ticket that asks something keeps its own row
/// in My turn. The same rules on Windows and Linux (`in_review.rs`).
public enum InReview {
    /// "In Review", "Code review", "Ready for review", "En revue", "Relecture"…
    public static func status(_ name: String?) -> Bool {
        guard let name = name?.lowercased() else { return false }
        return ["review", "revue", "relecture"].contains { name.contains($0) }
    }

    /// A pull or merge request page: GitHub `/pull/N`, GitLab `/-/merge_requests/N`, Bitbucket `/pull-requests/N`,
    /// Azure DevOps `/pullrequest/N`, on any host (Enterprise and self-hosted too).
    public static func isChange(_ url: URL) -> Bool {
        let parts = url.path.lowercased().split(separator: "/")
        return zip(parts, parts.dropFirst()).contains { kind, number in
            ["pull", "merge_requests", "pull-requests", "pullrequest"].contains(String(kind))
                && number.allSatisfy(\.isNumber)
        }
    }
}

extension InboxItem {
    /// Waiting on another ticket that blocks it: in Waiting until that one is done, when it comes back by itself.
    public func blockedByAnother() -> InboxItem {
        var item = self
        item.bundle = .blocked
        item.needsAction = false
        return item
    }

    /// Waiting on the review of `changes` (or on a review its status announces), tied to those pull requests.
    public func waitingOnReview(_ changes: [URL] = []) -> InboxItem {
        var item = self
        item.bundle = .awaiting
        item.needsAction = false
        if !changes.isEmpty { item.related = (related ?? []) + changes }
        return item
    }
}

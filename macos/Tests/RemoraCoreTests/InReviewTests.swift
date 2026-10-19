import Foundation
import Testing

@testable import RemoraCore

/// The same cases on Windows and Linux.
struct InReviewTests {
    @Test func reviewStatuses() {
        for name in ["In Review", "Code review", "Ready for review", "En revue", "Relecture"] {
            #expect(InReview.status(name), "\(name)")
        }
        #expect(!InReview.status("In Progress") && !InReview.status(nil))
    }

    @Test func pullAndMergeRequestPages() {
        for page in [
            "https://github.com/acme/app/pull/9", "https://gitlab.com/acme/app/-/merge_requests/3",
            "https://bitbucket.org/acme/app/pull-requests/4", "https://dev.azure.com/acme/Acme/_git/app/pullrequest/12",
        ] {
            #expect(InReview.isChange(URL(string: page)!), "\(page)")
        }
        #expect(!InReview.isChange(URL(string: "https://github.com/acme/app/issues/9")!))
        #expect(!InReview.isChange(URL(string: "https://github.com/acme/app/pull/new")!))
    }
}

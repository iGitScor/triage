import Foundation
import Testing

@testable import RemoraCore

/// Suggestions come from a model reading titles anyone can write; only reversible ones are ticked for you.
struct TriageSelectionTests {
    let suggestions = [
        TriageSuggestion(
            id: "a", action: .reschedule, until: .now.addingTimeInterval(86_400), reason: "Waiting on Bob"),
        TriageSuggestion(id: "b", action: .done, reason: "Stale"),
        TriageSuggestion(id: "c", action: .now, reason: "Due today"),
        TriageSuggestion(id: "d", action: .keep, reason: "Fine as is"),
    ]

    @Test func onlyReschedulesAreTickedByDefault() {
        #expect(TriageSuggestion.selection(suggestions, toggled: []).map(\.id) == ["a"])
    }

    @Test func theUserTicksDoneAndNowAndCanUntickAReschedule() {
        #expect(TriageSuggestion.selection(suggestions, toggled: ["b", "c"]).map(\.id) == ["a", "b", "c"])
        #expect(TriageSuggestion.selection(suggestions, toggled: ["a"]).isEmpty)
    }

    @Test func keepIsNeverApplied() {
        #expect(TriageSuggestion.selection(suggestions, toggled: ["d"]).map(\.id) == ["a"])
        #expect(TriageSuggestion.Action.allCases.filter(\.preselected) == [.reschedule])
    }
}

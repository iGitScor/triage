import Foundation
import RemoraCore

/// The Spotlight service: what this launch put in Spotlight, and the difference sent at each change.
/// Spotlight holds the inbox's titles and where they come from only while the user wants it, and never a blocked
/// tool's or one whose content is hidden. The demo indexes nothing. Mac only: Windows has no Spotlight.
@MainActor
final class SpotlightService {
    /// Nil until the first update, which starts the index over.
    private var indexed: [String: SpotlightEntry]?
    private let index: SpotlightIndexing
    private let isDemo: Bool

    init(index: SpotlightIndexing, demo: Bool) {
        self.index = index
        isDemo = demo
    }

    /// `items`: what may be indexed, refused tools left out. `sourceNames` names where each comes from.
    func update(items: [InboxItem], preferences: Preferences, sourceNames: [String: String]) {
        guard !isDemo else { return }
        let wanted = SpotlightIndex.entries(
            items: items, enabled: preferences.spotlight, hiddenContent: preferences.hiddenContentPlugins,
            sourceNames: sourceNames)
        let changes = SpotlightIndex.changes(indexed: indexed, wanted: wanted)
        if !changes.isEmpty { index.apply(changes) }
        indexed = Dictionary(wanted.map { ($0.id, $0) }, uniquingKeysWith: { first, _ in first })
    }

    /// A disconnected account's items leave Spotlight, whatever this launch knows of them.
    func remove(account id: UUID) {
        if !isDemo { index.delete(domain: id.uuidString) }
        indexed = indexed?.filter { $0.value.domain != id.uuidString }
    }

    /// Erase: whatever an earlier launch put in Spotlight goes too.
    func reset() {
        if !isDemo { index.apply(SpotlightIndex.Changes(deleteAll: true)) }
        indexed = [:]
    }
}

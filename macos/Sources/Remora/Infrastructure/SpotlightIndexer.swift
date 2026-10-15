import CoreSpotlight
import RemoraCore
import UniformTypeIdentifiers

/// This Mac's Spotlight index (IDEA-38): what the inbox puts there and takes back. A stand-in in tests.
@MainActor
protocol SpotlightIndexing {
    func apply(_ changes: SpotlightIndex.Changes)
    /// One account's items, when it is disconnected.
    func delete(domain: String)
}

/// Core Spotlight's index on this Mac. Requests are sent in order and not awaited: a failure leaves an entry that
/// Spotlight drops after a month, and the next launch starts the index over.
struct LiveSpotlight: SpotlightIndexing {
    private var index: CSSearchableIndex { .default() }

    func apply(_ changes: SpotlightIndex.Changes) {
        if changes.deleteAll { index.deleteAllSearchableItems(completionHandler: nil) }
        if !changes.delete.isEmpty {
            index.deleteSearchableItems(withIdentifiers: changes.delete, completionHandler: nil)
        }
        guard !changes.index.isEmpty else { return }
        let items = changes.index.map { entry in
            let attributes = CSSearchableItemAttributeSet(contentType: .text)
            attributes.title = entry.title
            attributes.displayName = entry.title
            attributes.contentDescription = entry.context
            return CSSearchableItem(
                uniqueIdentifier: entry.id, domainIdentifier: entry.domain, attributeSet: attributes)
        }
        index.indexSearchableItems(items, completionHandler: nil)
    }

    func delete(domain: String) {
        index.deleteSearchableItems(withDomainIdentifiers: [domain], completionHandler: nil)
    }
}

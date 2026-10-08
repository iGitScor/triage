import Foundation
import Testing
@testable import RemoraCore

struct PrioritizerTests {
    func item(_ id: String, priority: Priority? = nil, due: Date? = nil, minutesAgo: Double = 0) -> InboxItem {
        var item = makeItem(id, bundle: .tasks, date: now.addingTimeInterval(-minutesAgo * 60))
        item.priority = priority
        item.due = due
        return item
    }

    @Test func pressingThenPriorityThenDueThenRecent() {
        let items = [
            item("recent", minutesAgo: 1),
            item("high", priority: .high, minutesAgo: 60),
            item("overdue", due: now - 2 * 86_400, minutesAgo: 600),
            item("low", priority: .low),
            item("dueSoon", due: now + 2 * 86_400, minutesAgo: 300),
            item("urgent", priority: .urgent, minutesAgo: 900),
        ]
        #expect(Prioritizer(now: now).sorted(items).map(\.id) == ["overdue", "urgent", "high", "dueSoon", "recent", "low"])
    }

    @Test func lowPriorityIsShownButNotCounted() {
        let layout = InboxAssembler().layout(items: [item("a", priority: .low), item("b")], states: [:], now: now)
        #expect(layout.myTurnItems.count == 2)
        #expect(layout.actionCount == 1)
    }
}

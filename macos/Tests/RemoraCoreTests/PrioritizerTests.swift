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
        #expect(
            Prioritizer(now: now).sorted(items).map(\.id) == ["overdue", "urgent", "high", "dueSoon", "recent", "low"])
    }

    /// Within one priority, where a ticket stands: a comment for you, started, planned now, the rest (an unknown
    /// stage among them), triage, blocked, backlog. Priority still comes first. As Windows.
    @Test func withinAPriorityTheStageOrders() {
        func ticket(_ id: String, _ stage: TaskStage?, priority: Priority? = nil) -> InboxItem {
            var item = item(id, priority: priority, minutesAgo: Double(id.count))
            item.stage = stage
            return item
        }
        let items = [
            ticket("backlog", .backlog), ticket("blocked", .blocked), ticket("triage", .triage),
            ticket("unknown", nil), ticket("todo", .todo), ticket("planned", .planned), ticket("started", .started),
            ticket("discussed", .discussed), ticket("highTodo", .todo, priority: .high),
        ]
        #expect(
            Prioritizer(now: now).sorted(items).map(\.id) == [
                "highTodo", "discussed", "started", "planned", "todo", "unknown", "triage", "blocked", "backlog",
            ])
    }

    /// A group in My turn says how many of its rows aren't counted, and "+" when a list stopped at 50; Waiting and
    /// To read just say how many. The menu bar never counts Waiting, whatever the setting. As Windows.
    @Test func headersSayWhatIsLaterAndTheMenuBarSkipsWaiting() {
        let rows = [item("a"), item("b", priority: .low), item("c", priority: .low)]
        #expect(InboxLayout.headerCount(rows, mine: true, cut: false) == "1 · 2 later")
        #expect(InboxLayout.headerCount(rows, mine: true, cut: true) == "1+ · 2+ later")
        #expect(InboxLayout.headerCount(Array(rows.dropFirst()), mine: true, cut: false) == "2 later")
        #expect(InboxLayout.headerCount(rows, mine: false, cut: false) == "3")
        #expect(InboxLayout.headerCount([makeItem("r", bundle: .read)], mine: true, cut: false) == "1")
        var waiting = item("w")
        waiting.bundle = .awaiting
        let layout = InboxAssembler().layout(items: rows + [waiting], states: [:], now: now)
        #expect(layout.countsBySource(actionableOnly: false).map(\.count) == [3])
        #expect(layout.countsBySource(actionableOnly: true).map(\.count) == [1])
    }

    @Test func lowPriorityIsShownButNotCounted() {
        let layout = InboxAssembler().layout(items: [item("a", priority: .low), item("b")], states: [:], now: now)
        #expect(layout.myTurnItems.count == 2)
        #expect(layout.actionCount == 1)
    }
}

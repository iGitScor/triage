import RemoraCore
import SwiftUI

extension InboxView {
    // MARK: Keyboard

    /// The items in the order they're shown on this tab, skipping collapsed bundles and folded rows.
    func navigationOrder(_ layout: InboxLayout) -> [InboxItem] {
        switch tab {
        case .inProgress: layout.inProgress
        case .myTurn:
            shown(id: "pinned", layout.pinned)
                + layout.myTurn.flatMap { shown(id: "\(Tab.myTurn.rawValue)/\($0.id)", $0.items) }
        case .waiting: layout.waiting.flatMap { shown(id: "\(Tab.waiting.rawValue)/\($0.id)", $0.items) }
        case .snoozed: layout.snoozed
        case .done: layout.done
        }
    }

    /// A bundle's rows as displayed: none when collapsed, the first few when long and not expanded.
    func shown(id: String, _ items: [InboxItem]) -> [InboxItem] {
        if collapsed.contains(id) { return [] }
        let folded = items.count > visibleInBundle + 1 && !expanded.contains(id)
        return folded ? Array(items.prefix(visibleInBundle)) : items
    }

    /// ↑↓ or J K move, ⏎ opens (⌥⏎ in the browser), E done, S snooze, P pin, ⌘F search, ⌘R refresh.
    /// Any other letter starts a search, as typing in the popover did before.
    func handle(_ press: KeyPress) -> KeyPress.Result {
        guard picker == nil, draft == nil, !reviewing else { return .ignored }
        if press.modifiers.contains(.command) {
            switch press.characters {
            case "f": focus = .search
            case "r": Task { await model.refresh(manual: true) }
            default: return .ignored
            }
            return .handled
        }
        let items = navigationOrder(model.visibleLayout)
        let index = selection.flatMap { id in items.firstIndex { $0.id == id } }
        let current = index.map { items[$0] }
        func move(_ step: Int) {
            guard !items.isEmpty else { return }
            let next = index.map { $0 + step } ?? (step > 0 ? 0 : items.count - 1)
            selection = items[min(max(next, 0), items.count - 1)].id
        }
        /// After an action that takes the item off this list, the keyboard lands on its neighbour.
        func leaving(_ action: (InboxItem) -> Void) {
            guard let current, let index else { return }
            let neighbour =
                items.indices.contains(index + 1) ? items[index + 1].id : (index > 0 ? items[index - 1].id : nil)
            withAnimation(.snappy) { action(current) }
            selection = navigationOrder(model.visibleLayout).contains { $0.id == current.id } ? current.id : neighbour
        }
        switch press.key {
        case .downArrow: move(1)
        case .upArrow: move(-1)
        case .return:
            guard let current else { return .ignored }
            model.open(current, inBrowser: press.modifiers.contains(.option))
        default:
            guard press.modifiers.isEmpty || press.modifiers == .shift else { return .ignored }
            switch press.characters.lowercased() {
            case "j": move(1)
            case "k": move(-1)
            case "e": leaving { model.toggleDone($0) }
            case "s": if let current { picker = .snooze(current) }
            case "p": if let current { model.togglePin(current) }
            default:
                guard let first = press.characters.first, first.isLetter || first.isNumber else { return .ignored }
                model.query += press.characters
                focus = .search
            }
        }
        return .handled
    }
}

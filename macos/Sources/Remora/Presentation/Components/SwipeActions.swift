import AppKit
import RemoraCore
import SwiftUI

struct SwipeAction {
    var label: String
    var symbol: String
    var tint: Color
    var foreground: Color
    /// Slides the row out before running, for actions that remove it from the list.
    var dismisses: Bool
    var perform: () -> Void
}

extension View {
    /// Two-finger trackpad swipes: right runs `leading`, left runs `trailing`.
    func swipeActions(leading: SwipeAction, trailing: SwipeAction) -> some View {
        modifier(SwipeActionsModifier(leading: leading, trailing: trailing))
    }
}

/// Reads horizontal scroll events while the pointer is over the row. SwiftUI's `swipeActions`
/// only works inside `List` on macOS.
private struct SwipeActionsModifier: ViewModifier {
    let leading: SwipeAction
    let trailing: SwipeAction

    @State private var offset: CGFloat = 0
    @State private var width: CGFloat = 400
    @State private var monitor: Any?
    @State private var direction: Direction = .undecided
    @State private var hovering = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    private enum Direction { case undecided, horizontal, vertical }

    private let threshold: CGFloat = 90
    private let limit: CGFloat = 180

    func body(content: Content) -> some View {
        content
            .offset(x: offset)
            .background { revealed }
            .clipShape(RoundedRectangle(cornerRadius: Myna.radiusMedium, style: .continuous))
            .onGeometryChange(for: CGFloat.self) { $0.size.width } action: { width = $0 }
            .onHover { inside in
                hovering = inside
                inside ? startMonitoring() : stopMonitoring()
            }
            // A row that leaves mid-swipe (Done slides it out) takes its monitor with it.
            .onDisappear(perform: removeMonitor)
    }

    @ViewBuilder private var revealed: some View {
        if offset != 0 {
            let action = offset > 0 ? leading : trailing
            let armed = abs(offset) >= threshold
            HStack {
                if offset < 0 { Spacer() }
                Label(L(action.label), systemImage: action.symbol)
                    .font(Myna.font(13, armed ? .bold : .medium))
                    .scaleEffect(armed ? 1.08 : 1)
                    .opacity(min(1, abs(offset) / threshold))
                if offset > 0 { Spacer() }
            }
            .foregroundStyle(action.foreground)
            .padding(.horizontal, 20)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .background(armed ? action.tint : action.tint.opacity(0.55))
            .animation(.snappy(duration: 0.15), value: armed)
        }
    }

    private func startMonitoring() {
        guard monitor == nil else { return }
        monitor = NSEvent.addLocalMonitorForEvents(matching: .scrollWheel) { event in
            handle(event) ? nil : event
        }
    }

    /// When the pointer leaves: at once, or when the swipe in progress ends (`handle`).
    private func stopMonitoring() {
        guard direction != .horizontal else { return }
        removeMonitor()
    }

    private func removeMonitor() {
        if let monitor { NSEvent.removeMonitor(monitor) }
        monitor = nil
    }

    /// Returns true when the event was consumed by the swipe.
    private func handle(_ event: NSEvent) -> Bool {
        guard event.hasPreciseScrollingDeltas else { return false }
        if !event.momentumPhase.isEmpty { return direction == .horizontal }

        switch event.phase {
        case .began, .mayBegin:
            direction = .undecided
            return false
        case .changed:
            if direction == .undecided, event.scrollingDeltaX != 0 || event.scrollingDeltaY != 0 {
                direction = abs(event.scrollingDeltaX) > abs(event.scrollingDeltaY) ? .horizontal : .vertical
            }
            guard direction == .horizontal else { return false }
            let delta = event.isDirectionInvertedFromDevice ? event.scrollingDeltaX : -event.scrollingDeltaX
            move(by: delta)
            return true
        case .ended, .cancelled:
            let wasHorizontal = direction == .horizontal
            direction = .undecided
            if wasHorizontal { finish() }
            // The swipe ended after the pointer left the row: nothing keeps the monitor any more.
            if !hovering { removeMonitor() }
            return wasHorizontal
        default:
            return false
        }
    }

    private func move(by delta: CGFloat) {
        let wasArmed = abs(offset) >= threshold
        offset = max(-limit, min(limit, offset + delta))
        if wasArmed != (abs(offset) >= threshold) {
            NSHapticFeedbackManager.defaultPerformer.perform(.alignment, performanceTime: .now)
        }
    }

    private func finish() {
        guard abs(offset) >= threshold else {
            withAnimation(reduceMotion ? nil : .spring(duration: 0.3)) { offset = 0 }
            return
        }
        let action = offset > 0 ? leading : trailing
        if reduceMotion {
            // No slide-out: the action runs and the row goes back in place at once.
            offset = 0
            action.perform()
        } else if action.dismisses {
            withAnimation(.easeIn(duration: 0.18)) { offset = offset > 0 ? width : -width }
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.18) {
                action.perform()
                offset = 0
            }
        } else {
            withAnimation(.spring(duration: 0.3)) { offset = 0 }
            action.perform()
        }
    }
}

import AppKit
import SwiftUI

/// A borderless panel that can take keyboard input without activating the whole app.
final class FloatingPanel: NSPanel {
    init(interactive: Bool) {
        super.init(
            contentRect: .zero,
            styleMask: interactive ? [.borderless, .nonactivatingPanel] : [.borderless],
            backing: .buffered,
            defer: true
        )
        level = interactive ? .floating : .statusBar
        isOpaque = false
        backgroundColor = .clear
        hasShadow = true
        ignoresMouseEvents = !interactive
        isReleasedWhenClosed = false
        hidesOnDeactivate = false
        collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
    }

    override var canBecomeKey: Bool { !ignoresMouseEvents }

    func host(_ view: some View) {
        let hosting = NSHostingView(rootView: view)
        contentView = hosting
        setContentSize(hosting.fittingSize)
    }

    /// Places the panel's top-left corner near `point`, kept inside the screen.
    func place(near point: NSPoint, offset: NSPoint = NSPoint(x: 16, y: -12)) {
        let screen = NSScreen.screens.first { $0.frame.contains(point) } ?? NSScreen.main
        let bounds = screen?.visibleFrame ?? .zero
        var origin = NSPoint(x: point.x + offset.x, y: point.y + offset.y - frame.height)
        origin.x = min(max(origin.x, bounds.minX + 8), bounds.maxX - frame.width - 8)
        origin.y = min(max(origin.y, bounds.minY + 8), bounds.maxY - frame.height - 8)
        setFrameOrigin(origin)
    }
}

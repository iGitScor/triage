import AppKit

/// While dragging down from the menu bar icon: a line from the icon to the cursor, with a float and a hook.
/// Plain AppKit, like `DragHUD`, so it redraws inside the mouse-tracking loop.
@MainActor
final class FishingLine {
    private let panel = FloatingPanel(interactive: false)
    private let view = LineView()

    init() {
        panel.contentView = view
        panel.hasShadow = false
    }

    func show(from anchor: NSPoint, to tip: NSPoint, dark: Bool) {
        let frame = NSRect(
            x: min(anchor.x, tip.x) - 24, y: min(anchor.y, tip.y) - 24,
            width: abs(anchor.x - tip.x) + 48, height: abs(anchor.y - tip.y) + 48
        )
        panel.setFrame(frame, display: false)
        view.frame = NSRect(origin: .zero, size: frame.size)
        view.anchor = NSPoint(x: anchor.x - frame.minX, y: anchor.y - frame.minY)
        view.tip = NSPoint(x: tip.x - frame.minX, y: tip.y - frame.minY)
        view.dark = dark
        view.needsDisplay = true
        panel.orderFrontRegardless()
        panel.display()
        CATransaction.flush()
    }

    func hide() {
        panel.orderOut(nil)
    }
}

private final class LineView: NSView {
    var anchor = NSPoint.zero
    var tip = NSPoint.zero
    var dark = false

    override func draw(_ dirtyRect: NSRect) {
        let ink = (dark ? Myna.onDark : Myna.ink).resolved(dark: false)

        // A slightly slack line.
        let line = NSBezierPath()
        line.move(to: anchor)
        let middle = NSPoint(x: (anchor.x + tip.x) / 2 + 6, y: (anchor.y + tip.y) / 2 - 4)
        line.curve(to: tip, controlPoint1: middle, controlPoint2: middle)
        line.lineWidth = 1.4
        ink.withAlphaComponent(0.8).setStroke()
        line.stroke()

        // The float, two thirds down the line.
        let float = NSPoint(x: anchor.x + (tip.x - anchor.x) * 0.66, y: anchor.y + (tip.y - anchor.y) * 0.66)
        let bobber = NSBezierPath(ovalIn: NSRect(x: float.x - 5, y: float.y - 5, width: 10, height: 10))
        Myna.accent.resolved(dark: dark).setFill()
        bobber.fill()
        ink.setStroke()
        bobber.lineWidth = 1.2
        bobber.stroke()

        // The hook.
        let hook = NSBezierPath()
        hook.move(to: tip)
        hook.line(to: NSPoint(x: tip.x, y: tip.y - 9))
        hook.appendArc(
            withCenter: NSPoint(x: tip.x - 4.5, y: tip.y - 9), radius: 4.5, startAngle: 0, endAngle: 180,
            clockwise: true)
        hook.line(to: NSPoint(x: tip.x - 9, y: tip.y - 6))
        hook.lineWidth = 1.8
        hook.lineCapStyle = .round
        hook.lineJoinStyle = .round
        ink.setStroke()
        hook.stroke()
    }
}

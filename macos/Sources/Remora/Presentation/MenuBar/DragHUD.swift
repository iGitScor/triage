import AppKit

/// The bubble that follows the cursor while dragging from the menu bar icon.
/// Plain AppKit on purpose: it must redraw inside the mouse-tracking loop, where SwiftUI doesn't update.
@MainActor
final class DragHUD {
    private static let size = NSSize(width: 230, height: 54)
    private static let barWidth: CGFloat = 200

    /// Myna tokens for one appearance, high-contrast variants included.
    private struct Palette {
        var background, border, text, icon, track, fill: NSColor

        @MainActor init(dark: Bool, contrast: Bool) {
            background = (dark ? Myna.dark : Myna.surface).resolved(dark: false)
            border = Myna.border.resolved(dark: dark)
            text = (dark ? Myna.onDark : Myna.ink).resolved(dark: false)
            icon = Myna.accentText.resolved(dark: dark)
            track = text.withAlphaComponent(contrast ? 0.35 : dark ? 0.15 : 0.1)
            fill = (dark ? Myna.accent : Myna.accentDeep).resolved(dark: false)
        }
    }

    private let panel = FloatingPanel(interactive: false)
    private let background = NSView(frame: NSRect(origin: .zero, size: size))
    private let icon = NSImageView(frame: NSRect(x: 14, y: 24, width: 18, height: 18))
    private let label = NSTextField(labelWithString: "")
    private let track = CALayer()
    private let fill = CALayer()
    private var applied: (dark: Bool, contrast: Bool)?

    init() {
        background.wantsLayer = true
        background.layer?.cornerRadius = 12
        background.layer?.borderWidth = 1

        label.frame = NSRect(x: 38, y: 22, width: Self.size.width - 50, height: 22)
        label.font = Fonts.nsOutfit(size: 15, weight: .semibold) ?? .systemFont(ofSize: 15, weight: .semibold)

        track.frame = CGRect(x: 15, y: 12, width: Self.barWidth, height: 4)
        track.cornerRadius = 2
        fill.frame = CGRect(x: 15, y: 12, width: 4, height: 4)
        fill.cornerRadius = 2
        background.layer?.addSublayer(track)
        background.layer?.addSublayer(fill)

        background.addSubview(icon)
        background.addSubview(label)
        panel.contentView = background
        panel.setContentSize(Self.size)
    }

    func show(_ text: String, progress: Double, near point: NSPoint, dark: Bool) {
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        apply(dark: dark)
        label.stringValue = text
        fill.frame.size.width = max(4, Self.barWidth * progress)
        CATransaction.commit()
        panel.place(near: point)
        panel.orderFrontRegardless()
        panel.display()
        CATransaction.flush()
    }

    func hide() {
        panel.orderOut(nil)
    }

    private func apply(dark: Bool) {
        let contrast = NSWorkspace.shared.accessibilityDisplayShouldIncreaseContrast
        if let applied, applied == (dark, contrast) { return }
        applied = (dark, contrast)
        let palette = Palette(dark: dark, contrast: contrast)
        background.layer?.backgroundColor = palette.background.cgColor
        background.layer?.borderColor = palette.border.cgColor
        label.textColor = palette.text
        track.backgroundColor = palette.track.cgColor
        fill.backgroundColor = palette.fill.cgColor
        icon.image = NSImage(systemSymbolName: "alarm.fill", accessibilityDescription: nil)?
            .withSymbolConfiguration(.init(pointSize: 13, weight: .bold).applying(.init(paletteColors: [palette.icon])))
    }
}

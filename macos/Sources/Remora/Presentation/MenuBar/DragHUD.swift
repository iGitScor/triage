import AppKit

/// The bubble that follows the cursor while dragging from the menu bar icon.
/// Plain AppKit on purpose: it must redraw inside the mouse-tracking loop, where SwiftUI doesn't update.
@MainActor
final class DragHUD {
    private static let size = NSSize(width: 230, height: 54)
    private static let barWidth: CGFloat = 200

    /// Myna tokens for one appearance.
    private struct Palette {
        var background, border, text, icon, track, fill: NSColor

        static let light = Palette(
            background: NSColor(hex: 0xF7F7F1), border: NSColor(hex: 0xD0D0C8), text: NSColor(hex: 0x111111),
            icon: NSColor(hex: 0x356509), track: NSColor(hex: 0x111111, alpha: 0.1), fill: NSColor(hex: 0x9DE049)
        )
        static let dark = Palette(
            background: NSColor(hex: 0x111111), border: NSColor(hex: 0x2A2A3A), text: NSColor(hex: 0xF0F0E8),
            icon: NSColor(hex: 0xB9FF66), track: NSColor(hex: 0xF0F0E8, alpha: 0.15), fill: NSColor(hex: 0xB9FF66)
        )
    }

    private let panel = FloatingPanel(interactive: false)
    private let background = NSView(frame: NSRect(origin: .zero, size: size))
    private let icon = NSImageView(frame: NSRect(x: 14, y: 24, width: 18, height: 18))
    private let label = NSTextField(labelWithString: "")
    private let track = CALayer()
    private let fill = CALayer()
    private var isDark: Bool?

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
        guard dark != isDark else { return }
        isDark = dark
        let palette = dark ? Palette.dark : .light
        background.layer?.backgroundColor = palette.background.cgColor
        background.layer?.borderColor = palette.border.cgColor
        label.textColor = palette.text
        track.backgroundColor = palette.track.cgColor
        fill.backgroundColor = palette.fill.cgColor
        icon.image = NSImage(systemSymbolName: "alarm.fill", accessibilityDescription: nil)?
            .withSymbolConfiguration(.init(pointSize: 13, weight: .bold).applying(.init(paletteColors: [palette.icon])))
    }
}

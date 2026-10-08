import AppKit
import Observation
import RemoraCore
import RemoraPlugins
import SwiftUI

/// The menu bar icon. Click opens the inbox; drag down sets a reminder, Gestimer style.
@MainActor
final class StatusItemController: NSObject {
    private enum Gesture {
        case click
        case cancelled
        case picked(Date, at: NSPoint)
    }

    private let model: InboxModel
    private let statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
    private let popover = NSPopover()
    private let clock = SnoozeClock()
    private let hud = DragHUD()
    private let line = FishingLine()
    private let quickAdd: QuickReminderPanel

    init(model: InboxModel) {
        self.model = model
        quickAdd = QuickReminderPanel(model: model)
        super.init()

        popover.behavior = .transient
        popover.contentSize = NSSize(width: 400, height: 600)
        popover.contentViewController = NSHostingController(rootView: InboxView().environment(model))

        if let button = statusItem.button {
            button.target = self
            button.action = #selector(pressed)
            button.sendAction(on: [.leftMouseDown, .rightMouseDown])
            button.imagePosition = .imageLeading
        }
        updateLabel()
        SettingsOpener.install()
    }

    private func updateLabel() {
        let counts = withObservationTracking {
            menuBarCounts()
        } onChange: {
            Task { @MainActor [weak self] in self?.updateLabel() }
        }
        let branded = withObservationTracking { model.preferences.brandedMenuBar } onChange: {
            Task { @MainActor [weak self] in self?.updateLabel() }
        }
        guard let button = statusItem.button else { return }
        let total = counts.reduce(0) { $0 + $1.count }
        if branded, total > 0 {
            button.image = MenuBarTitle.pill(counts)
            button.attributedTitle = NSAttributedString()
        } else {
            button.image = RemoraArt.glyph()
            button.attributedTitle = MenuBarTitle.make(counts)
        }
        button.image?.accessibilityDescription = "Remora"
        button.toolTip = L("Click to open Remora, drag down to add a reminder, right-click for more")
    }

    /// One entry for the total, or one per source when the counter is split.
    private func menuBarCounts() -> [(pluginID: String?, count: Int)] {
        let preferences = model.preferences
        guard preferences.menuBarCount != .hidden else { return [] }
        let counts = model.layout.countsBySource(actionableOnly: preferences.menuBarCount == .waitingOnMe)
        if preferences.countPerSource {
            return counts.map { ($0.pluginID, $0.count) }
        }
        let total = counts.reduce(0) { $0 + $1.count }
        return total > 0 ? [(nil, total)] : []
    }

    @objc private func pressed() {
        if NSApp.currentEvent?.type == .rightMouseDown {
            showMenu()
            return
        }
        switch trackGesture() {
        case .click: togglePopover()
        case .cancelled: break
        case .picked(let date, let point): quickAdd.show(at: point, date: date)
        }
    }

    private func showMenu() {
        guard let button = statusItem.button else { return }
        let menu = NSMenu()
        menu.addItem(item(L("Open Remora"), symbol: "tray", key: "") { [weak self] in self?.togglePopover() })
        menu.addItem(item(L("New reminder…"), symbol: "alarm", key: "") { [weak self] in self?.newReminder() })
        menu.addItem(item(L("Refresh now"), symbol: "arrow.clockwise", key: "r") { [weak self] in
            Task { await self?.model.refresh() }
        })
        menu.addItem(.separator())
        menu.addItem(item(L("Settings…"), symbol: "gearshape", key: ",") { SettingsOpener.open() })
        menu.addItem(.separator())
        menu.addItem(item(L("Quit Remora"), symbol: "power", key: "q") { NSApp.terminate(nil) })
        menu.popUp(positioning: nil, at: NSPoint(x: 0, y: button.bounds.height + 4), in: button)
    }

    private func item(_ title: String, symbol: String, key: String, action: @escaping () -> Void) -> NSMenuItem {
        let item = MenuAction(title: title, keyEquivalent: key, action: action)
        item.image = NSImage(systemSymbolName: symbol, accessibilityDescription: nil)
        return item
    }

    /// The quick-add box under the icon, preset to one hour from now.
    private func newReminder() {
        guard let window = statusItem.button?.window else { return }
        let point = NSPoint(x: window.frame.midX, y: window.frame.minY - 8)
        quickAdd.show(at: point, date: .now.addingTimeInterval(3_600))
    }

    private func togglePopover() {
        guard let button = statusItem.button else { return }
        if popover.isShown {
            popover.performClose(nil)
        } else {
            popover.show(relativeTo: button.bounds, of: button, preferredEdge: .minY)
            popover.contentViewController?.view.window?.makeKey()
        }
    }

    /// Follows the mouse until release. The further down, the later the reminder.
    private func trackGesture() -> Gesture {
        let start = NSEvent.mouseLocation
        let range = max(200, (NSScreen.main?.visibleFrame.height ?? 800) * 0.7)
        var picked: Date?
        let anchor = statusItem.button?.window.map { NSPoint(x: $0.frame.midX, y: $0.frame.minY + 2) } ?? start
        defer {
            hud.hide()
            line.hide()
        }

        while let event = NSApp.nextEvent(
            matching: [.leftMouseDragged, .leftMouseUp, .keyDown],
            until: .distantFuture,
            inMode: .eventTracking,
            dequeue: true
        ) {
            let location = NSEvent.mouseLocation
            switch event.type {
            case .keyDown where event.keyCode == 53:
                return .cancelled
            case .leftMouseUp:
                guard let picked else { return .click }
                return .picked(picked, at: location)
            case .leftMouseDragged:
                let progress = (start.y - location.y) / range
                guard picked != nil || progress * range > 8 else { continue }
                let date = Date.now.addingTimeInterval(clock.duration(at: progress))
                picked = date
                let dark = model.preferences.appearance.isDark
                line.show(from: anchor, to: location, dark: dark)
                hud.show(clock.describe(date), progress: min(max(progress, 0), 1), near: location, dark: dark)
            default:
                continue
            }
        }
        return .cancelled
    }
}

/// Builds "3" or "[github] 3  [slack] 4" for the status item, symbols drawn as template images.
enum MenuBarTitle {
    /// Myna branding: an ink fish and counters on a lime capsule, shown when something needs you.
    /// Not a template image, so it keeps its colors on light and dark menu bars.
    static func pill(_ counts: [(pluginID: String?, count: Int)], height: CGFloat = 18) -> NSImage {
        let ink = NSColor(hex: 0x111111)
        let title = make(counts, color: ink)
        let fish = NSSize(width: 22, height: 11)
        let text = title.size()
        let size = NSSize(width: 7 + fish.width + text.width + 8, height: height)
        let image = NSImage(size: size, flipped: false) { rect in
            guard let context = NSGraphicsContext.current?.cgContext else { return false }
            NSColor(hex: 0xB9FF66).setFill()
            NSBezierPath(roundedRect: rect, xRadius: height / 2, yRadius: height / 2).fill()
            RemoraArt.drawCropped(
                in: CGRect(x: 7, y: (height - fish.height) / 2, width: fish.width, height: fish.height),
                color: ink, context: context
            )
            title.draw(at: NSPoint(x: 7 + fish.width, y: (height - text.height) / 2))
            return true
        }
        image.isTemplate = false
        return image
    }

    /// "3" or "[github] 3  [slack] 4"; adapts to the menu bar unless a color is given.
    static func make(_ counts: [(pluginID: String?, count: Int)], color: NSColor? = nil) -> NSAttributedString {
        let font = NSFont.monospacedDigitSystemFont(ofSize: 13, weight: .medium)
        var attributes: [NSAttributedString.Key: Any] = [.font: font]
        if let color { attributes[.foregroundColor] = color }
        let title = NSMutableAttributedString()
        for (index, entry) in counts.enumerated() {
            title.append(NSAttributedString(string: index == 0 ? " " : "  ", attributes: attributes))
            if let pluginID = entry.pluginID, let image = ToolIcon.image(pluginID: pluginID, pointSize: 12, color: color) {
                let attachment = NSTextAttachment()
                attachment.image = image
                attachment.bounds = CGRect(x: 0, y: -1.5, width: image.size.width, height: image.size.height)
                title.append(NSAttributedString(attachment: attachment))
                title.append(NSAttributedString(string: "\u{2009}", attributes: attributes))
            }
            title.append(NSAttributedString(string: "\(entry.count)", attributes: attributes))
        }
        return title
    }
}

/// A menu item that runs a closure.
private final class MenuAction: NSMenuItem {
    private let run: () -> Void

    init(title: String, keyEquivalent: String, action: @escaping () -> Void) {
        run = action
        super.init(title: title, action: #selector(runAction), keyEquivalent: keyEquivalent)
        target = self
    }

    required init(coder: NSCoder) { fatalError("not used") }

    @objc private func runAction() { run() }
}

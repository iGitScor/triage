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
    /// What the label shows, kept so the animation can redraw it without re-reading the model.
    private var label = MenuBarTitle.Content()
    /// Runs only while a task is in progress: a slow tick for the elapsed time, which starts each wiggle.
    private var animation: Task<Void, Never>?
    /// The frames of one wiggle (0.8 s), then it stops: nothing wakes the Mac between wiggles.
    private var burst: Task<Void, Never>?
    /// Frames already drawn for the current label, by motion; cleared when the label changes.
    private var frames: [MenuBarTitle.Motion: (image: NSImage, title: NSAttributedString)] = [:]
    private var shown: MenuBarTitle.Motion?
    private var shownMinute = -1

    init(model: InboxModel) {
        self.model = model
        quickAdd = QuickReminderPanel(model: model)
        super.init()

        popover.behavior = .transient
        popover.contentSize = InboxView.size
        popover.contentViewController = NSHostingController(rootView: InboxView().environment(model))

        if let button = statusItem.button {
            button.target = self
            button.action = #selector(pressed)
            button.sendAction(on: [.leftMouseDown, .rightMouseDown])
            button.imagePosition = .imageLeading
            // Dragging down to set a reminder needs a pointer: VoiceOver gets it as an action.
            button.setAccessibilityCustomActions([
                NSAccessibilityCustomAction(name: L("New reminder…")) { [weak self] in
                    self?.newReminder()
                    return true
                }
            ])
        }
        updateLabel()
        SettingsOpener.install()
        NSWorkspace.shared.notificationCenter.addObserver(
            forName: NSWorkspace.accessibilityDisplayOptionsDidChangeNotification, object: nil, queue: .main
        ) { [weak self] _ in
            // Not updateLabel: that would add a second observation of the model.
            MainActor.assumeIsolated {
                self?.scheduleAnimation()
                self?.render()
            }
        }
    }

    private func updateLabel() {
        // One tracking block for everything the label reads: a block per value would each schedule
        // their own update on the same change, and multiply.
        let (allCounts, branded, running, focus, startedAt) = withObservationTracking {
            let layout = model.layout
            return (
                menuBarCounts(layout),
                model.preferences.brandedMenuBar,
                layout.inProgress.count,
                model.preferences.focusWhileInProgress,
                layout.inProgress.first.flatMap { model.state(of: $0).startedAt }
            )
        } onChange: {
            Task { @MainActor [weak self] in self?.updateLabel() }
        }
        // Focus: while you're on something, the other counters wait.
        let focused = focus && running > 0
        let counts = focused ? [] : allCounts
        label = MenuBarTitle.Content(
            counts: counts, running: running, since: startedAt, branded: branded, focused: focused)
        frames = [:]
        shown = nil
        scheduleAnimation()
        render()
    }

    private func render(_ motion: MenuBarTitle.Motion = .still) {
        guard let button = statusItem.button else { return }
        // VoiceOver can't read a count drawn into an image: say it, and the task in progress.
        button.setAccessibilityLabel(MenuBarTitle.spokenText(label))
        // The elapsed time changes once a minute: frames drawn before are stale after that.
        let minute = label.since.map { Int(Date.now.timeIntervalSince($0) / 60) } ?? 0
        if minute != shownMinute {
            frames = [:]
            shown = nil
            shownMinute = minute
        }
        guard motion != shown else { return }
        shown = motion
        let frame = frames[motion] ?? draw(motion)
        frames[motion] = frame
        button.image = frame.image
        button.attributedTitle = frame.title
        button.image?.accessibilityDescription = "Remora"
        let hint = L("Click to open Remora, drag down to add a reminder, right-click for more")
        button.toolTip = label.running > 0 ? MenuBarTitle.runningText(label) + "\n" + hint : hint
    }

    private func draw(_ motion: MenuBarTitle.Motion) -> (image: NSImage, title: NSAttributedString) {
        let total = label.counts.reduce(0) { $0 + $1.count }
        if label.branded, total > 0 || label.running > 0 {
            return (MenuBarTitle.pill(label, motion: motion), NSAttributedString())
        }
        return (RemoraArt.glyph(), MenuBarTitle.make(label, motion: motion))
    }

    /// The fish swims now and then, unless the system asks for less motion.
    /// Either way the elapsed time keeps up.
    private func scheduleAnimation() {
        animation?.cancel()
        burst?.cancel()
        animation = nil
        burst = nil
        guard label.running > 0 else { return }
        let still = NSWorkspace.shared.accessibilityDisplayShouldReduceMotion
        if !still { wiggle() }
        // Every 10 s the fish wiggles and the elapsed time catches up; with Reduce Motion, only the time, every 30 s.
        // A task on the main actor rather than a timer forced onto it.
        animation = Task { @MainActor [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(still ? 30 : 10), tolerance: .seconds(still ? 5 : 1))
                guard !Task.isCancelled else { return }
                if still { self?.render(.still) } else { self?.wiggle() }
            }
        }
    }

    /// About seven frames over 0.8 s, then the burst stops and the fish rests.
    private func wiggle() {
        burst?.cancel()
        let start = Date.now
        burst = Task { @MainActor [weak self] in
            while !Task.isCancelled {
                let elapsed = Date.now.timeIntervalSince(start)
                guard let self, elapsed < 0.8 else {
                    self?.render(.still)
                    return
                }
                self.render(.at(elapsed))
                try? await Task.sleep(for: .milliseconds(125), tolerance: .milliseconds(20))
            }
        }
    }

    /// One entry for the total, or one per source when the counter is split.
    private func menuBarCounts(_ layout: InboxLayout) -> [(pluginID: String?, count: Int)] {
        let preferences = model.preferences
        guard preferences.menuBarCount != .hidden else { return [] }
        let counts = layout.countsBySource(actionableOnly: preferences.menuBarCount == .waitingOnMe)
        if preferences.countPerSource {
            return counts.map { ($0.pluginID, $0.count) }
        }
        let total = counts.reduce(0) { $0 + $1.count }
        return total > 0 ? [(nil, total)] : []
    }

    @objc private func pressed() {
        let event = NSApp.currentEvent?.type
        if event == .rightMouseDown {
            showMenu()
            return
        }
        // VoiceOver and Switch Control press the button without a mouse-down: the drag loop would wait for a mouse-up
        // that never comes. Only a real press can become a drag.
        guard Self.canStartDrag(event) else {
            togglePopover()
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
        // One task running: finish or stop it from here, without opening the inbox.
        let running = model.layout.inProgress
        if running.count == 1, let task = running.first {
            // A long Slack message would make the whole menu as wide: the menu shows its start.
            let header = NSMenuItem(title: Self.menuTitle(task.title), action: nil, keyEquivalent: "")
            header.toolTip = task.title
            header.isEnabled = false
            menu.addItem(header)
            menu.addItem(item(L("Done"), symbol: "checkmark", key: "") { [weak self] in self?.model.toggleDone(task) })
            menu.addItem(item(L("Stop"), symbol: "stop", key: "") { [weak self] in self?.model.stop(task) })
            menu.addItem(.separator())
        }
        menu.addItem(item(L("Open Remora"), symbol: "tray", key: "") { [weak self] in self?.togglePopover() })
        menu.addItem(item(L("New reminder…"), symbol: "alarm", key: "") { [weak self] in self?.newReminder() })
        menu.addItem(
            item(L("Refresh now"), symbol: "arrow.clockwise", key: "r") { [weak self] in
                Task { await self?.model.refresh(manual: true) }
            })
        menu.addItem(.separator())
        if let offer = model.updater.offer {
            menu.addItem(
                item(L("Update to Remora %@…", offer.version.description), symbol: "arrow.down.circle", key: "") {
                    SettingsOpener.open()
                })
        }
        menu.addItem(item(L("Settings…"), symbol: "gearshape", key: ",") { SettingsOpener.open() })
        menu.addItem(.separator())
        menu.addItem(item(L("Quit Remora"), symbol: "power", key: "q") { NSApp.terminate(nil) })
        menu.popUp(positioning: nil, at: NSPoint(x: 0, y: button.bounds.height + 4), in: button)
    }

    /// Only a left mouse-down can be the start of a drag to set a reminder.
    static func canStartDrag(_ event: NSEvent.EventType?) -> Bool {
        event == .leftMouseDown
    }

    /// At most `limit` characters, cut at a word when one is close, with an ellipsis; on one line.
    static func menuTitle(_ title: String, limit: Int = 60) -> String {
        let line = title.split(whereSeparator: \.isNewline).joined(separator: " ")
        guard line.count > limit else { return line }
        let cut = line.prefix(limit)
        let word = cut.lastIndex(of: " ").map { cut[..<$0] }.flatMap { $0.count >= limit - 15 ? $0 : nil } ?? cut
        return word.trimmingCharacters(in: .whitespacesAndNewlines.union(.punctuationCharacters)) + "…"
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
            model.popoverOpenings += 1
            // Reduce Motion: the popover appears without its zoom.
            popover.animates = !NSWorkspace.shared.accessibilityDisplayShouldReduceMotion
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

/// Builds "3" or "[github] 3  [slack] 4" for the status item, symbols drawn as template images,
/// led by "1 in progress (25 min)" while the user works on something.
enum MenuBarTitle {
    struct Content {
        var counts: [(pluginID: String?, count: Int)] = []
        var running = 0
        /// When the oldest task in progress started.
        var since: Date?
        var branded = true
        /// Only the task in progress shows, in inverted colors.
        var focused = false
    }

    /// One animation frame: how far the fish turns. Quantized, so a few cached frames cover the whole wiggle.
    struct Motion: Hashable {
        var swim: CGFloat = 0

        static let still = Motion()

        /// `time` in the 10-second cycle (or since a wiggle started): the fish wiggles during the first 0.8 s, damped.
        static func at(_ time: TimeInterval) -> Motion {
            let local = time.truncatingRemainder(dividingBy: 10)
            let swim = local < 0.8 ? 0.14 * sin(2 * .pi * 3 * local / 0.8) * (1 - local / 0.8) : 0
            return Motion(swim: (swim * 50).rounded() / 50)
        }
    }

    /// "1 in progress (25 min)".
    /// What VoiceOver says for the menu bar item: what the label shows, in words.
    static func spokenText(_ content: Content, now: Date = .now) -> String {
        let total = content.counts.reduce(0) { $0 + $1.count }
        var parts = ["Remora"]
        if content.running > 0 { parts.append(runningText(content, now: now)) }
        if !content.focused {
            parts.append(total > 0 ? L("%d needs you", plural: "%d need you", total) : L("Nothing needs you"))
        }
        return parts.joined(separator: ", ")
    }

    static func runningText(_ content: Content, now: Date = .now) -> String {
        let text = L("%d in progress", content.running)
        guard let since = content.since else { return text }
        let minutes = max(0, Int(now.timeIntervalSince(since) / 60))
        let elapsed =
            minutes < 60
            ? L("%d min", minutes)
            : L("%d h", minutes / 60) + (minutes % 60 == 0 ? "" : String(format: " %02d", minutes % 60))
        return "\(text) (\(elapsed))"
    }

    /// Myna branding: an ink fish and counters on a lime capsule, shown when something needs you.
    /// Not a template image, so it keeps its colors on light and dark menu bars.
    /// Focused on a task in progress: lime on ink, so it reads differently from the usual count.
    static func pill(_ content: Content, motion: Motion = .still, height: CGFloat = 18) -> NSImage {
        let lime = NSColor(hex: 0xB9FF66)
        let dark = NSColor(hex: 0x111111)
        let (ink, fill) = content.focused ? (lime, dark) : (dark, lime)
        let title = make(content, motion: motion, color: ink)
        let fish = NSSize(width: 22, height: 11)
        let text = title.size()
        let size = NSSize(width: 7 + fish.width + text.width + 8, height: height)
        let image = NSImage(size: size, flipped: false) { rect in
            guard let context = NSGraphicsContext.current?.cgContext else { return false }
            fill.setFill()
            NSBezierPath(roundedRect: rect, xRadius: height / 2, yRadius: height / 2).fill()
            let fishRect = CGRect(x: 7, y: (height - fish.height) / 2, width: fish.width, height: fish.height)
            context.saveGState()
            context.translateBy(x: fishRect.midX, y: fishRect.midY)
            context.rotate(by: motion.swim)
            context.translateBy(x: -fishRect.midX, y: -fishRect.midY)
            RemoraArt.drawCropped(in: fishRect, color: ink, context: context)
            context.restoreGState()
            title.draw(at: NSPoint(x: 7 + fish.width, y: (height - text.height) / 2))
            return true
        }
        image.isTemplate = false
        return image
    }

    /// "3" or "[github] 3  [slack] 4"; adapts to the menu bar unless a color is given.
    static func make(_ content: Content, motion: Motion = .still, color: NSColor? = nil) -> NSAttributedString {
        let counts = content.counts
        let running = content.running
        let font = NSFont.monospacedDigitSystemFont(ofSize: 13, weight: .medium)
        var attributes: [NSAttributedString.Key: Any] = [.font: font]
        if let color { attributes[.foregroundColor] = color }
        let title = NSMutableAttributedString()
        if running > 0 {
            title.append(NSAttributedString(string: " ", attributes: attributes))
            // The words say it on their own; next to other counters, ▶ tells this number apart.
            if !counts.isEmpty, let image = symbol("play.fill", pointSize: 9, color: color) {
                let attachment = NSTextAttachment()
                attachment.image = image
                // Centered on the digits' height, like the fish next to it.
                attachment.bounds = CGRect(
                    x: 0, y: ((font.capHeight - image.size.height) / 2).rounded(), width: image.size.width,
                    height: image.size.height)
                title.append(NSAttributedString(attachment: attachment))
                title.append(NSAttributedString(string: "\u{2009}", attributes: attributes))
            }
            title.append(
                NSAttributedString(string: counts.isEmpty ? runningText(content) : "\(running)", attributes: attributes)
            )
        }
        for (index, entry) in counts.enumerated() {
            title.append(NSAttributedString(string: index == 0 && running == 0 ? " " : "  ", attributes: attributes))
            if let pluginID = entry.pluginID,
                let image = ToolIcon.image(pluginID: pluginID, pointSize: 12, color: color)
            {
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

    private static func symbol(_ name: String, pointSize: CGFloat, color: NSColor?) -> NSImage? {
        let configuration = NSImage.SymbolConfiguration(pointSize: pointSize, weight: .semibold)
        guard
            let image = NSImage(systemSymbolName: name, accessibilityDescription: nil)?.withSymbolConfiguration(
                configuration)
        else { return nil }
        guard let color else {
            image.isTemplate = true
            return image
        }
        return image.withSymbolConfiguration(NSImage.SymbolConfiguration(paletteColors: [color])) ?? image
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

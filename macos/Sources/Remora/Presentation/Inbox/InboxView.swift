import RemoraCore
import RemoraPlugins
import SwiftUI

/// The inbox: the header, tabs and search here, with the overlays. The tab's list is in `InboxContent.swift`, the
/// keys in `InboxKeyboard.swift`, the footer in `InboxFooter.swift`, the tab row in `InboxTabs.swift` and a
/// bundle's header and summary in `BundleViews.swift`.
struct InboxView: View {
    enum Tab: String, CaseIterable {
        case inProgress = "In progress"
        case myTurn = "My turn"
        case waiting = "Waiting"
        case snoozed = "Snoozed"
        case done = "Done"

        var title: String { L(self == .done ? "Done items" : rawValue) }
    }

    /// Roomy on purpose: more air between items makes a busy inbox feel calmer.
    static let size = CGSize(width: 460, height: 700)
    /// Side margin shared by the header, tabs, search and list.
    static let gutter: CGFloat = 18

    @Environment(InboxModel.self) var model
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State var tab = Self.launchTab
    /// The tab a link asked for, by its id (`myTurn`, `waiting`…).
    private var requested: Tab? { model.requestedTab.flatMap { id in Tab.allCases.first { "\($0)" == id } } }

    /// `--demo --tab snoozed` opens on that tab, for screenshots.
    private static var launchTab: Tab {
        let arguments = CommandLine.arguments
        guard arguments.contains("--demo"), let index = arguments.firstIndex(of: "--tab"), index + 1 < arguments.count
        else { return .myTurn }
        return Tab.allCases.first { "\($0)".lowercased() == arguments[index + 1].lowercased() } ?? .myTurn
    }
    @State var picker: TimePickerSubject?
    @State var draft: DraftSubject?
    @State var reviewing = false
    @State var collapsed: Set<String> = []
    @State var showBrief = false
    @State var showingSummary: Set<String> = []
    @State var expanded: Set<String> = []
    /// The item the keyboard is on (↑↓ or J K), and where typing goes: the list or the search field.
    @State var selection: String?
    @FocusState var focus: Focus?
    @State var showShortcuts = false

    enum Focus: Hashable { case list, search }

    /// Long bundles show their most important items first and fold the rest.
    let visibleInBundle = 5

    var body: some View {
        let layout = model.visibleLayout
        VStack(spacing: 0) {
            header
            HStack(spacing: 6) {
                TabSwitch(
                    tab: $tab,
                    tabs: layout.inProgress.isEmpty ? [.myTurn, .waiting] : [.inProgress, .myTurn, .waiting],
                    counts: [
                        .inProgress: layout.inProgress.count, .myTurn: layout.actionCount,
                        .waiting: layout.waitingItems.count,
                    ]
                )
                ArchiveButton(
                    tab: .snoozed, symbol: "moon.zzz", count: layout.snoozed.count, compact: !layout.inProgress.isEmpty,
                    selection: $tab)
                ArchiveButton(
                    tab: .done, symbol: "checkmark.circle", count: nil, compact: !layout.inProgress.isEmpty,
                    selection: $tab)
            }
            .padding(.horizontal, Self.gutter)
            SearchField(text: Bindable(model).query, focus: $focus) {
                focus = .list
                if selection == nil { selection = navigationOrder(model.visibleLayout).first?.id }
            }
            .padding(.horizontal, Self.gutter)
            .padding(.vertical, 10)
            content(layout)
            if let undo = model.undoPoint { UndoBar(point: undo) }
            footer
        }
        .frame(width: Self.size.width, height: Self.size.height)
        .background(Myna.backdrop)
        .background {
            // ⌘F and ⌘R work wherever the focus is, the search field included.
            Group {
                Button("") { focus = .search }.keyboardShortcut("f", modifiers: .command)
                Button("") { Task { await model.refresh(manual: true) } }.keyboardShortcut("r", modifiers: .command)
                Button("") { withAnimation(.snappy) { model.undo() } }.keyboardShortcut("z", modifiers: .command)
                    .disabled(model.undoPoint == nil)
                // Escape closes the overlay on top, wherever the focus is.
                if isOverlaid { Button("") { closeOverlay() }.keyboardShortcut(.cancelAction) }
            }
            .frame(width: 0, height: 0)
            .opacity(0)
            .accessibilityHidden(true)
        }
        // An overlay is modal: VoiceOver doesn't reach the inbox behind it, and the dimmed backdrop, a pointer
        // shortcut, isn't read either.
        .accessibilityHidden(isOverlaid)
        .overlay {
            // The time picker goes over the review session, which stays as it is: closing the picker returns to the
            // same review, with its skipped items and its time (UX-23).
            ZStack {
                if reviewing {
                    Color.black.opacity(0.25)
                        .ignoresSafeArea()
                        .onTapGesture { reviewing = false }
                        .accessibilityHidden(true)
                    ReviewSession(snooze: { picker = .snooze($0) }, covered: picker != nil) { reviewing = false }
                        .accessibilityAddTraits(.isModal)
                        .accessibilityHidden(picker != nil)
                        .transition(.scale(scale: 0.95).combined(with: .opacity))
                } else if let draft, picker == nil {
                    Color.black.opacity(0.25)
                        .ignoresSafeArea()
                        .onTapGesture { self.draft = nil }
                        .accessibilityHidden(true)
                    DraftPanel(subject: draft, link: model.webLink(for: draft.item)) { self.draft = nil }
                        .accessibilityAddTraits(.isModal)
                        .transition(.scale(scale: 0.95).combined(with: .opacity))
                }
                if let picker {
                    Color.black.opacity(0.25)
                        .ignoresSafeArea()
                        .onTapGesture { self.picker = nil }
                        .accessibilityHidden(true)
                    TimePicker(subject: picker) { self.picker = nil }
                        .accessibilityAddTraits(.isModal)
                        .transition(.scale(scale: 0.95).combined(with: .opacity))
                }
            }
        }
        .animation(.snappy(duration: 0.2), value: picker?.id)
        // Reduce Motion: changes happen at once, without slides, springs or spins, everywhere in the inbox.
        .transaction { if reduceMotion { $0.animation = nil } }
        .preferredColorScheme(model.preferences.appearance.colorScheme)
        // Drawn again with the new size when it changes.
        .id(model.preferences.textSize)
        .onChange(of: model.popoverOpenings) {
            // Each opening starts fresh: no search left from last time.
            model.query = ""
            tab = requested ?? (layout.inProgress.isEmpty ? .myTurn : .inProgress)
            model.requestedTab = nil
            selection = nil
            focus = .list
        }
        // A `remora://open?tab=…` link while the inbox is already open (IDEA-06).
        .onChange(of: model.requestedTab) { _, asked in
            guard asked != nil, let requested else { return }
            tab = requested
            model.requestedTab = nil
        }
        .onChange(of: tab) { selection = nil }
        .onChange(of: layout.inProgress.isEmpty) { _, empty in
            if empty, tab == .inProgress { withAnimation(.snappy) { tab = .myTurn } }
        }
        .onAppear {
            focus = .list
            if !layout.inProgress.isEmpty, Self.launchTab == .myTurn { tab = .inProgress }
            model.refreshIfStale()
            if model.assistant.brief != nil, model.preferences.wholeInboxBrief { showBrief = true }
        }
    }

    private var header: some View {
        HStack(spacing: 8) {
            RemoraMark()
            Text("Remora").font(Myna.font(18, .bold)).foregroundStyle(Myna.ink)
            Spacer()
            if model.preferences.wholeInboxBrief, model.assistantAccount != nil || model.assistant.brief != nil {
                ActionButton(label: "Brief", symbol: "sparkles", prominent: showBrief) {
                    withAnimation(.snappy) { showBrief.toggle() }
                    if showBrief { tab = .myTurn }
                }
            }
            // Spins while refreshing; with Reduce Motion it dims instead.
            IconButton(symbol: "arrow.clockwise", help: "Refresh") { Task { await model.refresh(manual: true) } }
                .modifier(Spin(active: model.sync.isRefreshing && !reduceMotion))
                .opacity(model.sync.isRefreshing && reduceMotion ? 0.4 : 1)
            IconButton(symbol: "gearshape", help: "Settings", action: showSettings)
        }
        .padding(.horizontal, Self.gutter)
        .padding(.top, 14)
        .padding(.bottom, 10)
    }

    var isOverlaid: Bool { picker != nil || reviewing || draft != nil }

    /// The overlay on top: the time picker opens over a review session, so it closes first.
    private func closeOverlay() {
        if picker != nil { picker = nil } else if reviewing { reviewing = false } else { draft = nil }
    }

    func showSettings() {
        SettingsOpener.open()
    }
}

/// One turn a second while `active`, drawn from the clock: it stops as soon as `active` does. A `repeatForever`
/// animation kept spinning after the refresh ended.
private struct Spin: ViewModifier {
    let active: Bool

    func body(content: Content) -> some View {
        TimelineView(.animation(paused: !active)) { context in
            let turn = context.date.timeIntervalSinceReferenceDate.truncatingRemainder(dividingBy: 1)
            content.rotationEffect(.degrees(active ? turn * 360 : 0))
        }
    }
}

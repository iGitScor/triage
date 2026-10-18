import RemoraCore
import SwiftUI

/// The tabs that matter: what you're on (while something runs), your turn and theirs.
struct TabSwitch: View {
    @Binding var tab: InboxView.Tab
    let tabs: [InboxView.Tab]
    let counts: [InboxView.Tab: Int]

    var body: some View {
        HStack(spacing: 4) {
            ForEach(tabs, id: \.self) { value in
                let selected = tab == value
                Button {
                    withAnimation(.snappy(duration: 0.2)) { tab = value }
                } label: {
                    HStack(spacing: 6) {
                        if value == .inProgress {
                            Image(systemName: "play.fill").font(.system(size: 11, weight: .bold))
                        } else {
                            Text(value.title).font(RemoraDesign.font(13, .semibold)).lineLimit(1).fixedSize()
                        }
                        if let count = counts[value], count > 0 {
                            Text("\(count)")
                                .font(RemoraDesign.font(11, .bold))
                                .foregroundStyle(selected ? RemoraDesign.onAccent : RemoraDesign.muted)
                                .padding(.horizontal, 6)
                                .padding(.vertical, 1)
                                .background(selected ? RemoraDesign.accent : RemoraDesign.line, in: Capsule())
                        }
                    }
                    .foregroundStyle(selected ? RemoraDesign.onDark : RemoraDesign.ink)
                    .frame(maxWidth: value == .inProgress ? nil : .infinity)
                    .padding(.horizontal, value == .inProgress ? 10 : 0)
                    .padding(.vertical, 7)
                    .background(selected ? RemoraDesign.dark : .clear, in: Capsule())
                    .contentShape(Capsule())
                }
                .buttonStyle(.plain)
                .help(value.title)
                // The In progress tab is an icon: say its name, its count, and which tab is selected.
                .accessibilityLabel(value.title)
                .accessibilityValue(counts[value].map { "\($0)" } ?? "")
                .accessibilityAddTraits(selected ? .isSelected : [])
            }
        }
        .padding(4)
        .background(RemoraDesign.card, in: Capsule())
    }
}

/// Snoozed and Done: archives you open now and then, so smaller than the main tabs.
struct ArchiveButton: View {
    let tab: InboxView.Tab
    let symbol: String
    let count: Int?
    /// Icon (and count) only, when the tab row is crowded.
    var compact = false
    @Binding var selection: InboxView.Tab

    var body: some View {
        let selected = selection == tab
        Button {
            withAnimation(.snappy(duration: 0.2)) { selection = selected ? .myTurn : tab }
        } label: {
            HStack(spacing: 4) {
                Image(systemName: symbol).font(.system(size: 11, weight: .semibold))
                if compact {
                    if let count, count > 0 { Text("\(count)").font(RemoraDesign.font(11.5, .semibold)) }
                } else {
                    Text(count.map { $0 > 0 ? "\(tab.title) \($0)" : tab.title } ?? tab.title).font(
                        RemoraDesign.font(11.5, .semibold)
                    ).lineLimit(1)
                }
            }
            .foregroundStyle(selected ? RemoraDesign.onDark : RemoraDesign.muted)
            .padding(.horizontal, 9)
            .padding(.vertical, 8)
            .background(selected ? RemoraDesign.dark : RemoraDesign.card, in: Capsule())
            .contentShape(Capsule())
        }
        .buttonStyle(.plain)
        .help(tab.title)
        // Compact, it's an icon and a number: say what it is.
        .accessibilityLabel(tab.title)
        .accessibilityValue(count.map { "\($0)" } ?? "")
        // Set both ways: with only the addition, Done reported itself selected while it wasn't.
        .accessibilityAddTraits(selected ? .isSelected : [])
        .accessibilityRemoveTraits(selected ? [] : .isSelected)
    }
}

struct SearchField: View {
    @Binding var text: String
    var focus: FocusState<InboxView.Focus?>.Binding
    /// Esc (clearing the search) or ↓ hands the keyboard back to the list.
    let toList: () -> Void

    var body: some View {
        HStack(spacing: 6) {
            Image(systemName: "magnifyingglass").foregroundStyle(RemoraDesign.muted)
            TextField("Search", text: $text)
                .textFieldStyle(.plain)
                .focused(focus, equals: .search)
                .onKeyPress(.escape) {
                    text = ""
                    toList()
                    return .handled
                }
                .onKeyPress(.downArrow) {
                    toList()
                    return .handled
                }
            if !text.isEmpty {
                Button {
                    text = ""
                } label: {
                    Image(systemName: "xmark.circle.fill")
                }
                .buttonStyle(.plain)
                .foregroundStyle(RemoraDesign.muted)
                .help("Clear search")
                .accessibilityLabel(L("Clear search"))
            }
        }
        .font(RemoraDesign.font(13))
        .padding(.horizontal, 12)
        .padding(.vertical, 7)
        .background(RemoraDesign.field, in: Capsule())
        .overlay(Capsule().strokeBorder(RemoraDesign.border))
    }
}

import RemoraCore
import SwiftUI

/// What the picker was opened for: snoozing one item, several, or creating a reminder.
enum TimePickerSubject: Identifiable {
    case snooze(InboxItem)
    case snoozeMany([InboxItem])
    case newReminder

    var id: String {
        switch self {
        case .snooze(let item): item.id
        case .snoozeMany(let items): "many/" + items.map(\.id).joined(separator: ",")
        case .newReminder: "new-reminder"
        }
    }
}

struct TimePicker: View {
    @Environment(InboxModel.self) private var model
    let subject: TimePickerSubject
    let dismiss: () -> Void

    @State private var date = Date.now.addingTimeInterval(3_600)
    @State private var mode = Snooze.Mode.hide
    @State private var text = ""
    @State private var reason: SnoozeReason?
    @State private var untilNews = true
    @FocusState private var focused: Bool

    private let clock = SnoozeClock()

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                HStack {
                    Text(title).font(Myna.font(15, .semibold)).foregroundStyle(Myna.ink)
                    Spacer()
                    IconButton(symbol: "xmark", help: "Close", size: 24, action: dismiss)
                }

                if let subtitle {
                    Text(subtitle).font(Myna.font(12)).foregroundStyle(Myna.muted).lineLimit(1)
                }

                if !isReminder {
                    ReasonPicker(reason: $reason) { picked in
                        if let picked { date = model.suggestedReturn(for: picked) }
                    }
                    if reason == .motivation, let nudge {
                        Label(nudge, systemImage: "lightbulb")
                            .font(Myna.font(12, .medium))
                            .foregroundStyle(Myna.accentText)
                            .padding(10)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .background(Myna.accentSoft, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
                    }
                    if reason == .waiting {
                        Toggle(L("Until there’s news"), isOn: $untilNews)
                            .font(Myna.font(12.5))
                            .toggleStyle(.switch)
                            .controlSize(.small)
                            .tint(Myna.accentDeep)
                    }
                }

                if case .snooze = subject {
                    ModeSwitch(mode: $mode)
                }
                if !isMany {
                    TextField(L(isReminder ? "Remind me to…" : "Add a note (optional)"), text: $text)
                        .textFieldStyle(.plain)
                        .font(Myna.font(13))
                        .padding(10)
                        .background(Myna.field, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
                        .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).strokeBorder(Myna.border))
                        .focused($focused)
                        .onSubmit(confirm)
                }

                LazyVGrid(columns: [GridItem(.flexible()), GridItem(.flexible())], spacing: 8) {
                    ForEach(presets) { preset in
                        PresetButton(preset: preset, selected: abs(preset.date.timeIntervalSince(date)) < 60) {
                            date = preset.date
                        }
                    }
                }

                Scrubber(date: $date, clock: clock)

                Button(action: confirm) {
                    Text(confirmLabel).frame(maxWidth: .infinity)
                }
                .buttonStyle(PillButtonStyle())
                .disabled(isReminder && text.trimmingCharacters(in: .whitespaces).isEmpty)
                .keyboardShortcut(.defaultAction)
            }
            .padding(16)
        }
        .scrollIndicators(.never)
        .frame(maxHeight: 548)
        .fixedSize(horizontal: false, vertical: true)
        .background(Myna.surface, in: RoundedRectangle(cornerRadius: Myna.radiusLarge, style: .continuous))
        .shadow(color: .black.opacity(0.2), radius: 20, y: 6)
        .padding(16)
        .onAppear {
            focused = isReminder
            if case .snooze(let item) = subject, let usual = model.usualReason(for: item) {
                reason = usual
                date = model.suggestedReturn(for: usual)
            }
        }
    }

    private var isReminder: Bool {
        if case .newReminder = subject { return true }
        return false
    }

    private var isMany: Bool {
        if case .snoozeMany = subject { return true }
        return false
    }

    private var title: String {
        switch subject {
        case .snooze: L("Snooze until…")
        case .snoozeMany(let items): L("Snooze %d item until…", plural: "Snooze %d items until…", items.count)
        case .newReminder: L("New reminder")
        }
    }

    private var subtitle: String? {
        switch subject {
        case .snooze(let item): item.title
        case .snoozeMany(let items): items.prefix(3).map(\.title).joined(separator: " · ")
        case .newReminder: nil
        }
    }

    private var confirmLabel: String {
        if isReminder { return L("Add reminder") }
        return L(mode == .hide ? "Snooze" : "Remind me")
    }

    private var nudge: String? {
        switch subject {
        case .snooze(let item): model.advisor.nudge(for: item)
        case .snoozeMany(let items): items.first.map(model.advisor.nudge(for:))
        case .newReminder: nil
        }
    }

    /// Your usual time for this kind of item first, when there is one.
    private var presets: [SnoozeClock.Preset] {
        var presets = clock.presets()
        if case .snooze(let item) = subject, let usual = model.usualReturn(for: item) {
            presets.insert(SnoozeClock.Preset(label: L("Usual"), symbol: "clock.arrow.circlepath", date: usual), at: 0)
        }
        return presets
    }

    private func confirm() {
        let untilNews = reason == .waiting && self.untilNews
        switch subject {
        case .snooze(let item):
            model.snooze(item, until: date, mode: mode, note: text, reason: reason, untilNews: untilNews)
        case .snoozeMany(let items):
            model.snoozeMany(items, until: date, reason: reason, untilNews: untilNews)
        case .newReminder:
            let title = text.trimmingCharacters(in: .whitespaces)
            guard !title.isEmpty else { return }
            model.addReminder(title, at: date)
        }
        dismiss()
    }
}

/// "Why?": optional, one tap. Each reason moves the time to its suggestion.
private struct ReasonPicker: View {
    @Binding var reason: SnoozeReason?
    let changed: (SnoozeReason?) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(L("Why? (optional)")).font(Myna.font(11.5, .semibold)).foregroundStyle(Myna.muted)
            LazyVGrid(columns: [GridItem(.adaptive(minimum: 110), spacing: 6)], alignment: .leading, spacing: 6) {
                ForEach(SnoozeReason.allCases, id: \.self) { value in
                    let selected = reason == value
                    Button {
                        reason = selected ? nil : value
                        changed(reason)
                    } label: {
                        Label(value.title, systemImage: value.symbol)
                            .font(Myna.font(11.5, .medium))
                            .lineLimit(1)
                            .foregroundStyle(selected ? Myna.onDark : Myna.ink)
                            .padding(.horizontal, 9)
                            .padding(.vertical, 6)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .background(selected ? Myna.dark : Myna.card, in: Capsule())
                            .contentShape(Capsule())
                    }
                    .buttonStyle(.plain)
                }
            }
        }
    }
}

private struct ModeSwitch: View {
    @Binding var mode: Snooze.Mode

    var body: some View {
        HStack(spacing: 4) {
            segment(.hide, "Hide until then", "moon.zzz")
            segment(.remind, "Keep & remind me", "bell")
        }
        .padding(3)
        .background(Myna.card, in: Capsule())
    }

    private func segment(_ value: Snooze.Mode, _ label: String, _ symbol: String) -> some View {
        Button {
            mode = value
        } label: {
            Label(L(label), systemImage: symbol)
                .font(Myna.font(12, .medium))
                .foregroundStyle(mode == value ? Myna.onDark : Myna.ink)
                .frame(maxWidth: .infinity)
                .padding(.vertical, 6)
                .background(mode == value ? Myna.dark : .clear, in: Capsule())
                .contentShape(Capsule())
        }
        .buttonStyle(.plain)
    }
}

private struct PresetButton: View {
    let preset: SnoozeClock.Preset
    let selected: Bool
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: 8) {
                Image(systemName: preset.symbol).font(.system(size: 13, weight: .semibold))
                VStack(alignment: .leading, spacing: 1) {
                    Text(preset.label).font(Myna.font(12.5, .semibold))
                    Text(preset.date.formatted(.dateTime.weekday(.abbreviated).hour().minute()))
                        .font(Myna.font(10.5))
                        .opacity(0.7)
                }
                Spacer(minLength: 0)
            }
            .foregroundStyle(selected ? Myna.onAccent : Myna.ink)
            .padding(10)
            .background(selected ? Myna.accent : Myna.card, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }
}

/// Gestimer-style: drag along the track, time accelerates from minutes to days.
private struct Scrubber: View {
    @Binding var date: Date
    let clock: SnoozeClock

    /// Always derived from the date, so presets and suggestions move the knob too.
    private var progress: Double { clock.progress(for: date.timeIntervalSinceNow) }
    @FocusState private var focused: Bool

    /// One of 18 steps along the track, as on Windows: from the keyboard and VoiceOver.
    private func step(_ steps: Int) {
        let position = min(max(progress + Double(steps) / 18, 0), 1)
        date = clock.roundedUp(Date.now.addingTimeInterval(clock.duration(at: position)))
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(clock.describe(date))
                .font(Myna.font(13, .semibold))
                .foregroundStyle(Myna.accentText)
                .contentTransition(.numericText())
            GeometryReader { geometry in
                let knob: CGFloat = 22
                let x = (geometry.size.width - knob) * progress
                ZStack(alignment: .leading) {
                    Capsule().fill(Myna.card).frame(height: 8)
                    Capsule().fill(Myna.accent).frame(width: x + knob / 2, height: 8)
                    Circle()
                        .fill(Myna.dark)
                        .overlay(Image(systemName: "clock.fill").font(.system(size: 10)).foregroundStyle(Myna.accent))
                        .frame(width: knob, height: knob)
                        .offset(x: x)
                }
                .frame(maxHeight: .infinity)
                .contentShape(Rectangle())
                .gesture(DragGesture(minimumDistance: 0).onChanged { value in
                    let position = min(max((value.location.x - knob / 2) / (geometry.size.width - knob), 0), 1)
                    date = Date.now.addingTimeInterval(clock.duration(at: position))
                })
            }
            .frame(height: 24)
            // Not only a drag: ← → from the keyboard, swipe up or down with VoiceOver, which reads the time.
            .focusable()
            .focused($focused)
            .overlay(RoundedRectangle(cornerRadius: 12).strokeBorder(Myna.accent, lineWidth: focused ? 2 : 0).padding(-3))
            .onKeyPress(.rightArrow) { step(1); return .handled }
            .onKeyPress(.leftArrow) { step(-1); return .handled }
            .accessibilityElement()
            .accessibilityLabel(L("When"))
            .accessibilityValue(clock.describe(date))
            .accessibilityAdjustableAction { direction in
                switch direction {
                case .increment: step(1)
                case .decrement: step(-1)
                @unknown default: break
                }
            }
            HStack {
                Text("5 min")
                Spacer()
                Text("1 week")
            }
            .font(Myna.font(10))
            .foregroundStyle(Myna.muted)
            // An exact date and time, by keyboard too, and beyond a week.
            DatePicker(L("Exact date"), selection: $date, in: Date.now..., displayedComponents: [.date, .hourAndMinute])
                .datePickerStyle(.compact)
                .font(Myna.font(11.5))
                .foregroundStyle(Myna.muted)
        }
    }
}

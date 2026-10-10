import RemoraCore
import SwiftUI

struct Avatar: View {
    @Environment(InboxModel.self) private var model
    let person: Person?
    var size: CGFloat = 28

    var body: some View {
        // Remote images only from allowed tools' hosts; initials otherwise.
        AsyncImage(url: model.allowsImage(person?.avatarURL) ? person?.avatarURL : nil) { image in
            image.resizable().scaledToFill()
        } placeholder: {
            Text(person?.initials ?? "")
                .font(Myna.font(size * 0.38, .semibold))
                .foregroundStyle(Myna.inkSoft)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .background(Myna.card2)
        }
        .frame(width: size, height: size)
        .clipShape(Circle())
        .help(person?.name ?? "")
    }
}

/// Overlapping reviewer avatars, each ringed with its review tone and marked ✓ (approved) or ✕ (changes requested),
/// so the state doesn't rest on colour alone; VoiceOver reads each reviewer with their state.
struct PeopleStack: View {
    let people: [Person]
    var limit = 4

    var body: some View {
        HStack(spacing: -6) {
            ForEach(people.prefix(limit)) { person in
                Avatar(person: person, size: 18)
                    .overlay(Circle().strokeBorder(ring(for: person), lineWidth: 1.5))
                    .overlay(alignment: .bottomTrailing) { mark(for: person) }
                    .help(Self.describe(person))
            }
            if people.count > limit {
                Text("+\(people.count - limit)")
                    .font(Myna.font(9, .semibold))
                    .foregroundStyle(Myna.muted)
                    .padding(.leading, 8)
            }
        }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(L("Reviewers: %@", people.map(Self.describe).formatted(.list(type: .and))))
    }

    /// "Erin, approved", "Dave, changes requested", "Frank, waiting".
    static func describe(_ person: Person) -> String {
        switch person.tone {
        case .accent?, .positive?: L("%@, approved", person.name)
        case .negative?: L("%@, changes requested", person.name)
        default: L("%@, waiting", person.name)
        }
    }

    @ViewBuilder private func mark(for person: Person) -> some View {
        let symbol: String? = switch person.tone {
        case .accent?, .positive?: "checkmark"
        case .negative?: "xmark"
        default: nil
        }
        if let symbol {
            Image(systemName: symbol)
                .font(.system(size: 5.5, weight: .black))
                .foregroundStyle(person.tone == .negative ? Myna.onDark : Myna.onAccent)
                .frame(width: 9, height: 9)
                .background(ring(for: person), in: Circle())
                .overlay(Circle().strokeBorder(Myna.card, lineWidth: 1))
                .offset(x: 2, y: 2)
        }
    }

    private func ring(for person: Person) -> Color {
        guard let tone = person.tone else { return Myna.card }
        return tone == .accent ? Myna.accentDeep : Myna.color(for: tone).foreground
    }
}

struct BadgeChip: View {
    let badge: Badge

    var body: some View {
        let colors = Myna.color(for: badge.tone)
        HStack(spacing: 3) {
            if let symbol = badge.symbol {
                Image(systemName: symbol).font(.system(size: 8.5, weight: .bold))
            }
            Text(badge.label).font(Myna.font(10.5, .semibold)).lineLimit(1)
        }
        .fixedSize()
        .foregroundStyle(colors.foreground)
        .padding(.horizontal, 7)
        .padding(.vertical, 2.5)
        .background(colors.background, in: Capsule())
    }
}

struct IconButton: View {
    let symbol: String
    let help: String
    var size: CGFloat = 28
    var prominent = false
    let action: () -> Void

    @State private var hovering = false

    var body: some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: size * 0.45, weight: .semibold))
                .foregroundStyle(prominent ? Myna.onAccent : Myna.ink)
                .frame(width: size, height: size)
                .background(background, in: Circle())
                .contentShape(Circle())
        }
        .buttonStyle(.plain)
        // An icon alone: the tooltip is also what VoiceOver says (it read "gearshape" for Settings).
        .help(L(help))
        .accessibilityLabel(L(help))
        .onHover { hovering = $0 }
    }

    private var background: Color {
        if prominent { return hovering ? Myna.accentDeep : Myna.accent }
        return hovering ? Myna.card2 : .clear
    }
}

/// A small labeled action: icon plus a word, so it reads without a tooltip.
struct ActionButton: View {
    let label: String
    let symbol: String
    var prominent = false
    let action: () -> Void

    @State private var hovering = false

    var body: some View {
        Button(action: action) {
            Label(L(label), systemImage: symbol)
                .lineLimit(1)
                .fixedSize()
                .font(Myna.font(11.5, .semibold))
                .foregroundStyle(prominent ? Myna.onAccent : Myna.ink)
                .padding(.horizontal, 9)
                .padding(.vertical, 5)
                .background(background, in: Capsule())
                .contentShape(Capsule())
        }
        .buttonStyle(.plain)
        .onHover { hovering = $0 }
    }

    private var background: Color {
        if prominent { return hovering ? Myna.accentDeep : Myna.accent }
        return hovering ? Myna.card2 : Myna.card
    }
}

struct PillButtonStyle: ButtonStyle {
    var prominent = true

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(Myna.font(13, .semibold))
            .foregroundStyle(prominent ? Myna.onAccent : Myna.ink)
            .padding(.horizontal, 16)
            .padding(.vertical, 8)
            .background(prominent ? Myna.accent : Myna.card, in: Capsule())
            .opacity(configuration.isPressed ? 0.8 : 1)
            .scaleEffect(configuration.isPressed ? 0.97 : 1)
    }
}

struct SoonBadge: View {
    var body: some View {
        Text("Soon")
            .font(Myna.font(10, .bold))
            .foregroundStyle(Myna.onAccent)
            .padding(.horizontal, 6)
            .padding(.vertical, 1.5)
            .background(Myna.accent, in: Capsule())
    }
}

/// The app mark: the remora on a lime disc.
struct RemoraMark: View {
    var size: CGFloat = 26

    var body: some View {
        Image(nsImage: RemoraArt.mark(size: size, disc: NSColor(hex: 0xB9FF66), fish: NSColor(hex: 0x111111)))
            .frame(width: size, height: size)
    }
}

/// Text that depends on the time ("5m", "updated 3 min ago"): redrawn every minute while on screen, by itself, so
/// the model doesn't publish the clock for it.
struct LiveText: View {
    let text: () -> String

    init(_ text: @escaping () -> String) {
        self.text = text
    }

    var body: some View {
        TimelineView(.everyMinute) { _ in Text(text()) }
    }
}

extension InboxItem {
    /// The context as shown: a reminder's is stored as "Reminder" and translated here, at display, so a
    /// language change applies to reminders saved before it.
    var shownContext: String {
        bundle == .reminders && context == "Reminder" ? L("Reminder") : context
    }
}

extension Date {
    var shortRelative: String {
        let seconds = -timeIntervalSinceNow
        switch seconds {
        case ..<60: return L("now")
        case ..<3_600: return L("%dm", Int(seconds / 60))
        case ..<86_400: return L("%dh", Int(seconds / 3_600))
        case ..<604_800: return L("%dd", Int(seconds / 86_400))
        default: return formatted(.dateTime.day().month(.abbreviated))
        }
    }
}

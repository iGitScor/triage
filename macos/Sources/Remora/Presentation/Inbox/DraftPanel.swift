import AppKit
import RemoraCore
import SwiftUI

/// A message drafted on this Mac. Remora never sends it: you copy it where you want.
struct DraftSubject: Identifiable {
    var item: InboxItem
    var text: String
    var id: String { item.id }
}

struct DraftPanel: View {
    let subject: DraftSubject
    let dismiss: () -> Void

    @State private var text = ""
    @State private var copied = false

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text(L("Draft")).font(Myna.font(15, .semibold)).foregroundStyle(Myna.ink)
                Spacer()
                IconButton(symbol: "xmark", help: "Close", size: 24, action: dismiss)
            }
            Text(subject.item.title).font(Myna.font(12)).foregroundStyle(Myna.muted).lineLimit(1)
            TextEditor(text: $text)
                .font(Myna.font(13))
                .scrollContentBackground(.hidden)
                .padding(8)
                .frame(minHeight: 130)
                .background(Myna.field, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
                .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).strokeBorder(Myna.border))
            Text(L("Written on this Mac. Remora doesn’t send it: paste it in Slack, GitHub or GitLab."))
                .font(Myna.font(11))
                .foregroundStyle(Myna.muted)
            HStack {
                ActionButton(label: copied ? "Copied" : "Copy", symbol: copied ? "checkmark" : "doc.on.doc", action: copy)
                Spacer()
                if let url = subject.item.url {
                    Button {
                        copy()
                        NSWorkspace.shared.open(url)
                        dismiss()
                    } label: { Text(L("Copy & open")) }
                    .buttonStyle(PillButtonStyle())
                }
            }
        }
        .padding(16)
        .background(Myna.surface, in: RoundedRectangle(cornerRadius: Myna.radiusLarge, style: .continuous))
        .shadow(color: .black.opacity(0.2), radius: 20, y: 6)
        .padding(16)
        .onAppear { text = subject.text }
    }

    private func copy() {
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(text, forType: .string)
        copied = true
    }
}

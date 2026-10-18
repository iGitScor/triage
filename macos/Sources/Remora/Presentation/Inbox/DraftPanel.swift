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
    /// The item's page, when Remora may open it (`InboxModel.webLink(for:)`).
    let link: URL?
    let dismiss: () -> Void

    @State private var text = ""
    @State private var copied = false

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text(L("Draft")).font(RemoraDesign.font(15, .semibold)).foregroundStyle(RemoraDesign.ink)
                Spacer()
                IconButton(symbol: "xmark", help: "Close", size: 24, action: dismiss)
            }
            Text(subject.item.title).font(RemoraDesign.font(12)).foregroundStyle(RemoraDesign.muted).lineLimit(1)
            TextEditor(text: $text)
                .font(RemoraDesign.font(13))
                .scrollContentBackground(.hidden)
                .padding(8)
                .frame(minHeight: 130)
                .background(RemoraDesign.field, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
                .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).strokeBorder(RemoraDesign.border))
            Text(L("Written on this Mac. Remora doesn’t send it: paste it in Slack, GitHub or GitLab."))
                .font(RemoraDesign.font(11))
                .foregroundStyle(RemoraDesign.muted)
            HStack {
                ActionButton(
                    label: copied ? "Copied" : "Copy", symbol: copied ? "checkmark" : "doc.on.doc", action: copy)
                Spacer()
                if let url = link {
                    Button {
                        copy()
                        NSWorkspace.shared.open(url)
                        dismiss()
                    } label: {
                        Text(L("Copy & open"))
                    }
                    .buttonStyle(PillButtonStyle())
                }
            }
        }
        .padding(16)
        .background(
            RemoraDesign.surface, in: RoundedRectangle(cornerRadius: RemoraDesign.radiusLarge, style: .continuous)
        )
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

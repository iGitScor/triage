import AppKit
import RemoraCore
import RemoraPlugins
import SwiftUI

/// What may leave this Mac, where data goes, and the local data. Read-only when managed by MDM.
struct PrivacyPane: View {
    @Environment(InboxModel.self) private var model
    @State private var confirmingErase = false
    @State private var eraseError: String?

    /// What an agent asked, in the words of the tabs.
    static func asked(_ read: AgentRead) -> String {
        switch read.command {
        case "count": L("the count")
        case "remind": L("a reminder")
        default:
            switch read.tab ?? "my-turn" {
            case "in-progress": L("In progress")
            case "waiting": L("Waiting")
            case "snoozed": L("Snoozed")
            case "done": L("Done")
            default: L("My turn")
            }
        }
    }

    var body: some View {
        @Bindable var model = model
        let managed = ManagedPolicy.isManaged()
        Form {
            Section {
                Label(
                    L("Remora only talks to the tools you allow. No analytics, no telemetry."),
                    systemImage: "lock.shield"
                )
                .font(Myna.font(12.5))
                if managed {
                    Label(L("Managed by your organization"), systemImage: "building.columns")
                        .font(Myna.font(12.5, .semibold))
                        .foregroundStyle(Myna.accentText)
                }
            }

            Section(L("External AI")) {
                Toggle(L("Allow external AI"), isOn: $model.preferences.allowExternalAI)
                    .disabled(managed)
                Text(
                    L(
                        "The assistant sends the titles, contexts, authors and statuses of inbox items to its provider: Anthropic for Claude, or the server you set. When off, only a server on this computer can write the brief, summaries and triage."
                    )
                )
                .font(Myna.font(11.5))
                .foregroundStyle(Myna.muted)
            }

            Section(L("Allowed tools")) {
                ForEach(PluginRegistry.manifests + PluginRegistry.assistantManifests, id: \.id) { manifest in
                    Toggle(isOn: allowed(manifest.id)) {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(manifest.name).font(Myna.font(13, .medium))
                            Text(L(manifest.egress.description)).font(Myna.font(11)).foregroundStyle(Myna.muted)
                        }
                    }
                    .disabled(managed)
                }
                Toggle(L("Load avatars from allowed tools"), isOn: $model.preferences.allowRemoteImages)
                    .disabled(managed)
            }

            Section(L("Data flows")) {
                if model.accounts.isEmpty {
                    Text(L("Nothing connected.")).foregroundStyle(Myna.muted)
                }
                ForEach(model.accounts) { account in
                    if let manifest = PluginRegistry.manifest(account.pluginID) {
                        let refusal = model.privacy.refusal(for: account)
                        VStack(alignment: .leading, spacing: 3) {
                            HStack {
                                Text(account.name ?? manifest.name).font(Myna.font(13, .semibold))
                                Spacer()
                                Label(
                                    refusal == nil ? L("Allowed") : L("Blocked"),
                                    systemImage: refusal == nil ? "checkmark.circle.fill" : "nosign"
                                )
                                .font(Myna.font(11.5, .semibold))
                                .foregroundStyle(refusal == nil ? Myna.ok : Myna.danger)
                            }
                            let hosts = model.privacy.allowedHosts(for: account, manifest: manifest)
                            Text(hosts.isEmpty ? localProgram(account) : hosts.joined(separator: ", "))
                                .font(Myna.font(11.5, .medium))
                                .foregroundStyle(Myna.inkSoft)
                            Text(refusal ?? L(manifest.egress.description)).font(Myna.font(11)).foregroundStyle(
                                Myna.muted)
                        }
                    }
                }
            }

            Section(L("Agents on this Mac")) {
                Text(
                    L(
                        "AI agents such as Claude Desktop, Claude Code or Gemini CLI can ask Remora what waits on you, through MCP. They read the inbox; they never write to your tools."
                    )
                )
                .font(Myna.font(11.5))
                .foregroundStyle(Myna.muted)
                if !LocalAPIPolicy.allowed() {
                    Label(L("Turned off by your organization."), systemImage: "building.columns")
                        .font(Myna.font(12, .semibold))
                } else if model.agentReads.isEmpty {
                    Text(L("No agent has asked yet.")).font(Myna.font(12)).foregroundStyle(Myna.muted)
                } else {
                    // The most recent first; the name is what the agent says it is.
                    ForEach(Array(model.agentReads.suffix(5).reversed().enumerated()), id: \.offset) { _, read in
                        HStack {
                            Text(read.agent).font(Myna.font(12.5, .medium))
                            Text(Self.asked(read)).font(Myna.font(12)).foregroundStyle(Myna.inkSoft)
                            Spacer()
                            Text(read.at, format: .relative(presentation: .named))
                                .font(Myna.font(11.5)).foregroundStyle(Myna.muted)
                        }
                    }
                }
                Link(L("How to set it up"), destination: URL(string: "https://triage.iscor.me/docs/guide/agents")!)
                    .font(Myna.font(12))
            }

            Section(L("Data on this Mac")) {
                Text(AppFolder.url.path).font(Myna.system(11)).textSelection(.enabled)
                Text(L("Inbox cache, settings and snooze history. Tokens are in the Keychain.")).font(Myna.font(11))
                    .foregroundStyle(Myna.muted)
                Toggle(L("Find inbox items in Spotlight"), isOn: $model.preferences.spotlight)
                Text(
                    L(
                        "Puts each item’s title and where it comes from in this Mac’s Spotlight index, never previews or people. Tools whose content is hidden stay out. Turning this off, disconnecting a tool or erasing removes them."
                    )
                )
                .font(Myna.font(11))
                .foregroundStyle(Myna.muted)
                // A file Remora couldn't read (kept aside, never overwritten) or save.
                ForEach(model.storageIssues) { issue in
                    Label(issue.message, systemImage: "exclamationmark.triangle.fill")
                        .font(Myna.font(11.5))
                        .foregroundStyle(Myna.color(for: .warning).foreground)
                        .fixedSize(horizontal: false, vertical: true)
                }
                HStack {
                    Button(L("Show in Finder")) { NSWorkspace.shared.activateFileViewerSelecting([AppFolder.url]) }
                    Spacer()
                    Button(L("Erase local data…"), role: .destructive) { confirmingErase = true }
                }
            }
        }
        .formStyle(.grouped)
        .confirmationDialog(L("Erase all of Remora’s local data?"), isPresented: $confirmingErase) {
            Button(L("Erase"), role: .destructive) {
                do { try model.eraseLocalData() } catch { eraseError = error.localizedDescription }
            }
        } message: {
            Text(
                L(
                    "Disconnects every account, deletes the cache, history, settings and notifications, and removes the tokens from the Keychain."
                ))
        }
        .alert(
            L("The tokens are still in the Keychain"),
            isPresented: Binding(get: { eraseError != nil }, set: { if !$0 { eraseError = nil } })
        ) {
            Button(L("OK")) { eraseError = nil }
        } message: {
            Text(eraseError ?? "")
        }
    }

    private func allowed(_ id: String) -> Binding<Bool> {
        Binding(
            get: { model.preferences.allowedPlugins?.contains(id) ?? true },
            set: { isOn in
                var allowed = Set(
                    model.preferences.allowedPlugins
                        ?? (PluginRegistry.manifests + PluginRegistry.assistantManifests).map(\.id))
                if isOn { allowed.insert(id) } else { allowed.remove(id) }
                model.preferences.allowedPlugins = allowed.sorted()
            }
        )
    }

    /// For Claude Code, the program that runs: the user sees which `claude` gets their inbox.
    private func localProgram(_ account: Account) -> String {
        guard account.pluginID == ClaudeCodePlugin.manifest.id else { return L("Local program on this Mac") }
        guard let program = ClaudeCodePlugin.locate(ManagedPolicy.claudeCodePath() ?? account.settings["path"] ?? "")
        else { return L("Claude Code not found on this Mac") }
        return L("Local program: %@", program.path)
    }
}

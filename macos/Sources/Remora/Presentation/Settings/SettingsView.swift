import RemoraCore
import RemoraPlugins
import ServiceManagement
import SwiftUI

struct SettingsView: View {
    @Environment(InboxModel.self) private var model

    var body: some View {
        TabView {
            SourcesPane().tabItem { Label("Sources", systemImage: "tray.2") }
            GeneralPane().tabItem { Label("General", systemImage: "gearshape") }
            PrivacyPane().tabItem { Label("Privacy", systemImage: "lock.shield") }
        }
        .frame(width: 560, height: 520)
        .tint(Myna.accentDeep)
        .preferredColorScheme(model.preferences.appearance.colorScheme)
    }
}

private struct SourcesPane: View {
    @Environment(InboxModel.self) private var model
    @State private var adding: PluginManifest?

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                heading("Connected")
                if model.accounts.isEmpty {
                    Text("Nothing connected yet.").font(Myna.font(13)).foregroundStyle(Myna.muted)
                }
                ForEach(model.accounts) { AccountRow(account: $0) }
                heading("Add a source")
                tiles(PluginRegistry.manifests)
                heading("Assistant")
                tiles(PluginRegistry.assistantManifests)
            }
            .padding(20)
        }
        .background(Myna.backdrop)
        .sheet(item: Binding(get: { adding.map(ManifestBox.init) }, set: { adding = $0?.manifest })) { box in
            ConnectForm(manifest: box.manifest) { adding = nil }
        }
    }

    private func heading(_ title: String) -> some View {
        Text(L(title)).font(Myna.font(13, .semibold)).foregroundStyle(Myna.muted).padding(.top, 4)
    }

    private func tiles(_ manifests: [PluginManifest]) -> some View {
        LazyVGrid(columns: [GridItem(.flexible()), GridItem(.flexible())], spacing: 10) {
            ForEach(manifests, id: \.id) { manifest in
                Button { adding = manifest } label: { PluginTile(manifest: manifest) }
                    .buttonStyle(.plain)
                    .disabled(manifest.isComingSoon)
            }
        }
    }
}

private struct ManifestBox: Identifiable {
    let manifest: PluginManifest
    var id: String { manifest.id }
}

private struct AccountRow: View {
    @Environment(InboxModel.self) private var model
    let account: Account

    @State private var editing = false
    @State private var draft = ""
    @FocusState private var focused: Bool

    var body: some View {
        let manifest = PluginRegistry.manifest(account.pluginID)
        HStack(spacing: 12) {
            Image(systemName: manifest?.symbol ?? "questionmark")
                .font(.system(size: 13, weight: .bold))
                .foregroundStyle(Myna.onAccent)
                .frame(width: 32, height: 32)
                .background(Myna.accent, in: Circle())
            VStack(alignment: .leading, spacing: 2) {
                if editing {
                    TextField(manifest?.name ?? "Name", text: $draft)
                        .textFieldStyle(.roundedBorder)
                        .font(Myna.font(14, .semibold))
                        .focused($focused)
                        .onSubmit(save)
                        .onExitCommand { editing = false }
                } else {
                    HStack(spacing: 6) {
                        Text(account.name ?? manifest?.name ?? account.pluginID)
                            .font(Myna.font(14, .semibold))
                            .foregroundStyle(Myna.ink)
                        if account.name != nil, let manifest {
                            Text(manifest.name).font(Myna.font(11.5)).foregroundStyle(Myna.muted)
                        }
                        Button(action: startEditing) { Image(systemName: "pencil") }
                            .buttonStyle(.plain)
                            .foregroundStyle(Myna.muted)
                            .help("Rename")
                    }
                }
                if let error = model.errors[account.id] {
                    Text(error).font(Myna.font(11.5)).foregroundStyle(Myna.danger).lineLimit(2)
                } else {
                    Text(account.subtitle).font(Myna.font(11.5)).foregroundStyle(Myna.muted)
                }
            }
            Spacer()
            Button("Remove") { model.disconnect(account) }
                .buttonStyle(PillButtonStyle(prominent: false))
        }
        .padding(12)
        .background(Myna.card, in: RoundedRectangle(cornerRadius: Myna.radiusMedium, style: .continuous))
    }

    private func startEditing() {
        draft = account.name ?? ""
        editing = true
        focused = true
    }

    private func save() {
        model.rename(account, to: draft)
        editing = false
    }
}

private struct PluginTile: View {
    let manifest: PluginManifest
    @State private var hovering = false

    var body: some View {
        HStack(alignment: .top, spacing: 10) {
            Image(systemName: manifest.symbol)
                .font(.system(size: 13, weight: .bold))
                .foregroundStyle(Myna.onDark)
                .frame(width: 30, height: 30)
                .background(Myna.dark, in: Circle())
            VStack(alignment: .leading, spacing: 3) {
                HStack(spacing: 6) {
                    Text(manifest.name).font(Myna.font(13.5, .semibold)).foregroundStyle(Myna.ink)
                    if manifest.isComingSoon { SoonBadge() }
                }
                Text(L(manifest.summary)).font(Myna.font(11)).foregroundStyle(Myna.muted).lineLimit(3)
            }
            Spacer(minLength: 0)
        }
        .padding(12)
        .frame(maxWidth: .infinity, minHeight: 84, alignment: .topLeading)
        .background(hovering && !manifest.isComingSoon ? Myna.card2 : Myna.card, in: RoundedRectangle(cornerRadius: Myna.radiusMedium, style: .continuous))
        .opacity(manifest.isComingSoon ? 0.6 : 1)
        .contentShape(Rectangle())
        .onHover { hovering = $0 }
    }
}

/// A form built from the plugin's declared fields; connecting validates by fetching once.
private struct ConnectForm: View {
    @Environment(InboxModel.self) private var model
    let manifest: PluginManifest
    let dismiss: () -> Void

    @State private var name = ""
    @State private var values: [String: String] = [:]
    @State private var connecting = false
    @State private var error: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            ScrollView {
                VStack(alignment: .leading, spacing: 14) {
                    header
                    if !manifest.setupSteps.isEmpty { SetupGuide(manifest: manifest, config: config) }
                    field("Name (optional)", help: "Tells accounts apart, e.g. “Work” or “Side project”.") {
                        TextField(manifest.name, text: $name)
                    }
                    ForEach(manifest.fields) { field in
                        self.field(field.isOptional ? L("%@ (optional)", L(field.label)) : field.label, help: field.help) {
                            if field.isSecret {
                                SecureField(L(field.placeholder), text: binding(field))
                            } else {
                                TextField(L(field.placeholder), text: binding(field))
                            }
                        }
                    }
                    if let error {
                        Text(error).font(Myna.font(12)).foregroundStyle(Myna.danger)
                    }
                }
                .padding(22)
            }
            HStack {
                Spacer()
                Button("Cancel", action: dismiss).buttonStyle(PillButtonStyle(prominent: false))
                Button(connecting ? L("Connecting…") : L("Connect"), action: connect)
                    .buttonStyle(PillButtonStyle())
                    .disabled(connecting)
                    .keyboardShortcut(.defaultAction)
            }
            .padding(16)
            .overlay(alignment: .top) { Myna.line.frame(height: 1) }
        }
        .frame(width: 460, height: 560)
        .onAppear {
            for field in manifest.fields where values[field.key] == nil { values[field.key] = field.defaultValue }
        }
    }

    private var header: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 10) {
                Image(systemName: manifest.symbol)
                    .font(.system(size: 13, weight: .bold))
                    .foregroundStyle(Myna.onAccent)
                    .frame(width: 30, height: 30)
                    .background(Myna.accent, in: Circle())
                Text("Connect \(manifest.name)").font(Myna.font(17, .semibold))
            }
            Text(L(manifest.summary)).font(Myna.font(12.5)).foregroundStyle(Myna.muted)
        }
    }

    private func field(_ label: String, help: String?, @ViewBuilder input: () -> some View) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(L(label)).font(Myna.font(12, .semibold))
            input().textFieldStyle(.roundedBorder)
            if let help {
                Text(L(help)).font(Myna.font(11)).foregroundStyle(Myna.muted)
            }
        }
    }

    private var config: PluginConfig { PluginConfig(accountID: UUID(), values: values) }

    private func binding(_ field: ConfigField) -> Binding<String> {
        Binding(get: { values[field.key] ?? "" }, set: { values[field.key] = $0 })
    }

    private func connect() {
        let secretKeys = Set(manifest.fields.filter(\.isSecret).map(\.key))
        let settings = values.filter { !secretKeys.contains($0.key) }
        let secrets = values.filter { secretKeys.contains($0.key) }
        connecting = true
        error = nil
        Task {
            defer { connecting = false }
            do {
                try await model.connect(pluginID: manifest.id, name: name, settings: settings, secrets: secrets)
                dismiss()
            } catch {
                self.error = error.localizedDescription
            }
        }
    }
}

/// Numbered setup steps with the button that opens the provider's page.
private struct SetupGuide: View {
    let manifest: PluginManifest
    let config: PluginConfig

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            ForEach(Array(manifest.setupSteps.enumerated()), id: \.offset) { index, step in
                HStack(alignment: .top, spacing: 10) {
                    Text("\(index + 1)")
                        .font(Myna.font(11, .bold))
                        .foregroundStyle(Myna.onAccent)
                        .frame(width: 20, height: 20)
                        .background(Myna.accent, in: Circle())
                    Text(L(step))
                        .font(Myna.font(12.5))
                        .foregroundStyle(Myna.ink)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
            if let url = manifest.setupURL?(config) {
                Link(destination: url) {
                    Label(L(manifest.setupLabel), systemImage: "arrow.up.right")
                        .font(Myna.font(13, .semibold))
                        .foregroundStyle(Myna.onDark)
                        .padding(.horizontal, 14)
                        .padding(.vertical, 7)
                        .background(Myna.dark, in: Capsule())
                }
                .buttonStyle(.plain)
                .padding(.leading, 30)
            }
        }
        .padding(14)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Myna.card, in: RoundedRectangle(cornerRadius: Myna.radiusMedium, style: .continuous))
    }
}

private struct GeneralPane: View {
    static let version: String = {
        let info = Bundle.main.infoDictionary ?? [:]
        let version = info["CFBundleShortVersionString"] as? String ?? "dev"
        let build = info["CFBundleVersion"] as? String ?? "0"
        return "\(version) (\(build))"
    }()

    @Environment(InboxModel.self) private var model
    @State private var launchAtLogin = SMAppService.mainApp.status == .enabled

    var body: some View {
        @Bindable var model = model
        Form {
            Section("Refresh") {
                Picker("Check every", selection: $model.preferences.refreshMinutes) {
                    ForEach([1, 2, 5, 10, 15, 30], id: \.self) { Text("\($0) min").tag($0) }
                }
                Toggle("Open in desktop apps when installed", isOn: $model.preferences.openInApps)
                Text(L("Slack and Linear open in their app; ⌥-click opens the web page instead."))
                    .font(Myna.font(11.5))
                    .foregroundStyle(Myna.muted)
                Toggle("Open at login", isOn: $launchAtLogin)
                    .onChange(of: launchAtLogin) { _, enabled in
                        try? enabled ? SMAppService.mainApp.register() : SMAppService.mainApp.unregister()
                    }
            }
            Section("Menu bar") {
                Picker("Show count of", selection: $model.preferences.menuBarCount) {
                    ForEach(Preferences.MenuBarCount.allCases) { Text($0.label).tag($0) }
                }
                Toggle("One counter per source", isOn: $model.preferences.countPerSource)
                    .disabled(model.preferences.menuBarCount == .hidden)
                Toggle("Lime pill when something needs me", isOn: $model.preferences.brandedMenuBar)
                    .disabled(model.preferences.menuBarCount == .hidden)
            }
            Section("Notifications") {
                Toggle("New items that need me (reviews, mentions…)", isOn: $model.preferences.notifyArrivals)
                Toggle("Status changes (approved, changes requested, checks failed)", isOn: $model.preferences.notifyStatusChanges)
            }
            Section("Assistant") {
                Toggle("Whole-inbox brief", isOn: $model.preferences.wholeInboxBrief)
                Text("Off: no Brief button and no brief is written. The ✦ summary on each bundle stays available.")
                    .font(Myna.font(11.5))
                    .foregroundStyle(Myna.muted)
                Picker("Reuse a brief or summary for", selection: $model.preferences.briefCacheMinutes) {
                    Text("Always write a new one").tag(0)
                    ForEach([5, 15, 30, 60, 120], id: \.self) { Text($0 < 60 ? L("%d min", $0) : L("%d h", $0 / 60)).tag($0) }
                }
                Text("While fresh, Remora shows it again instead of asking Claude.")
                    .font(Myna.font(11.5))
                    .foregroundStyle(Myna.muted)
            }
            Section("Snooze") {
                Toggle("Bring snoozed items back early on new activity", isOn: $model.preferences.wakeOnActivity)
            }
            Section("Appearance") {
                Picker("Theme", selection: $model.preferences.appearance) {
                    ForEach(Preferences.Appearance.allCases) { Text($0.label).tag($0) }
                }
                .pickerStyle(.segmented)
            }
            Section {
                LabeledContent("Version", value: Self.version)
            }
        }
        .formStyle(.grouped)
    }
}

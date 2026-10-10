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
        // Drawn again with the new size when it changes.
        .id(model.preferences.textSize)
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
                // What the policy refuses can't be started, and says why, before any form is filled in.
                let refusal = model.policy.refusal(for: manifest)
                Button {
                    adding = manifest
                } label: {
                    PluginTile(manifest: manifest, refusal: refusal)
                }
                .buttonStyle(.plain)
                .disabled(manifest.isComingSoon || refusal != nil)
                .accessibilityHint(refusal ?? "")
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
    @State private var confirmingRemove = false
    @State private var reconnecting = false
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
                if let failure = model.errors[account.id] {
                    Text(failure.message).font(Myna.font(11.5)).foregroundStyle(Self.color(failure.kind)).lineLimit(2)
                    // A rejected token is fixed here, keeping the account's Done, snoozes and pins.
                    if failure.kind == .auth, manifest != nil {
                        Button(L("Reconnect…")) { reconnecting = true }
                            .controlSize(.small)
                            .sheet(isPresented: $reconnecting) {
                                if let manifest {
                                    ConnectForm(manifest: manifest, account: account) { reconnecting = false }
                                }
                            }
                    }
                } else {
                    Text(account.subtitle).font(Myna.font(11.5)).foregroundStyle(Myna.muted)
                }
                ForEach(model.remarks[account.id] ?? [], id: \.self) { remark in
                    Label(remark, systemImage: "info.circle")
                        .font(Myna.font(11.5))
                        .foregroundStyle(Myna.inkSoft)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
            Spacer()
            Button("Remove") { confirmingRemove = true }
                .buttonStyle(PillButtonStyle(prominent: false))
                // The token is deleted from the Keychain: no undo, so it asks first.
                .confirmationDialog(
                    L(
                        "Remove %@?",
                        account.name ?? PluginRegistry.manifest(account.pluginID)?.name ?? account.pluginID),
                    isPresented: $confirmingRemove
                ) {
                    Button(L("Remove"), role: .destructive) { model.disconnect(account) }
                } message: {
                    Text(L("Its token is deleted from the Keychain and its items leave the inbox."))
                }
        }
        .padding(12)
        .background(Myna.card, in: RoundedRectangle(cornerRadius: Myna.radiusMedium, style: .continuous))
    }

    /// Calm for what isn't the tool's fault or only means waiting; a warning for a token to replace.
    static func color(_ kind: FailureKind) -> Color {
        switch kind {
        case .offline, .rateLimited: Myna.muted
        case .auth: Myna.color(for: .warning).foreground
        case .unreachable, .other: Myna.danger
        }
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
    /// Why the privacy policy refuses this tool, if it does: the tile is then locked.
    var refusal: String? = nil
    @State private var hovering = false

    private var available: Bool { !manifest.isComingSoon && refusal == nil }

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
                if let refusal {
                    Label(refusal, systemImage: "lock.fill")
                        .font(Myna.font(11))
                        .foregroundStyle(Myna.inkSoft)
                        .lineLimit(3)
                } else {
                    Text(L(manifest.summary)).font(Myna.font(11)).foregroundStyle(Myna.muted).lineLimit(3)
                }
            }
            Spacer(minLength: 0)
        }
        .padding(12)
        .frame(maxWidth: .infinity, minHeight: 84, alignment: .topLeading)
        .background(
            hovering && available ? Myna.card2 : Myna.card,
            in: RoundedRectangle(cornerRadius: Myna.radiusMedium, style: .continuous)
        )
        .opacity(available ? 1 : 0.6)
        .contentShape(Rectangle())
        .onHover { hovering = $0 }
    }
}

/// A form built from the plugin's declared fields; connecting validates by fetching once.
private struct ConnectForm: View {
    @Environment(InboxModel.self) private var model
    let manifest: PluginManifest
    /// Reconnecting an account: its name and settings are filled in, only the token is asked again.
    var account: Account? = nil
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
                        let locked = forcedServer != nil && field.key == manifest.egress.serverField
                        self.field(
                            field.isOptional ? L("%@ (optional)", L(field.label)) : field.label, help: field.help
                        ) {
                            if field.isSecret {
                                SecureField(L(field.placeholder), text: binding(field))
                            } else {
                                // The organization's `AIServer`: shown, not editable.
                                TextField(
                                    L(field.placeholder), text: locked ? .constant(forcedServer ?? "") : binding(field)
                                )
                                .disabled(locked)
                                .help(locked ? L("Managed by your organization") : "")
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
            if let account {
                name = account.name ?? ""
                values = account.settings
            }
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

    /// The AI server the organization set, for a plugin whose server the user chooses.
    private var forcedServer: String? {
        manifest.egress.serverField == nil ? nil : ManagedPolicy.aiServer()
    }

    private func binding(_ field: ConfigField) -> Binding<String> {
        Binding(get: { values[field.key] ?? "" }, set: { values[field.key] = $0 })
    }

    private func connect() {
        let secretKeys = Set(manifest.fields.filter(\.isSecret).map(\.key))
        var settings = values.filter { !secretKeys.contains($0.key) }
        if let field = manifest.egress.serverField, let forcedServer { settings[field] = forcedServer }
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
    /// Read back from the system after each change, never assumed.
    @State private var loginStatus = SMAppService.mainApp.status
    @State private var loginError: String?
    /// The user turned notifications off for Remora in System Settings.
    @State private var notificationsDenied = false

    var body: some View {
        @Bindable var model = model
        Form {
            Section("Refresh") {
                if ManagedPolicy.isForced(ManagedPolicy.refreshMinutesKey) {
                    LabeledContent(L("Check every"), value: L("%d min", model.effectivePreferences.refreshMinutes))
                        .help(L("Managed by your organization"))
                } else {
                    Picker("Check every", selection: $model.preferences.refreshMinutes) {
                        ForEach([1, 2, 5, 10, 15, 30], id: \.self) { Text("\($0) min").tag($0) }
                    }
                }
                Toggle(
                    "Open in desktop apps when installed",
                    isOn: Binding(
                        get: { model.effectivePreferences.openInApps },
                        set: { model.preferences.openInApps = $0 }
                    )
                )
                .managedByOrganization(ManagedPolicy.isForced(ManagedPolicy.openInAppsKey))
                Text(L("Slack and Linear open in their app; ⌥-click opens the web page instead."))
                    .font(Myna.font(11.5))
                    .foregroundStyle(Myna.muted)
                Toggle(
                    "Open at login",
                    isOn: Binding(
                        get: { loginStatus == .enabled || loginStatus == .requiresApproval },
                        // A closure, not the method itself: Swift 6.2 crashes converting that method.
                        set: { setOpenAtLogin($0) }
                    ))
                if loginStatus == .requiresApproval {
                    hint(
                        L("macOS needs your approval: allow Remora in System Settings → General → Login Items."),
                        action: L("Open Login Items")
                    ) { SMAppService.openSystemSettingsLoginItems() }
                }
                if let loginError {
                    Text(L("Couldn’t change it: %@", loginError)).font(Myna.font(11.5)).foregroundStyle(Myna.danger)
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
                Toggle("Only show the task in progress while one runs", isOn: $model.preferences.focusWhileInProgress)
            }
            Section("Notifications") {
                if notificationsDenied {
                    hint(
                        L(
                            "Notifications are off for Remora in System Settings: these choices apply once you allow them."
                        ),
                        action: L("Open Notifications")
                    ) { Notifier.openSystemSettings() }
                }
                Toggle("New items that need me (reviews, mentions…)", isOn: $model.preferences.notifyArrivals)
                Toggle(
                    "Status changes (approved, changes requested, checks failed)",
                    isOn: $model.preferences.notifyStatusChanges)
            }
            Section {
                ForEach(notifyingSources, id: \.id) { source in
                    let locked = ManagedPolicy.managedHiddenContent().contains(source.id)
                    Toggle(source.name, isOn: hidesContent(of: source.id))
                        .managedByOrganization(locked)
                }
            } header: {
                Text("Hide message content")
            } footer: {
                Text(
                    "Notifications still say where something happened (mention, channel, repository), not what was written."
                )
                .font(Myna.font(11.5))
                .foregroundStyle(Myna.muted)
            }
            Section("Assistant") {
                Toggle("Whole-inbox brief", isOn: $model.preferences.wholeInboxBrief)
                Text("Off: no Brief button and no brief is written. The ✦ summary on each bundle stays available.")
                    .font(Myna.font(11.5))
                    .foregroundStyle(Myna.muted)
                Picker("Reuse a brief or summary for", selection: $model.preferences.briefCacheMinutes) {
                    Text("Always write a new one").tag(0)
                    ForEach([5, 15, 30, 60, 120], id: \.self) {
                        Text($0 < 60 ? L("%d min", $0) : L("%d h", $0 / 60)).tag($0)
                    }
                }
                Text("While fresh, Remora shows it again instead of asking the assistant.")
                    .font(Myna.font(11.5))
                    .foregroundStyle(Myna.muted)
            }
            Section {
                ForEach(notifyingSources, id: \.id) { source in
                    let locked = ManagedPolicy.managedAIExclusions().contains(source.id)
                    Toggle(source.name, isOn: sendsToAssistant(source.id))
                        .disabled(locked)
                        .help(locked ? L("Managed by your organization") : "")
                }
            } header: {
                Text("Send to the assistant")
            } footer: {
                Text("Items from a tool that's off never reach the assistant: not in the brief, summaries or triage.")
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
                Picker("Text size", selection: $model.preferences.textSize) {
                    ForEach(Preferences.TextSize.allCases) { Text($0.label).tag($0) }
                }
                .pickerStyle(.segmented)
            }
            UpdatesSection(version: Self.version)
        }
        .formStyle(.grouped)
        .task { await readSystemSettings() }
        // Back from System Settings: what the user changed there shows at once.
        .onReceive(NotificationCenter.default.publisher(for: NSApplication.didBecomeActiveNotification)) { _ in
            Task { await readSystemSettings() }
        }
    }

    private func readSystemSettings() async {
        loginStatus = SMAppService.mainApp.status
        notificationsDenied = await Notifier.shared.isDenied()
    }

    private func setOpenAtLogin(_ enabled: Bool) {
        do {
            try enabled ? SMAppService.mainApp.register() : SMAppService.mainApp.unregister()
            loginError = nil
        } catch {
            loginError = error.localizedDescription
        }
        loginStatus = SMAppService.mainApp.status
    }

    /// A short explanation with the button that fixes it.
    private func hint(_ text: String, action: String, perform: @escaping () -> Void) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: 8) {
            Image(systemName: "exclamationmark.triangle.fill").foregroundStyle(Myna.color(for: .warning).foreground)
            Text(text).font(Myna.font(11.5)).foregroundStyle(Myna.inkSoft).fixedSize(horizontal: false, vertical: true)
            Spacer(minLength: 4)
            Button(action, action: perform).controlSize(.small)
        }
    }

    /// Connected integrations that can notify, plus your own reminders.
    private var notifyingSources: [(id: String, name: String)] {
        let connected = Set(model.accounts.map(\.pluginID))
        let sources = PluginRegistry.manifests
            .filter { connected.contains($0.id) && !PluginRegistry.isAssistant($0.id) }
            .map { (id: $0.id, name: $0.name) }
        return sources + [(id: "reminders", name: L("Reminders"))]
    }

    private func sendsToAssistant(_ pluginID: String) -> Binding<Bool> {
        Binding(
            get: { !model.assistantPolicy.excludedSources.contains(pluginID) },
            set: { send in
                if send {
                    model.preferences.assistantExcludedSources.remove(pluginID)
                } else {
                    model.preferences.assistantExcludedSources.insert(pluginID)
                }
            }
        )
    }

    private func hidesContent(of pluginID: String) -> Binding<Bool> {
        Binding(
            get: { model.effectivePreferences.hiddenContentPlugins.contains(pluginID) },
            set: { hidden in
                if hidden {
                    model.preferences.hiddenContentPlugins.insert(pluginID)
                } else {
                    model.preferences.hiddenContentPlugins.remove(pluginID)
                }
            }
        )
    }
}

extension View {
    /// A setting the organization forced: locked, and says why.
    func managedByOrganization(_ locked: Bool) -> some View {
        disabled(locked).help(locked ? L("Managed by your organization") : "")
    }
}

/// The version, and updates once the user turns them on. Off by default: nothing calls home before.
private struct UpdatesSection: View {
    let version: String
    @Environment(InboxModel.self) private var model

    var body: some View {
        @Bindable var model = model
        let updater = model.updater
        Section {
            LabeledContent("Version", value: version)
            if !updater.isSupported {
                Text(L("This copy of Remora doesn’t update itself: download new versions from the releases page."))
                    .font(Myna.font(11.5)).foregroundStyle(Myna.muted)
            } else if !model.updatesAllowed {
                Text(L("Your organization installs new versions of Remora.")).font(Myna.font(11.5)).foregroundStyle(
                    Myna.muted
                )
                .help(L("Managed by your organization"))
            } else {
                Toggle(
                    "Check for updates automatically",
                    isOn: Binding(
                        get: { model.effectivePreferences.checkForUpdates },
                        set: { model.preferences.checkForUpdates = $0 }
                    )
                )
                .managedByOrganization(ManagedPolicy.isForced(ManagedPolicy.automaticUpdatesKey))
                status(updater)
            }
        } header: {
            Text("Updates")
        } footer: {
            if updater.isSupported, model.updatesAllowed {
                Text(
                    "Once a day, Remora asks GitHub whether a new version is out. Nothing about you or your inbox is sent. A new version is installed only when you choose, and only if it is signed by Remora’s release key."
                )
                .font(Myna.font(11.5))
                .foregroundStyle(Myna.muted)
            }
        }
    }

    @ViewBuilder private func status(_ updater: Updater) -> some View {
        switch updater.state {
        case .available(let offer):
            HStack {
                Text(L("Remora %@ is available.", offer.version.description)).font(Myna.font(12, .semibold))
                if let notes = offer.notes.flatMap(URL.init(string:)), UpdateHosts.allows(notes) {
                    Link(L("What’s new"), destination: notes).font(Myna.font(11.5))
                }
                Spacer()
                Button(L("Install and relaunch")) { Task { await updater.install() } }
            }
        case .installing:
            HStack(spacing: 8) {
                ProgressView().controlSize(.small)
                Text(L("Downloading and checking the new version…")).font(Myna.font(11.5)).foregroundStyle(Myna.inkSoft)
            }
        default:
            HStack {
                switch updater.state {
                case .checking:
                    ProgressView().controlSize(.small)
                    Text(L("Checking…")).font(Myna.font(11.5)).foregroundStyle(Myna.muted)
                case .upToDate:
                    Text(L("Remora is up to date.")).font(Myna.font(11.5)).foregroundStyle(Myna.muted)
                case .failed(let message):
                    Text(message).font(Myna.font(11.5)).foregroundStyle(Myna.danger).fixedSize(
                        horizontal: false, vertical: true)
                default:
                    EmptyView()
                }
                Spacer()
                Button(L("Check now")) { Task { await model.checkForUpdates() } }
                    .disabled(updater.state == .checking)
            }
        }
    }
}

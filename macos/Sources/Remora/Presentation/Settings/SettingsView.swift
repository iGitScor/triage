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
        .tint(RemoraDesign.accentDeep)
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
                    Text("Nothing connected yet.").font(RemoraDesign.font(13)).foregroundStyle(RemoraDesign.muted)
                }
                ForEach(model.accounts) { AccountRow(account: $0) }
                heading("Add a source")
                tiles(PluginRegistry.manifests)
                heading("Assistant")
                tiles(PluginRegistry.assistantManifests)
            }
            .padding(20)
        }
        .background(RemoraDesign.backdrop)
        .sheet(item: Binding(get: { adding.map(ManifestBox.init) }, set: { adding = $0?.manifest })) { box in
            ConnectForm(manifest: box.manifest) { adding = nil }
        }
    }

    private func heading(_ title: String) -> some View {
        Text(L(title)).font(RemoraDesign.font(13, .semibold)).foregroundStyle(RemoraDesign.muted).padding(.top, 4)
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
                .foregroundStyle(RemoraDesign.onAccent)
                .frame(width: 32, height: 32)
                .background(RemoraDesign.accent, in: Circle())
            VStack(alignment: .leading, spacing: 2) {
                if editing {
                    TextField(manifest?.name ?? "Name", text: $draft)
                        .textFieldStyle(.roundedBorder)
                        .font(RemoraDesign.font(14, .semibold))
                        .focused($focused)
                        .onSubmit(save)
                        .onExitCommand { editing = false }
                } else {
                    HStack(spacing: 6) {
                        Text(account.name ?? manifest?.name ?? account.pluginID)
                            .font(RemoraDesign.font(14, .semibold))
                            .foregroundStyle(RemoraDesign.ink)
                        if account.name != nil, let manifest {
                            Text(manifest.name).font(RemoraDesign.font(11.5)).foregroundStyle(RemoraDesign.muted)
                        }
                        Button(action: startEditing) { Image(systemName: "pencil") }
                            .buttonStyle(.plain)
                            .foregroundStyle(RemoraDesign.muted)
                            .help("Rename")
                            .accessibilityLabel(L("Rename"))
                    }
                }
                let failure = model.sync.errors[account.id]
                if let failure {
                    Text(failure.message).font(RemoraDesign.font(11.5)).foregroundStyle(Self.color(failure.kind))
                        .lineLimit(2)
                } else {
                    Text(account.subtitle).font(RemoraDesign.font(11.5)).foregroundStyle(RemoraDesign.muted)
                }
                // A new token goes in here, keeping the account's Done, snoozes and pins: after a rejection, or any
                // time (a token about to expire). Not for an account without one (Claude Code).
                if let manifest, manifest.fields.contains(where: \.isSecret) {
                    Button(failure?.kind == .auth ? L("Reconnect…") : L("Update token…")) { reconnecting = true }
                        .controlSize(.small)
                        .sheet(isPresented: $reconnecting) {
                            ConnectForm(manifest: manifest, account: account) { reconnecting = false }
                        }
                }
                ForEach(model.sync.remarks[account.id] ?? [], id: \.self) { remark in
                    Label(remark, systemImage: "info.circle")
                        .font(RemoraDesign.font(11.5))
                        .foregroundStyle(RemoraDesign.inkSoft)
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
        .background(
            RemoraDesign.card, in: RoundedRectangle(cornerRadius: RemoraDesign.radiusMedium, style: .continuous))
    }

    /// Calm for what isn't the tool's fault or only means waiting; a warning for a token to replace.
    static func color(_ kind: FailureKind) -> Color {
        switch kind {
        case .offline, .rateLimited: RemoraDesign.muted
        case .auth: RemoraDesign.color(for: .warning).foreground
        case .unreachable, .other: RemoraDesign.danger
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
                .foregroundStyle(RemoraDesign.onDark)
                .frame(width: 30, height: 30)
                .background(RemoraDesign.dark, in: Circle())
            VStack(alignment: .leading, spacing: 3) {
                HStack(spacing: 6) {
                    Text(manifest.name).font(RemoraDesign.font(13.5, .semibold)).foregroundStyle(RemoraDesign.ink)
                    if manifest.isComingSoon { SoonBadge() }
                }
                if let refusal {
                    Label(refusal, systemImage: "lock.fill")
                        .font(RemoraDesign.font(11))
                        .foregroundStyle(RemoraDesign.inkSoft)
                        .lineLimit(3)
                } else {
                    Text(L(manifest.summary)).font(RemoraDesign.font(11)).foregroundStyle(RemoraDesign.muted).lineLimit(
                        3)
                }
            }
            Spacer(minLength: 0)
        }
        .padding(12)
        .frame(maxWidth: .infinity, minHeight: 84, alignment: .topLeading)
        .background(
            hovering && available ? RemoraDesign.card2 : RemoraDesign.card,
            in: RoundedRectangle(cornerRadius: RemoraDesign.radiusMedium, style: .continuous)
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
    /// Waiting for the browser to come back from signing in.
    @State private var signingIn = false
    @State private var connection: Task<Void, Never>?
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
                        let forced = forcedValue(field)
                        let locked = forced != nil
                        if field.isToggle {
                            toggle(field, locked: locked, forced: forced)
                        } else {
                            self.field(
                                field.isOptional ? L("%@ (optional)", L(field.label)) : field.label, help: field.help
                            ) {
                                if field.isSecret {
                                    SecureField(L(field.placeholder), text: binding(field))
                                } else {
                                    // The organization's `AIServer`, or its value for this field: shown, not editable.
                                    TextField(
                                        L(field.placeholder), text: locked ? .constant(forced ?? "") : binding(field)
                                    )
                                    .disabled(locked)
                                    .help(locked ? L("Managed by your organization") : "")
                                }
                            }
                        }
                    }
                    if let error {
                        Text(error).font(RemoraDesign.font(12)).foregroundStyle(RemoraDesign.danger)
                    }
                }
                .padding(22)
            }
            HStack {
                Spacer()
                Button("Cancel") {
                    // Stops waiting for the browser.
                    connection?.cancel()
                    dismiss()
                }
                .buttonStyle(PillButtonStyle(prominent: false))
                Button(connectLabel, action: connect)
                    .buttonStyle(PillButtonStyle())
                    .disabled(connecting)
                    .keyboardShortcut(.defaultAction)
            }
            .padding(16)
            .overlay(alignment: .top) { RemoraDesign.line.frame(height: 1) }
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
                    .foregroundStyle(RemoraDesign.onAccent)
                    .frame(width: 30, height: 30)
                    .background(RemoraDesign.accent, in: Circle())
                Text("Connect \(manifest.name)").font(RemoraDesign.font(17, .semibold))
            }
            Text(L(manifest.summary)).font(RemoraDesign.font(12.5)).foregroundStyle(RemoraDesign.muted)
        }
    }

    /// A labelled field. The label above it is its spoken name and the help below its hint: built from a
    /// placeholder, the field would be named "github.com", or nothing at all for a token.
    private func field(_ label: String, help: String?, @ViewBuilder input: () -> some View) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(L(label)).font(RemoraDesign.font(12, .semibold)).accessibilityHidden(true)
            input().textFieldStyle(.roundedBorder)
                .accessibilityLabel(L(label))
                .accessibilityHint(help.map { L($0) } ?? "")
            if let help {
                Text(L(help)).font(RemoraDesign.font(11)).foregroundStyle(RemoraDesign.muted).accessibilityHidden(true)
            }
        }
    }

    /// An on/off option: a switch with its help below, "true" when on.
    private func toggle(_ field: ConfigField, locked: Bool, forced: String?) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Toggle(
                L(field.label),
                isOn: Binding(
                    get: { (locked ? forced : values[field.key]) == "true" },
                    set: { values[field.key] = $0 ? "true" : "" })
            )
            .font(RemoraDesign.font(12, .semibold))
            .disabled(locked)
            .help(locked ? L("Managed by your organization") : "")
            .accessibilityHint(field.help.map { L($0) } ?? "")
            if let help = field.help {
                Text(L(help)).font(RemoraDesign.font(11)).foregroundStyle(RemoraDesign.muted).accessibilityHidden(true)
            }
        }
    }

    private var config: PluginConfig { PluginConfig(accountID: UUID(), values: values) }

    private var connectLabel: String {
        if signingIn { return L("Waiting for the browser…") }
        if connecting { return L("Connecting…") }
        return manifest.signIn == nil ? L("Connect") : L("Sign in with %@", manifest.name)
    }

    /// The AI server the organization set, for a plugin whose server the user chooses.
    private var forcedServer: String? {
        manifest.egress.serverField == nil ? nil : ManagedPolicy.aiServer()
    }

    /// What the organization set for this field: its `AIServer`, or the field's `managedKey`.
    private func forcedValue(_ field: ConfigField) -> String? {
        if field.key == manifest.egress.serverField, let forcedServer { return forcedServer }
        return field.managedKey.flatMap { ManagedPolicy.setting($0) }
    }

    private func binding(_ field: ConfigField) -> Binding<String> {
        Binding(get: { values[field.key] ?? "" }, set: { values[field.key] = $0 })
    }

    private func connect() {
        let secretKeys = Set(manifest.fields.filter(\.isSecret).map(\.key))
        var settings = values.filter { !secretKeys.contains($0.key) }
        for field in manifest.fields { if let forced = forcedValue(field) { settings[field.key] = forced } }
        var secrets = values.filter { secretKeys.contains($0.key) }
        connecting = true
        error = nil
        connection = Task {
            defer { connecting = false }
            do {
                if manifest.signIn != nil {
                    signingIn = true
                    defer { signingIn = false }
                    secrets.merge(try await model.signIn(pluginID: manifest.id, settings: settings)) { _, new in new }
                }
                try await model.connect(pluginID: manifest.id, name: name, settings: settings, secrets: secrets)
                dismiss()
            } catch {
                self.error = error.localizedDescription
                // Shown under the form, and said aloud: VoiceOver users pressed Connect and would hear nothing.
                AccessibilityNotification.Announcement(error.localizedDescription).post()
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
                        .font(RemoraDesign.font(11, .bold))
                        .foregroundStyle(RemoraDesign.onAccent)
                        .frame(width: 20, height: 20)
                        .background(RemoraDesign.accent, in: Circle())
                    Text(L(step))
                        .font(RemoraDesign.font(12.5))
                        .foregroundStyle(RemoraDesign.ink)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
            if let url = manifest.setupURL?(config) {
                Link(destination: url) {
                    Label(L(manifest.setupLabel), systemImage: "arrow.up.right")
                        .font(RemoraDesign.font(13, .semibold))
                        .foregroundStyle(RemoraDesign.onDark)
                        .padding(.horizontal, 14)
                        .padding(.vertical, 7)
                        .background(RemoraDesign.dark, in: Capsule())
                }
                .buttonStyle(.plain)
                .padding(.leading, 30)
            }
        }
        .padding(14)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(
            RemoraDesign.card, in: RoundedRectangle(cornerRadius: RemoraDesign.radiusMedium, style: .continuous))
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

    /// The same choices as on Windows; the managed key allows any value from 1 to 10000.
    static let dailyLimits = [0, 20, 50, 100, 200, 500]

    private func limitLabel(_ limit: Int) -> String {
        limit == 0 ? L("No limit") : L("%lld requests", limit)
    }

    /// Requests and tokens, tokens only as the providers reported them.
    private func usageLabel(_ day: AssistantUsage.Day) -> String {
        guard day.requests > 0 else { return L("Nothing yet") }
        return L("%lld request", plural: "%lld requests", day.requests) + " · " + L("%@ tokens", day.tokens.formatted())
    }

    var body: some View {
        @Bindable var model = model
        Form {
            Section("Refresh") {
                if ManagedPolicy.isForced(ManagedPolicy.refreshMinutesKey) {
                    LabeledContent(L("Check every"), value: L("%d min", model.effectivePreferences.refreshMinutes))
                        .help(L("Managed by your organization"))
                } else {
                    Picker("Check every", selection: $model.preferences.refreshMinutes) {
                        // The same choices as on Windows; the managed key allows any value from 1 to 60.
                        ForEach([1, 2, 5, 10, 15, 30, 60], id: \.self) { minutes in
                            Text(minutes < 60 ? L("%d min", minutes) : L("%d h", minutes / 60)).tag(minutes)
                        }
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
                    .font(RemoraDesign.font(11.5))
                    .foregroundStyle(RemoraDesign.muted)
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
                    Text(L("Couldn’t change it: %@", loginError)).font(RemoraDesign.font(11.5)).foregroundStyle(
                        RemoraDesign.danger)
                }
            }
            Section("Menu bar") {
                Picker("Show count of", selection: $model.preferences.menuBarCount) {
                    ForEach(Preferences.MenuBarCount.allCases) { Text($0.label).tag($0) }
                }
                Toggle("One counter per source", isOn: $model.preferences.countPerSource)
                    .disabled(model.preferences.menuBarCount == .hidden || model.preferences.menuBarCount == .dot)
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
                // They wait for the next set time, then come grouped.
                Toggle("Group them at set times", isOn: $model.preferences.batchNotifications)
                if model.preferences.batchNotifications {
                    TextField(
                        "At", text: $model.preferences.notificationTimes, prompt: Text(NotificationBatch.defaultTimes)
                    )
                    .help(L("Reminders and snoozes coming back don’t wait."))
                    Text(L("Reminders and snoozes coming back don’t wait."))
                        .font(RemoraDesign.font(11)).foregroundStyle(RemoraDesign.muted)
                }
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
                .font(RemoraDesign.font(11.5))
                .foregroundStyle(RemoraDesign.muted)
            }
            Section("Assistant") {
                Toggle("Whole-inbox brief", isOn: $model.preferences.wholeInboxBrief)
                Text("Off: no Brief button and no brief is written. The ✦ summary on each bundle stays available.")
                    .font(RemoraDesign.font(11.5))
                    .foregroundStyle(RemoraDesign.muted)
                Picker("Reuse a brief or summary for", selection: $model.preferences.briefCacheMinutes) {
                    Text("Always write a new one").tag(0)
                    ForEach([5, 15, 30, 60, 120], id: \.self) {
                        Text($0 < 60 ? L("%d min", $0) : L("%d h", $0 / 60)).tag($0)
                    }
                }
                Text("While fresh, Remora shows it again instead of asking the assistant.")
                    .font(RemoraDesign.font(11.5))
                    .foregroundStyle(RemoraDesign.muted)
                // What the assistant used, and an optional cap on it.
                if ManagedPolicy.isForced(ManagedPolicy.aiDailyLimitKey) {
                    LabeledContent(L("Daily limit"), value: limitLabel(model.effectivePreferences.assistantDailyLimit))
                        .help(L("Managed by your organization"))
                } else {
                    Picker("Daily limit", selection: $model.preferences.assistantDailyLimit) {
                        ForEach(Self.dailyLimits, id: \.self) { Text(limitLabel($0)).tag($0) }
                    }
                }
                Text(
                    "Each brief, summary or triage is one request. Past the limit, the assistant waits until tomorrow."
                )
                .font(RemoraDesign.font(11.5))
                .foregroundStyle(RemoraDesign.muted)
                LabeledContent(L("Used today"), value: usageLabel(model.assistant.usage.today()))
                LabeledContent(L("This month"), value: usageLabel(model.assistant.usage.thisMonth()))
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
                    .font(RemoraDesign.font(11.5))
                    .foregroundStyle(RemoraDesign.muted)
            }
            Section("Snooze") {
                Toggle("Bring snoozed items back early on new activity", isOn: $model.preferences.wakeOnActivity)
            }
            Section("Bots and agents") {
                Toggle("Group bots and agents apart", isOn: $model.preferences.groupBots)
                Text("Their items go last in My turn, still counted, so people come first.")
                    .font(RemoraDesign.font(11.5))
                    .foregroundStyle(RemoraDesign.muted)
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
                // Opens the inbox from any app.
                Picker("Open Remora from anywhere", selection: $model.preferences.openShortcut) {
                    ForEach(Preferences.OpenShortcut.allCases) { Text($0.label).tag($0) }
                }
                if model.openShortcutTaken {
                    Text(L("Another app already uses this shortcut: pick another one."))
                        .font(RemoraDesign.font(11)).foregroundStyle(RemoraDesign.danger)
                }
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
            Image(systemName: "exclamationmark.triangle.fill").foregroundStyle(
                RemoraDesign.color(for: .warning).foreground)
            Text(text).font(RemoraDesign.font(11.5)).foregroundStyle(RemoraDesign.inkSoft).fixedSize(
                horizontal: false, vertical: true)
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
                    .font(RemoraDesign.font(11.5)).foregroundStyle(RemoraDesign.muted)
            } else if !model.privacy.updatesAllowed {
                Text(L("Your organization installs new versions of Remora.")).font(RemoraDesign.font(11.5))
                    .foregroundStyle(
                        RemoraDesign.muted
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
            if updater.isSupported, model.privacy.updatesAllowed {
                Text(
                    "Once a day, Remora asks GitHub whether a new version is out. Nothing about you or your inbox is sent. A new version is installed only when you choose, and only if it is signed by Remora’s release key."
                )
                .font(RemoraDesign.font(11.5))
                .foregroundStyle(RemoraDesign.muted)
            }
        }
    }

    @ViewBuilder private func status(_ updater: Updater) -> some View {
        switch updater.state {
        case .available(let offer):
            HStack {
                Text(L("Remora %@ is available.", offer.version.description)).font(RemoraDesign.font(12, .semibold))
                if let notes = offer.notes.flatMap(URL.init(string:)), UpdateHosts.allows(notes) {
                    Link(L("What’s new"), destination: notes).font(RemoraDesign.font(11.5))
                }
                Spacer()
                Button(L("Install and relaunch")) { Task { await updater.install() } }
            }
        case .installing:
            HStack(spacing: 8) {
                ProgressView().controlSize(.small)
                Text(L("Downloading and checking the new version…")).font(RemoraDesign.font(11.5)).foregroundStyle(
                    RemoraDesign.inkSoft)
            }
        default:
            HStack {
                switch updater.state {
                case .checking:
                    ProgressView().controlSize(.small)
                    Text(L("Checking…")).font(RemoraDesign.font(11.5)).foregroundStyle(RemoraDesign.muted)
                case .upToDate:
                    Text(L("Remora is up to date.")).font(RemoraDesign.font(11.5)).foregroundStyle(RemoraDesign.muted)
                case .failed(let message):
                    Text(message).font(RemoraDesign.font(11.5)).foregroundStyle(RemoraDesign.danger).fixedSize(
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

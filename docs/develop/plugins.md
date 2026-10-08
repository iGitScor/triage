---
title: Writing a plugin
description: Add a tool to Remora. The SourcePlugin contract, the manifest that drives the settings form and declares egress, mapping to InboxItems, fixture tests with StubHTTP, translations and the Windows port.
---

# Writing a plugin

Every integration is a plugin in `macos/Sources/RemoraPlugins/<Name>/`. Plugins depend on `RemoraCore` only. A
plugin is two things: a **manifest** (what it is, what the user fills in, where its data goes) and a **fetch**
(turn the tool’s API into `InboxItem`s). The settings form, the logo in the menu bar, the Privacy pane and the
compliance gate all come from the manifest.

## The contract

```swift
public protocol SourcePlugin: Sendable {
    static var manifest: PluginManifest { get }
    init(config: PluginConfig, http: HTTPClient) throws
    func fetch() async throws -> SourceSnapshot   // identity + items
}
```

`http` is a `GuardedHTTPClient` limited to the hosts the manifest declares, plus the account’s own host for
self-hosted tools. A request anywhere else throws “Blocked: … is not an allowed destination”.

## A minimal plugin

```swift
import Foundation
import RemoraCore

/// Open tickets assigned to you in Acme Desk.
public struct AcmeDeskPlugin: SourcePlugin {
    public static let manifest = PluginManifest(
        id: "acmedesk",                              // stable: stored in accounts and MDM policies
        name: "Acme Desk",
        symbol: "lifepreserver",                     // an SF Symbol, used where there's no logo
        summary: "Tickets assigned to you.",
        fields: [.token("API key", help: "Acme Desk → Profile → API keys.")],
        setupSteps: ["Click “Create a key”, name it Remora (read only), and paste it below."],
        setupLabel: "Create a key",
        setupURL: { _ in URL(string: "https://desk.acme.example/profile/keys") },
        egress: Egress(hosts: ["api.desk.acme.example"], description: "Reads tickets assigned to you."),
        logo: "acmedesk"                             // Resources/Logos/acmedesk.svg
    )

    private let accountID: UUID
    private let token: String
    private let http: HTTPClient

    public init(config: PluginConfig, http: HTTPClient) throws {
        accountID = config.accountID
        token = try config.required("token")         // fail early on a missing field
        self.http = http
    }

    public func fetch() async throws -> SourceSnapshot {
        var request = URLRequest(url: URL(string: "https://api.desk.acme.example/v1/me/tickets")!)
        request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        let (data, _) = try await http.send(request)
        let tickets = try JSONDecoder().decode(Tickets.self, from: data)
        return SourceSnapshot(identity: tickets.me, items: tickets.open.map { $0.item(accountID: accountID) })
    }
}
```

Then:

1. Add it to `PluginRegistry.sources` (`Sources/RemoraPlugins/PluginRegistry.swift`).
2. Add its logo to `Resources/Logos/` as a single-colour SVG ([Simple Icons](https://simpleicons.org) has most
   tools, under CC0), and credit it in `Resources/LICENCES.txt` if it comes from elsewhere.
3. Add tests (below), and run `make test`.

## Mapping to inbox items

- **`id`** must be stable across refreshes: `"\(accountID.uuidString)/<remote id>"`. Done, Pin and Snooze are keyed
  on it.
- **`bundle`**: pick the *kind* the tool reported (`.reviews`, `.mentions`, `.directMessages`, `.authored`,
  `.tasks`). `VerbClassifier` then moves it to a verb (*To reply*, *To fix*…). Add a bundle to `InboxBundle.swift`
  only for a genuinely new kind of ask.
- **`date`** is the last meaningful activity. It drives sorting, and a change brings Done items back.
- **`needsAction`**: someone is waiting on the user. It drives arrival notifications and the menu bar count.
- **`priority`** and **`due`** when the tool has them: they order the bundle.
- **`badges`**: status chips with a `Tone`. A badge with `notify` set announces itself the first time it appears
  (*Approved*, *Checks failed*). Keep badge IDs stable (`approved`, `checks.failing`, `priority.urgent`…).
- **`changes`** (file paths and line counts) turn on review prep. Never store code.
- **`url`** opens in the browser; **`appURL`** (`slack://…`, `linear://…`) opens the desktop app when it’s
  installed.

## Manifest options

| Field | What it does |
|---|---|
| `fields` | The connect form. `.token(…)` and fields with `isSecret: true` go to the Keychain; the others to `accounts.json`. `.host(default)` for self-hosted tools. |
| `setupSteps`, `setupLabel`, `setupURL` | The numbered guide and the button at the top of the form. `setupURL` gets the config, so it can point at the user’s own host. |
| `egress` | Hosts and a one-sentence description, shown in Settings → Privacy and enforced. `externalAI: true` for plugins that send inbox content to an AI provider: they stay off until it’s allowed. |
| `isComingSoon` | Listed with a *Soon* badge, not connectable. |
| `logo` | The SVG in `Resources/Logos`, shown in the menu bar counters. |

A plugin built on a local program uses `CommandRunner` (see `ClaudeCodePlugin`). Assistant plugins implement
`AssistantPlugin` (`brief`, `digest`, `triage`) and go in `PluginRegistry.assistants`.

## Tests

Plugins are tested against JSON fixtures, never the network, with `StubHTTP` (`Tests/RemoraPluginsTests`):

```swift
@Test func ticketsAssignedToMe() async throws {
    let http = StubHTTP(routes: ["/v1/me/tickets": #"{"me": "alice", "open": [{"id": "T-1", "title": "Printer on fire"}]}"#])
    let plugin = try AcmeDeskPlugin(config: config(["token": "k"]), http: http)
    let snapshot = try await plugin.fetch()
    #expect(snapshot.identity == "alice")
    #expect(snapshot.items.map(\.title) == ["Printer on fire"])
}
```

`ComplianceTests` checks every manifest in the registry declares its egress, so a new plugin without one fails CI.
Use made-up people (alice, bob…) and domains (`acme.example`) in fixtures.

## Translations

User-facing strings go through `L("…")`. Add the French for each new string to
`macos/scripts/translations_fr.py`; `python3 scripts/make-strings.py` regenerates `Localizable.strings` and lists
any string still missing a translation.

## The Windows port

The Windows app has the same plugins in Rust (`windows/crates/remora_plugins`), with the same queries, mappings
and fixtures: port the plugin there too, or open an issue. See [Remora for Windows](./windows).

# Plugins

Every integration is a plugin in `Sources/RemoraPlugins/<Name>/`. Plugins depend on `RemoraCore` only.

Setting up each tool as a user: [docs/SOURCES.md](../../docs/SOURCES.md).

## Writing a plugin

1. Create `Sources/RemoraPlugins/<Name>/<Name>Plugin.swift`:

```swift
public struct LinearPlugin: SourcePlugin {
    public static let manifest = PluginManifest(
        id: "linear",
        name: "Linear",
        symbol: "square.stack",                 // an SF Symbol
        summary: "Issues assigned to you.",
        fields: [.token("API key")]             // drives the settings form
    )

    private let config: PluginConfig
    private let http: HTTPClient

    public init(config: PluginConfig, http: HTTPClient) throws {
        _ = try config.required("token")        // fail early on missing fields
        self.config = config
        self.http = http
    }

    public func fetch() async throws -> SourceSnapshot {
        // Call the API through `http`, map results to InboxItems.
        SourceSnapshot(identity: "me", items: [])
    }
}
```

2. Add it to `PluginRegistry.sources`.
3. Add a test with `StubHTTP` (see `Tests/RemoraPluginsTests`).

Guidelines:

- **IDs** must be stable: `"\(config.accountID.uuidString)/<remote id>"`.
- **Bundle**: pick an existing `InboxBundle`; add one to `InboxBundle.swift` only for a genuinely new kind of ask.
- **date** is the last meaningful activity: it drives sorting and resurfacing Done items.
- **needsAction** = someone is waiting on the user. It controls arrival notifications and the menu bar count.
- **Badges** with `notify` announce status changes. Keep badge IDs stable (`approved`, `checks.failing`…).
- Fields with `isSecret: true` go to the Keychain; others are saved in `accounts.json`.
- `isComingSoon: true` lists a plugin with a *Soon* badge without letting users connect it.
- `setupSteps`, `setupLabel` and `setupURL` render the numbered guide and button at the top of the connect form.

Plugins built on a local program use `CommandRunner` (see `ClaudeCodePlugin`).
Assistant plugins implement `AssistantPlugin` (`brief(_:now:)`, `digest(_:topic:now:)` and `triage(_:now:)`) and go in `PluginRegistry.assistants`.

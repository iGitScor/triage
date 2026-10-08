# Remora for Windows

Status: **in progress**. A [Tauri](https://tauri.app) app (Rust core + web UI) for colleagues on Windows, following
the macOS app's design and rules. Product overview: [../README.md](../README.md).

| Crate | What | Status |
|---|---|---|
| `crates/remora_core` | Domain and rules, ported from the macOS `RemoraCore` with its tests: verbs, My turn / Waiting, prioritizer, Done and snooze rules, notifications, snooze clock, compliance policy | Done |
| `crates/remora_plugins` | GitHub, GitLab, Slack and Linear sources with their fixture tests; HTTP client limited to each plugin's declared hosts | Done |
| `src-tauri`, `src` | Tray app, storage (Credential Manager, `%LOCALAPPDATA%\Remora`), Svelte UI | Next |

Strings in the core are English and act as translation keys for the UI (the same idea as `L()` on macOS).

## Develop

```sh
cargo test --workspace     # core tests (also run by .github/workflows/windows.yml on windows-latest)
```

Compliance follows the same model as macOS: see [../docs/COMPLIANCE.md](../docs/COMPLIANCE.md).

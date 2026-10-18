# Contributing to Remora

Thanks for helping. This page gets you from a fresh clone to a change ready for review. The details live in the
[developer docs](https://triage.iscor.me/docs/develop/): [architecture](docs/develop/index.md),
[writing a plugin](docs/develop/plugins.md), [building and releasing](docs/develop/building.md) and
[Remora for Windows](docs/develop/windows.md).

## The repository

| Folder | What | Needs |
|---|---|---|
| [`macos/`](macos) | The menu bar app: `RemoraCore` (rules), `RemoraPlugins` (sources, assistant), `Remora` (the app) | macOS 14+, Xcode 16 |
| [`desktop/`](desktop) | The same rules and sources in Rust, and a Tauri 2 tray app with a Svelte interface | Rust, Node 22 |
| [`docs/`](docs) | The user guide, IT and compliance, and developer docs, in English and French (VitePress) | Node 22 |
| [`site/`](site) | The marketing pages, written by `site/build.py`, served with the docs by Cloudflare | Python 3 |

Some files feed more than one folder: the French dictionary (`macos/scripts/translations_fr.py`) serves both apps,
and the tool logos (`macos/Resources/Logos`) are shared with the Windows app. The design tokens come from the
Estuary design system (`@iscor/design`): see `scripts/tokens-check.py`.

## First run

```sh
make setup        # Node packages (tooling, Windows interface, docs) and the pre-commit hook
make test         # every test suite: Swift, then the Rust crates
make mac-demo     # the Mac app with sample data in a window, nothing saved
make desktop-demo     # the Windows app with sample data (runs on a Mac too)
```

`make` alone lists every target. Each one calls the folder's own tooling, so the commands in the folder READMEs
keep working; on Windows without `make`, run those directly.

## Before you open a pull request

- **`make check`** runs what CI runs: formatting, the tests (compiler warnings fail, on both apps), the Windows
  type-check and build, clippy, the translation and policy-file checks, and the docs build with its CSP and links.
  Only the accessibility pass is apart (`make a11y`).
- **Formatting is automatic.** The pre-commit hook formats what you commit: Biome for TypeScript, Svelte, CSS and
  JSON, `swift format`, `rustfmt` and ruff (through [uv](https://docs.astral.sh/uv/)). `make format` formats
  everything. Lines go up to 120 columns; the configs are `biome.json`, `macos/.swift-format`,
  `desktop/rustfmt.toml` and `ruff.toml`, with `.editorconfig` for editors.
- **Both apps follow the same rules.** A change to a rule, a verb or a source in `RemoraCore` / `RemoraPlugins`
  usually needs its Rust twin in `desktop/crates` (or a line in [the Windows page](docs/develop/windows.md) saying
  it isn't there yet), with a test on each side using the same fixture. On Windows, the interface's types come from
  the Rust structs: `make desktop-bindings` after changing what a command sends.
- **Strings are English in the code.** Add the French to `macos/scripts/translations_fr.py`, then `make i18n`
  regenerates both apps' files. CI fails when a string the interface shows has no French: `L(…)` and SwiftUI texts
  on the Mac; `t(…)` calls (and objects marked `// t-keys`) and the Rust messages that reach the interface on
  Windows, where `{name}` reads as `%@`. On Windows, an error or remark with values reaches the interface as a
  `Message` (a code, its values, the English): give it a code in `PluginError::message()` (or `Message::new`) and its
  wording in `MESSAGES` in `i18n.ts`; a test fails when a code has none. Fixed sentences go as they are.
- **User-visible changes are documented**: the guide in `docs/guide` and its French twin in `docs/fr/guide`, the
  website copy in `site/copy/en.json` and `fr.json` (`make site`) when a feature is advertised there, and a line under *Unreleased*
  in [`CHANGELOG.md`](CHANGELOG.md). Keep it short.
- **Privacy rules hold**: a plugin declares every host it reaches (`egress`, enforced by the guarded HTTP client
  and its tests); tokens go only to the Keychain or the Credential Manager; no telemetry, analytics or crash
  reporting; nothing is sent to an AI unless external AI is allowed. A new or changed flow (a host, an API call, a
  field the assistant gets) updates both lists of flows, in English and French: [Data flows](docs/admin/data-flows.md)
  and the website's privacy page (`privacy` in `site/copy/en.json` and `fr.json`).

- **Dependencies**: a new Rust crate needs a licence listed in `desktop/deny.toml` (`make deps-check`, with
  `cargo-deny`); Dependabot proposes updates every week.

Commit messages follow the history: `feat(macos): …`, `fix(desktop): …`, `docs: …`, `ci: …`, `i18n: …`.

## Releases and the website

Maintainers release with `make release V=x.y.z`: checks, then one commit (version, changelog, site pages), the
signed tag and the push ([building and releasing](docs/develop/building.md#releasing)). The release workflow
publishes, then deploys the website and docs.
`make deploy` does it from a maintainer's computer and needs a Cloudflare login; `make site-preview` shows them locally
first, `make a11y` checks their accessibility with axe, and `make links` that every internal link leads to a page.

## Security and licence

Report vulnerabilities privately, as [SECURITY.md](SECURITY.md) explains, not in issues. Bugs and ideas go through
the issue forms.

Remora is [GPL-3.0-or-later](LICENSE). By contributing, you agree that your contribution is released under the
same licence.

# Contributing to Remora

Thanks for helping. This page gets you from a fresh clone to a change ready for review. The details live in the
[developer docs](https://triage.iscor.me/docs/develop/): [architecture](docs/develop/index.md),
[writing a plugin](docs/develop/plugins.md), [building and releasing](docs/develop/building.md) and
[Remora for Windows](docs/develop/windows.md).

## The repository

| Folder | What | Needs |
|---|---|---|
| [`macos/`](macos) | The menu bar app: `RemoraCore` (rules), `RemoraPlugins` (sources, assistant), `Remora` (the app) | macOS 14+, Xcode 16 |
| [`windows/`](windows) | The same rules and sources in Rust, and a Tauri 2 tray app with a Svelte interface | Rust, Node 22 |
| [`docs/`](docs) | The user guide, IT and compliance, and developer docs, in English and French (VitePress) | Node 22 |
| [`site/`](site) | The marketing pages, written by `site/build.py`, served with the docs by Cloudflare | Python 3 |

Some files feed more than one folder: the French dictionary (`macos/scripts/translations_fr.py`) serves both apps,
and the tool logos (`macos/Resources/Logos`) and design tokens (`site/public/site/tokens.css`) are shared with the
Windows app.

## First run

```sh
make setup        # Node packages for the Windows interface and the docs
make test         # every test suite: Swift, then the Rust crates
make mac-demo     # the Mac app with sample data in a window, nothing saved
make win-demo     # the Windows app with sample data (runs on a Mac too)
```

`make` alone lists every target. Each one calls the folder's own tooling, so the commands in the folder READMEs
keep working; on Windows without `make`, run those directly.

## Before you open a pull request

- **`make check`** runs what CI runs: the tests, the Windows type-check and build, clippy with warnings as errors,
  and the translation check.
- **Both apps follow the same rules.** A change to a rule, a verb or a source in `RemoraCore` / `RemoraPlugins`
  usually needs its Rust twin in `windows/crates` (or a line in [the Windows page](docs/develop/windows.md) saying
  it isn't there yet), with a test on each side using the same fixture.
- **Strings are English in the code.** Add the French to `macos/scripts/translations_fr.py`, then `make i18n`
- **Privacy rules hold**: a plugin declares every host it reaches (`egress`, enforced by the guarded HTTP client
  and its tests); tokens go only to the Keychain or the Credential Manager; no telemetry, analytics or crash
  reporting; nothing is sent to an AI unless external AI is allowed.

Commit messages follow the history: `feat(macos): …`, `fix(windows): …`, `docs: …`, `ci: …`, `i18n: …`.

## Releases and the website

Maintainers release with `make version V=x.y.z`, a commit, then the tag `vx.y.z`
([building and releasing](docs/develop/building.md#releasing)); the release workflow refuses a tag that doesn't
match the source.
`make deploy` publishes the website and docs and needs a Cloudflare login; `make site-preview` shows them locally
first.

## Security and licence

Remora is [GPL-3.0-or-later](LICENSE). By contributing, you agree that your contribution is released under the
same licence.

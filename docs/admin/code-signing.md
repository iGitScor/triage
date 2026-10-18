---
title: Code signing policy
description: How Remora's releases are signed today and once signing is set up, who approves a signature, and the privacy commitment.
---

# Code signing policy

Free code signing on Windows provided by [SignPath.io](https://about.signpath.io), certificate by
[SignPath Foundation](https://signpath.org).

This page is the one place that says how releases are signed; the other pages link here. Each release's notes also
say how its own files were signed.

## What is signed

| Platform | Files | Today | Once set up |
|---|---|---|---|
| macOS | `Remora.app` in `Remora.dmg` | **Ad hoc**, not notarized. Gatekeeper asks the first time, and the Keychain asks once after each update, because macOS recognizes an ad hoc app by its exact build. | Remora's own certificate: macOS recognizes each update as the same app and the Keychain stops asking. Still not an Apple Developer ID, so Gatekeeper asks the first time until the app is notarized. |
| Windows | `Remora-Setup.exe`, `Remora.msi` and the `Remora.exe` they install | **Not signed.** SmartScreen warns the first time (*More info* → *Run anyway*). | SignPath, with the SignPath Foundation certificate. Windows shows *SignPath Foundation* as the publisher. The MSI needs its own SignPath artifact configuration (`msi`). |
| Linux | `Remora-x86_64.AppImage`, `Remora-amd64.deb` | **Not signed**, like most AppImages; check the SHA-256 and the build provenance below. | No change planned. |

To avoid the prompts today, an organization can build and sign its own copy with its Developer ID or code-signing
certificate: see [Deploying Remora](./deployment).

Only the [release workflow](https://github.com/iGitScor/triage/blob/main/.github/workflows/release.yml) signs, and
only for a version tag pushed to [the repository](https://github.com/iGitScor/triage). Nothing built elsewhere,
including on a developer's computer, is signed with these certificates. Each release also carries a signed build
provenance that ties every file to the commit it was built from (`gh attestation verify`, see
[Deploying Remora](./deployment)).

## Team and roles

| Role | Who |
|---|---|
| Committers and reviewers | [Repository maintainers](https://github.com/iGitScor/triage/graphs/contributors). Outside contributions are reviewed before they are merged. |
| Approvers | [@iGitScor](https://github.com/iGitScor). Each Windows signing request is approved by hand in SignPath. |

Every account in these roles uses multi-factor authentication on GitHub and SignPath.

## Privacy

This program will not transfer any information to other networked systems unless specifically requested by the user
or the person installing or operating it. Remora only talks to the tools you connect, and to an AI assistant if you
turn one on: see [Data flows](./data-flows).

## Reporting a problem

A file signed with these certificates that you didn't get from the
[releases page](https://github.com/iGitScor/triage/releases), or a signature that doesn't verify: report it
privately as [SECURITY.md](https://github.com/iGitScor/triage/blob/main/SECURITY.md) explains.

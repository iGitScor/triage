---
title: Code signing policy
description: How Remora's releases are signed. On Windows, by SignPath with the SignPath Foundation certificate; on macOS, with Remora's own certificate, not yet notarized. Who approves a signature, what is signed, and the privacy commitment.
---

# Code signing policy

Free code signing on Windows provided by [SignPath.io](https://about.signpath.io), certificate by
[SignPath Foundation](https://signpath.org).

This signing is being set up: until it is, each release's notes say whether its files are signed.

## What is signed

| Platform | Files | Signed by |
|---|---|---|
| Windows | `Remora-Setup.exe` and the `Remora.exe` it installs | SignPath, with the SignPath Foundation certificate. Windows shows *SignPath Foundation* as the publisher. |
| macOS | `Remora.app` in `Remora.dmg` | Remora's own certificate, so macOS recognizes each update as the same app and the Keychain stops asking. It isn't an Apple Developer ID: the app is not notarized yet, and Gatekeeper still asks the first time. |

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

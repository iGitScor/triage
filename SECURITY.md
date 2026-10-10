# Security

Remora handles tokens for your tools and the content of your inbox, so security reports are welcome and treated
first.

## Reporting a vulnerability

Report it privately through
[a GitHub security advisory](https://github.com/iGitScor/triage/security/advisories/new). Please don't open a public
issue or discussion for it.

Include what you can of:

- the platform (macOS or Windows) and the version (Settings → General → Version);
- what an attacker could do, and what they would need (a malicious server, a crafted message, local access…);
- the steps to reproduce it.

Never send real tokens, API keys or someone's messages: a made-up example is enough.

## What happens next

We aim to acknowledge a report within a few working days, then keep you informed until it is fixed. A fix ships in
a new release; the advisory is published with that release, with credit to you if you want it. Please keep the
details private until then.

## Supported versions

Only the [latest release](https://github.com/iGitScor/triage/releases/latest) receives security fixes. Each release
publishes the SHA-256 of `Remora.dmg` and `Remora-Setup.exe`, their SBOM (CycloneDX) and a signed build
provenance: check them before installing (`gh attestation verify Remora.dmg --repo iGitScor/triage`).

## Scope

In scope: the macOS app, the Windows app, and the website and documentation at
[triage.iscor.me](https://triage.iscor.me). The tools Remora connects to (GitHub, GitLab, Slack, Linear, Notion) and
Anthropic's services have their own programs.

How Remora is meant to protect data, which a report can measure it against:
[security overview](https://triage.iscor.me/docs/admin/security),
[data flows](https://triage.iscor.me/docs/admin/data-flows), [code signing policy](https://triage.iscor.me/docs/admin/code-signing) and
[managing the policy](https://triage.iscor.me/docs/admin/mdm).

---
title: For IT and compliance
description: What a security or compliance review of Remora needs. No server, account or telemetry, enforced destinations, AI off by default, a lockable policy.
---

# For IT and compliance

Remora is a menu bar app on macOS, and a tray app on Windows (in preview), that reads your people’s work tools (GitHub,
GitLab, Slack, Linear, Jira, Notion, Microsoft 365) and shows what needs them. It is built for organizations with strict data rules: **no
data leaves the computer except to the tools the user, or the organization, allows.**

## In short

| Question                                       | Answer                                                                                                                                                                                   |
| ---------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Is there a Remora server or cloud?             | No. The app talks directly to the tools; nothing goes through a third party.                                                                                                             |
| Accounts, telemetry, analytics, crash reports? | None.                                                                                                                                                                                    |
| What does it send to the tools?                | Each user’s own token and read requests. It never writes to the tools.                                                                                                                   |
| Where is data stored?                          | On the computer only: a few JSON files in the user’s own folder, tokens in the login Keychain (macOS) or the Credential Manager (Windows).                                               |
| AI?                                            | On-device only by default (Apple’s NaturalLanguage framework on macOS, keyword rules on Windows). The assistant (Claude or an OpenAI-compatible server) is opt-in, and can be forbidden. |
| Can IT lock it down?                           | Yes: allowed tools, external AI, the assistant’s servers and avatars, through a configuration profile (macOS) or Group Policy and Intune (Windows).                                      |
| Accessibility?                                 | Aims at WCAG 2.1 AA, partially conformant today: VoiceOver, Narrator and the keyboard, with the known gaps listed in the [accessibility statement](./accessibility).                     |
| Source code?                                   | Public, GPL-3.0-or-later: [github.com/iGitScor/triage](https://github.com/iGitScor/triage).                                                                                              |

## How it is enforced

```mermaid
flowchart LR
  subgraph Computer["The user's computer"]
    UI["Remora"] --> Gate["Policy gate"]
    Policy["Managed policy<br/>MDM / registry overrides<br/>user settings"] --> Gate
    Gate --> Client["Guarded HTTP client<br/>Plugin-declared hosts only"]
    Store[("Local files<br/>Credentials in Keychain<br/>or Credential Manager")]
    UI --- Store
  end

  Client --> Tools["Allowed integrations<br/>Approved tools only"]
  Client -.->|"If policy permits"| AI["AI provider<br/>Anthropic or an<br/>OpenAI-compatible server"]
  Client --x|"Any other host"| Blocked["Blocked"]

  %% Colours in the docs theme (custom.css), light and dark.
  classDef neutral stroke-width:1px;
  classDef security stroke-width:1.5px;
  classDef blocked stroke-width:1.5px;
  classDef zone stroke-width:1px;

  class UI,Policy,Store,Tools,AI neutral;
  class Gate,Client security;
  class Blocked blocked;
  class Computer zone;
```

1. **Declared destinations.** Every plugin declares, in its manifest, the hosts it may reach and what it sends. A
   test fails if a plugin doesn’t.
2. **Policy gate.** The app builds plugins in one place. It refuses a plugin the policy doesn’t allow, and gives the
   others an HTTP client limited to their own hosts (plus the account’s host for self-hosted tools). Any other host
   fails with “Blocked: … is not an allowed destination”, look-alike domains included, and that is tested too.
3. **External AI off by default.** An assistant that sends content off the computer is refused until external AI is
   allowed; a server on this computer follows `AllowLocalAI`.
4. **Visible to the user.** Settings → Privacy lists each connected account’s destinations, and erases all local
   data in one click.

## Next

- [Security overview](./security): the answers to a vendor security questionnaire, on one page.
- [Data flows](./data-flows): every flow, what is sent, and what stays on the computer.
- [Managing the policy with MDM](./mdm): the keys, a ready-to-use configuration profile, and the Windows registry values.
- [Deploying Remora](./deployment): the DMG and the Windows installer, signing, and updates.

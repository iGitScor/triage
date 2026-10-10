---
title: Managing the policy with MDM
description: Lock which tools Remora may connect, whether external AI is allowed, and whether avatars load, with a configuration profile (Jamf, Kandji, Intune, Mosyle, Apple Business Essentials). Keys and a complete profile.
---

# Managing the policy with MDM

The organization can lock Remora’s privacy policy with a configuration profile for the preference domain
**`fr.igitscor.remora`** on Macs, and with [registry values](#windows-group-policy-or-intune) on Windows. Any MDM that deploys custom settings works: Jamf Pro, Kandji, Microsoft Intune, Mosyle,
Apple Business Essentials…

When at least one key is set, Settings → Privacy shows **Managed by your organization** and the user can’t change
the policy.

## Keys

| Key | Type | Meaning | Default when not managed |
|---|---|---|---|
| `AllowedPlugins` | Array of strings | The tools that may be connected and refreshed. Any other is refused with “Not allowed by your privacy policy.” | All |
| `AllowExternalAI` | Boolean | Allow Claude plugins to send inbox content to Anthropic | `false` |
| `AllowRemoteImages` | Boolean | Load avatars, from the hosts of allowed tools only | `true` |
| `AIExcludedSources` | Array of strings | Tools whose items never reach the assistant (brief, summaries, triage); `reminders` for the user’s own reminders. Added to the user’s own choice and locked in Settings | None |
| `AllowedAIModels` | Array of strings | The Claude models the assistant may use. A model that isn’t listed (or the default, when it isn’t) is replaced by the first one | Any |
| `HiddenContentSources` | Array of strings | Tools whose notifications say where something happened, not what was written; `reminders` for the user’s own reminders. Added to the user’s own choice and locked in Settings | None |
| `RefreshMinutes` | Integer | How often tools are checked, from 1 to 60 minutes (another value is ignored). Locked in Settings | 5 |
| `OpenInApps` | Boolean | Open Slack and Linear items in their desktop app when installed; `false` always opens the web page | `true` |
| `ClaudeCodePath` | String | The `claude` Remora runs, instead of the one found or entered in the account. It must still be named `claude`, belong to the user or root, and answer as Claude Code | Found automatically |
| `AutomaticUpdates` | Boolean | `true`: check GitHub for a new version once a day, locked on. `false`: no update check at all, not even *Check now*, for fleets you redeploy yourself | Off; the user decides |

The last five are settings, not privacy rules: they don’t make Settings → Privacy read-only.

Plugin IDs: `github`, `gitlab`, `slack`, `linear`, `notion`, `claude-code` (Claude Code), `claude` (Claude API).
To choose between Claude Code and the Claude API, list only one of them in `AllowedPlugins`.

For example, Claude allowed for code reviews and tasks, never for Slack, and only with Sonnet and Haiku:

```xml
<key>AllowExternalAI</key><true/>
<key>AIExcludedSources</key>
<array>
    <string>slack</string>
</array>
<key>AllowedAIModels</key>
<array>
    <string>claude-sonnet-5-5</string>
    <string>claude-haiku-5-5</string>
</array>
```

These two keys are read by the macOS app; the Windows app has no assistant yet.

An account that is already connected to a tool the policy no longer allows stops refreshing at the next sync.

## A complete profile

This profile allows GitHub, GitLab and Linear, forbids external AI, and keeps avatars. Replace the two UUIDs with
your own (`uuidgen` in Terminal) and the identifiers with your organization’s.

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>PayloadType</key><string>Configuration</string>
    <key>PayloadVersion</key><integer>1</integer>
    <key>PayloadScope</key><string>System</string>
    <key>PayloadIdentifier</key><string>com.example.remora</string>
    <key>PayloadUUID</key><string>2F1A7D7E-0000-4000-8000-000000000001</string>
    <key>PayloadDisplayName</key><string>Remora privacy policy</string>
    <key>PayloadContent</key>
    <array>
        <dict>
            <key>PayloadType</key><string>fr.igitscor.remora</string>
            <key>PayloadVersion</key><integer>1</integer>
            <key>PayloadIdentifier</key><string>com.example.remora.policy</string>
            <key>PayloadUUID</key><string>2F1A7D7E-0000-4000-8000-000000000002</string>
            <key>PayloadDisplayName</key><string>Remora</string>
            <key>AllowedPlugins</key>
            <array>
                <string>github</string>
                <string>gitlab</string>
                <string>linear</string>
            </array>
            <key>AllowExternalAI</key><false/>
            <key>AllowRemoteImages</key><true/>
        </dict>
    </array>
</dict>
</plist>
```

In MDMs that take the settings only (an “Application & Custom Settings” payload in Jamf, “Custom Configuration” in
Kandji, “Preference file” in Intune), give `fr.igitscor.remora` as the domain and the keys as the
property list.

## Windows: Group Policy or Intune

The Windows app reads the same three keys, and `AutomaticUpdates`, from **`HKEY_LOCAL_MACHINE\SOFTWARE\Policies\Remora`**,
then from `HKEY_CURRENT_USER\SOFTWARE\Policies\Remora` for user-scoped policies (a value set in HKLM wins):

| Value | Type | Meaning |
|---|---|---|
| `AllowedPlugins` | `REG_MULTI_SZ` | The allowed tools, one ID per line |
| `AllowExternalAI` | `REG_DWORD` | `1` to allow external AI, `0` to forbid it |
| `AllowRemoteImages` | `REG_DWORD` | `1` to load avatars from allowed tools, `0` not to |
| `AutomaticUpdates` | `REG_DWORD` | `1` to check for updates daily (locked on), `0` to turn updating off |

Deploy them with a Group Policy Preference (Computer Configuration → Preferences → Windows Settings → Registry), an
Intune remediation or configuration script, or a `.reg` file. The same policy as the profile above, in PowerShell
(as administrator):

```powershell
$key = 'HKLM:\SOFTWARE\Policies\Remora'
New-Item -Path $key -Force | Out-Null
New-ItemProperty -Path $key -Name AllowedPlugins -PropertyType MultiString -Value 'github','gitlab','linear' -Force
New-ItemProperty -Path $key -Name AllowExternalAI -PropertyType DWord -Value 0 -Force
New-ItemProperty -Path $key -Name AllowRemoteImages -PropertyType DWord -Value 1 -Force
```

Check it with `reg query HKLM\SOFTWARE\Policies\Remora`. Remora reads the policy at launch and again at every refresh:
a change applies within the refresh interval, without a restart, and users see *Managed by your organization* in
Settings → Privacy. Users can’t write under `HKLM\SOFTWARE\Policies`, so they can’t loosen what it sets.

A typing mistake never opens things up. `AllowedPlugins` also accepts a `REG_SZ` list (`github, slack`), and the
switches a `REG_SZ` `0` or `1`. Any other type, or a value Remora can’t read, is applied as strictly as possible (no
tool, no external AI, no avatars, no updates), and Settings → Privacy names the value to fix.

## Checking it on a Mac

```sh
# The profile is installed:
sudo profiles show -type configuration | grep -A3 fr.igitscor.remora
# The managed values Remora reads:
defaults read /Library/Managed\ Preferences/fr.igitscor.remora
```

Then open Remora → Settings → Privacy: it shows *Managed by your organization*, the allowed tools, and the AI and
avatar switches locked.

::: info Why `defaults write` isn’t enough
Remora only applies values that macOS reports as managed (forced) by a profile. A `defaults write` by the user is
ignored for these three keys, so a user can’t loosen the policy, and a test can’t accidentally lock it.
:::

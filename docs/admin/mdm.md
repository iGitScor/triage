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

Plugin IDs: `github`, `gitlab`, `slack`, `linear`, `notion`, `claude-code` (Claude Code), `claude` (Claude API).

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
Kandji, “Preference file” in Intune), give `fr.igitscor.remora` as the domain and the three keys as the
property list.

## Windows: Group Policy or Intune

The Windows app reads the same three keys from **`HKEY_LOCAL_MACHINE\SOFTWARE\Policies\Remora`**:

| Value | Type | Meaning |
|---|---|---|
| `AllowedPlugins` | `REG_MULTI_SZ` | The allowed tools, one ID per line |
| `AllowExternalAI` | `REG_DWORD` | `1` to allow external AI, `0` to forbid it |
| `AllowRemoteImages` | `REG_DWORD` | `1` to load avatars from allowed tools, `0` not to |

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

Check it with `reg query HKLM\SOFTWARE\Policies\Remora`. Remora reads the policy when it starts: users see *Managed by
your organization* in Settings → Privacy after their next sign-in, or after quitting and reopening Remora. Users can’t
write under `HKLM\SOFTWARE\Policies`, so they can’t loosen it.

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

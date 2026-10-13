#!/usr/bin/env python3
"""Writes the ready-to-import policy files in policy/ from the list of managed keys below, so they can't drift from
each other: a macOS configuration profile, a Windows .reg file, Group Policy templates (ADMX, with English and French
ADML), a Jamf custom-settings schema, and the Linux policy file. The profile, .reg and policy.json hold the example
policy of the MDM page (GitHub, GitLab and Linear, no external AI, avatars); the templates and the schema list every
key. `python3 scripts/make-policy-files.py` after editing; `--check` fails when a file is out of date."""

import json
import sys
import uuid
from pathlib import Path
from xml.sax.saxutils import escape

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "policy"
DOMAIN = "fr.igitscor.remora"
PLUGINS = ["github", "gitlab", "slack", "linear", "jira", "notion", "microsoft", "claude-code", "claude", "openai"]
SOURCES = ["github", "gitlab", "slack", "linear", "jira", "notion", "microsoft", "reminders"]

# name, type (list, bool, string, minutes), English title and help, French title and help.
KEYS = [
    (
        "AllowedPlugins",
        "list",
        "Allowed tools",
        "The tools that may be connected and refreshed, by plugin ID: github, gitlab, slack, linear, jira, notion, microsoft, claude-code, claude, openai. Any other is refused.",
        "Outils autorisés",
        "Les outils qui peuvent être connectés et actualisés, par identifiant : github, gitlab, slack, linear, jira, notion, microsoft, claude-code, claude, openai. Tout autre est refusé.",
    ),
    (
        "AllowExternalAI",
        "bool",
        "Allow external AI",
        "Allow the assistant to send inbox content off the computer: to Anthropic for Claude, to the configured server for OpenAI-compatible.",
        "Autoriser l’IA externe",
        "Autoriser l’assistant à envoyer le contenu de la boîte hors de l’ordinateur : à Anthropic pour Claude, au serveur configuré pour OpenAI-compatible.",
    ),
    (
        "AllowRemoteImages",
        "bool",
        "Allow avatars",
        "Load avatars, from the hosts of allowed tools only.",
        "Autoriser les avatars",
        "Charger les avatars, depuis les adresses des outils autorisés seulement.",
    ),
    (
        "AIExcludedSources",
        "list",
        "Tools kept from the assistant",
        "Tools whose items never reach the assistant (brief, summaries, triage); reminders for the user's own reminders. Added to the user's choice.",
        "Outils cachés à l’assistant",
        "Les outils dont les éléments ne parviennent jamais à l’assistant (brief, résumés, tri) ; reminders pour les rappels de l’utilisateur. S’ajoute au choix de l’utilisateur.",
    ),
    (
        "AllowedAIModels",
        "list",
        "Allowed AI models",
        "The models the assistant may use, by any provider's IDs. A model not listed is replaced by the first one.",
        "Modèles d’IA autorisés",
        "Les modèles que l’assistant peut utiliser, avec les identifiants de n’importe quel fournisseur. Un modèle absent est remplacé par le premier.",
    ),
    (
        "AllowedAIServers",
        "list",
        "Allowed AI servers",
        "The servers the OpenAI-compatible assistant may use, as URL prefixes (scheme, host, port and path).",
        "Serveurs d’IA autorisés",
        "Les serveurs que l’assistant OpenAI-compatible peut utiliser, en préfixes d’URL (schéma, hôte, port et chemin).",
    ),
    (
        "AIServer",
        "string",
        "AI server",
        "The server the OpenAI-compatible assistant uses, filled in and locked in Settings.",
        "Serveur d’IA",
        "Le serveur qu’utilise l’assistant OpenAI-compatible, rempli et verrouillé dans les Réglages.",
    ),
    (
        "AllowLocalAI",
        "bool",
        "Allow an AI server on this computer",
        "A server on this computer (localhost): allowed even with external AI off, or refused.",
        "Autoriser un serveur d’IA sur cet ordinateur",
        "Un serveur sur cet ordinateur (localhost) : autorisé même sans IA externe, ou refusé.",
    ),
    (
        "HiddenContentSources",
        "list",
        "Hide message content",
        "Tools whose notifications say where something happened, not what was written; reminders for the user's own reminders. Added to the user's choice.",
        "Masquer le contenu des messages",
        "Les outils dont les notifications disent où quelque chose s’est passé, pas ce qui a été écrit ; reminders pour les rappels de l’utilisateur. S’ajoute au choix de l’utilisateur.",
    ),
    (
        "RefreshMinutes",
        "minutes",
        "Refresh interval",
        "How often tools are checked, from 1 to 60 minutes. Locked in Settings.",
        "Fréquence d’actualisation",
        "La fréquence de vérification des outils, de 1 à 60 minutes. Verrouillée dans les Réglages.",
    ),
    (
        "OpenInApps",
        "bool",
        "Open in desktop apps",
        "Open Slack and Linear items in their desktop app when installed; disabled always opens the web page.",
        "Ouvrir dans les apps",
        "Ouvrir les éléments Slack et Linear dans leur app quand elle est installée ; désactivé, ouvre toujours la page web.",
    ),
    (
        "ClaudeCodePath",
        "string",
        "Claude Code path",
        "The claude program Remora runs, instead of the one found or entered. It must still be named claude and pass Remora's checks.",
        "Chemin de Claude Code",
        "Le programme claude que Remora exécute, au lieu de celui trouvé ou saisi. Il doit toujours s’appeler claude et passer les vérifications de Remora.",
    ),
    (
        "MicrosoftTenant",
        "string",
        "Microsoft 365 tenant",
        "The organization's Microsoft Entra tenant, by domain or ID, for signing in to Microsoft 365. Filled in and locked in the connect form.",
        "Tenant Microsoft 365",
        "Le tenant Microsoft Entra de l’organisation, par domaine ou identifiant, pour la connexion à Microsoft 365. Rempli et verrouillé dans le formulaire de connexion.",
    ),
    (
        "MicrosoftClientID",
        "string",
        "Microsoft 365 client ID",
        "The application (client) ID of the app the organization registered in Microsoft Entra for Remora. Filled in and locked in the connect form.",
        "ID client Microsoft 365",
        "L’identifiant d’application (client) de l’app que l’organisation a inscrite dans Microsoft Entra pour Remora. Rempli et verrouillé dans le formulaire de connexion.",
    ),
    (
        "AutomaticUpdates",
        "bool",
        "Automatic updates",
        "Enabled: check for a new version once a day, locked on. Disabled: no update check at all.",
        "Mises à jour automatiques",
        "Activé : vérifier une nouvelle version une fois par jour, verrouillé. Désactivé : aucune vérification.",
    ),
]

# The MDM page's example: GitHub, GitLab and Linear, no external AI, avatars.
EXAMPLE = {"AllowedPlugins": ["github", "gitlab", "linear"], "AllowExternalAI": False, "AllowRemoteImages": True}


def stable_uuid(name: str) -> str:
    """The same UUID at each run, so the file only changes when its content does. Replace them with your own."""
    return str(uuid.uuid5(uuid.NAMESPACE_URL, f"https://triage.iscor.me/{name}")).upper()


def plist_value(value, indent: str) -> str:
    if isinstance(value, bool):
        return "<true/>" if value else "<false/>"
    if isinstance(value, list):
        items = "".join(f"\n{indent}    <string>{escape(v)}</string>" for v in value)
        return f"<array>{items}\n{indent}</array>"
    return f"<string>{escape(str(value))}</string>"


def mobileconfig() -> str:
    pad = " " * 12
    keys = "\n".join(f"{pad}<key>{k}</key>{plist_value(v, pad)}" for k, v in EXAMPLE.items())
    return f"""<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<!-- Remora's example policy (GitHub, GitLab and Linear, no external AI, avatars). Generated by
     scripts/make-policy-files.py. Replace the identifiers with your organization's, and the UUIDs with your own. -->
<plist version="1.0">
<dict>
    <key>PayloadType</key><string>Configuration</string>
    <key>PayloadVersion</key><integer>1</integer>
    <key>PayloadScope</key><string>System</string>
    <key>PayloadIdentifier</key><string>com.example.remora</string>
    <key>PayloadUUID</key><string>{stable_uuid("profile")}</string>
    <key>PayloadDisplayName</key><string>Remora privacy policy</string>
    <key>PayloadContent</key>
    <array>
        <dict>
            <key>PayloadType</key><string>{DOMAIN}</string>
            <key>PayloadVersion</key><integer>1</integer>
            <key>PayloadIdentifier</key><string>com.example.remora.policy</string>
            <key>PayloadUUID</key><string>{stable_uuid("payload")}</string>
            <key>PayloadDisplayName</key><string>Remora</string>
{keys}
        </dict>
    </array>
</dict>
</plist>
"""


def reg_multi_sz(values: list[str]) -> str:
    data = "".join(v + "\0" for v in values) + "\0"
    return "hex(7):" + ",".join(f"{b:02x}" for b in data.encode("utf-16-le"))


def reg() -> str:
    lines = ["Windows Registry Editor Version 5.00", "", r"[HKEY_LOCAL_MACHINE\SOFTWARE\Policies\Remora]"]
    for key, value in EXAMPLE.items():
        if isinstance(value, bool):
            lines.append(f'"{key}"=dword:{int(value):08x}')
        elif isinstance(value, list):
            lines.append(f'"{key}"={reg_multi_sz(value)}')
        else:
            lines.append(f'"{key}"="{value}"')
    return "\r\n".join(lines) + "\r\n"


def admx() -> str:
    policies = []
    for name, kind, *_ in KEYS:
        head = (
            f'    <policy name="{name}" class="Both" displayName="$(string.{name})" '
            f'explainText="$(string.{name}_Help)" key="Software\\Policies\\Remora"'
        )
        refs = '      <parentCategory ref="Remora" />\n      <supportedOn ref="SUPPORTED_Remora" />\n'
        if kind == "bool":
            policies.append(
                f'{head} valueName="{name}">\n{refs}'
                '      <enabledValue><decimal value="1" /></enabledValue>\n'
                '      <disabledValue><decimal value="0" /></disabledValue>\n    </policy>'
            )
            continue
        element = {
            "list": f'<multiText id="{name}" valueName="{name}" />',
            "string": f'<text id="{name}" valueName="{name}" />',
            "minutes": f'<decimal id="{name}" valueName="{name}" minValue="1" maxValue="60" />',
        }[kind]
        policies.append(
            f'{head} presentation="$(presentation.{name})">\n{refs}'
            f"      <elements>\n        {element}\n      </elements>\n    </policy>"
        )
    body = "\n".join(policies)
    return f"""<?xml version="1.0" encoding="utf-8"?>
<!-- Remora's Group Policy template, every managed key. Generated by scripts/make-policy-files.py. Copy it to
     PolicyDefinitions (or the central store) with en-US\\Remora.adml and fr-FR\\Remora.adml. -->
<policyDefinitions xmlns:xsd="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" revision="1.0" schemaVersion="1.0" xmlns="http://schemas.microsoft.com/GroupPolicy/2006/07/PolicyDefinitions">
  <policyNamespaces>
    <target prefix="remora" namespace="Remora.Policies.Remora" />
  </policyNamespaces>
  <resources minRequiredRevision="1.0" />
  <supportedOn>
    <definitions>
      <definition name="SUPPORTED_Remora" displayName="$(string.SUPPORTED_Remora)" />
    </definitions>
  </supportedOn>
  <categories>
    <category name="Remora" displayName="$(string.Remora)" />
  </categories>
  <policies>
{body}
  </policies>
</policyDefinitions>
"""


def adml(french: bool) -> str:
    strings = [
        ("Remora", "Remora"),
        ("SUPPORTED_Remora", "Remora 0.6 ou ultérieure" if french else "Remora 0.6 or later"),
    ]
    presentations = []
    for name, kind, title_en, help_en, title_fr, help_fr in KEYS:
        strings.append((name, title_fr if french else title_en))
        strings.append((f"{name}_Help", help_fr if french else help_en))
        if kind == "list":
            label = "Une valeur par ligne" if french else "One value per line"
            box = f'<multiTextBox refId="{name}">{label}</multiTextBox>'
        elif kind == "string":
            box = f'<textBox refId="{name}"><label>{escape(title_fr if french else title_en)}</label></textBox>'
        elif kind == "minutes":
            box = f'<decimalTextBox refId="{name}" defaultValue="5">Minutes</decimalTextBox>'
        else:
            continue
        presentations.append(f'      <presentation id="{name}">{box}</presentation>')
    table = "\n".join(f'      <string id="{k}">{escape(v)}</string>' for k, v in strings)
    description = (
        "Réglages gérés de Remora (fr.igitscor.remora)."
        if french
        else "Remora's managed settings (fr.igitscor.remora)."
    )
    return f"""<?xml version="1.0" encoding="utf-8"?>
<!-- Generated by scripts/make-policy-files.py. -->
<policyDefinitionResources xmlns:xsd="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" revision="1.0" schemaVersion="1.0" xmlns="http://schemas.microsoft.com/GroupPolicy/2006/07/PolicyDefinitions">
  <displayName>Remora</displayName>
  <description>{description}</description>
  <resources>
    <stringTable>
{table}
    </stringTable>
    <presentationTable>
{chr(10).join(presentations)}
    </presentationTable>
  </resources>
</policyDefinitionResources>
"""


def jamf() -> str:
    properties = {}
    for order, (name, kind, title, help_text, *_) in enumerate(KEYS, start=1):
        prop = {"title": title, "description": help_text, "property_order": order * 5}
        if kind == "bool":
            prop["type"] = "boolean"
        elif kind == "minutes":
            prop.update(type="integer", minimum=1, maximum=60, default=5)
        elif kind == "string":
            prop["type"] = "string"
        else:
            items = {"type": "string"}
            if name == "AllowedPlugins":
                items["enum"] = PLUGINS
            elif name in ("AIExcludedSources", "HiddenContentSources"):
                items["enum"] = SOURCES
            prop.update(type="array", items=items)
        properties[name] = prop
    schema = {
        "title": f"Remora ({DOMAIN})",
        "description": "Remora's managed settings. Generated by scripts/make-policy-files.py.",
        "properties": properties,
    }
    return json.dumps(schema, indent=2, ensure_ascii=False) + "\n"


def linux() -> str:
    return json.dumps(EXAMPLE, indent=2) + "\n"


TARGETS = {
    "Remora.mobileconfig": (mobileconfig, "utf-8"),
    "Remora.reg": (reg, "utf-16"),
    "Remora.admx": (admx, "utf-8"),
    "en-US/Remora.adml": (lambda: adml(False), "utf-8"),
    "fr-FR/Remora.adml": (lambda: adml(True), "utf-8"),
    "Remora.jamf.json": (jamf, "utf-8"),
    "policy.json": (linux, "utf-8"),
}

if __name__ == "__main__":
    check = "--check" in sys.argv
    stale = []
    for name, (render, encoding) in TARGETS.items():
        path = OUT / name
        # Regedit expects UTF-16 with a byte order mark for version 5.00 files.
        data = render().encode(encoding)
        if check:
            if not path.exists() or path.read_bytes() != data:
                stale.append(name)
        else:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
            print(f"policy/{name}")
    if stale:
        sys.exit(f"Out of date, run python3 scripts/make-policy-files.py: {', '.join(stale)}")

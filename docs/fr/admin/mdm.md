---
title: Gérer la politique par MDM
description: Verrouiller les outils que Remora peut connecter, l’IA externe, les serveurs que l’assistant peut utiliser et le chargement des avatars, par un profil de configuration sur macOS (Jamf, Kandji, Intune, Mosyle, Apple Business Essentials) ou des valeurs de registre sous Windows (stratégie de groupe, Intune). Les clés et un profil complet.
---

# Gérer la politique par MDM

L’organisation peut verrouiller la politique de confidentialité de Remora par un profil de configuration pour le
domaine de préférences **`fr.igitscor.remora`** sur Mac, et par des [valeurs de registre](#windows-strategie-de-groupe-ou-intune) sous Windows. Tout MDM qui déploie des réglages personnalisés convient : Jamf
Pro, Kandji, Microsoft Intune, Mosyle, Apple Business Essentials…

Dès qu’une clé est définie, Réglages → Confidentialité affiche **Géré par votre organisation** et l’utilisateur ne
peut plus modifier la politique.

## Les clés

| Clé | Type | Signification | Par défaut sans gestion |
|---|---|---|---|
| `AllowedPlugins` | Tableau de chaînes | Les outils qui peuvent être connectés et actualisés. Les autres sont refusés avec « Non autorisé par votre politique de confidentialité. » | Tous |
| `AllowExternalAI` | Booléen | Autoriser l’assistant à envoyer le contenu de la boîte hors de l’ordinateur : à Anthropic pour Claude, au serveur choisi pour OpenAI-compatible | `false` |
| `AllowRemoteImages` | Booléen | Charger les avatars, depuis les outils autorisés seulement | `true` |
| `AIExcludedSources` | Tableau de chaînes | Les outils dont les éléments ne parviennent jamais à l’assistant (brief, résumés, tri) ; `reminders` pour les rappels de l’utilisateur. S’ajoute au choix de l’utilisateur et le verrouille dans les Réglages | Aucun |
| `AllowedAIModels` | Tableau de chaînes | Les modèles que l’assistant peut utiliser, avec les identifiants de n’importe quel fournisseur. Un modèle absent de la liste (ou celui par défaut, s’il n’y est pas) est remplacé par le premier | Tous |
| `AllowedAIServers` | Tableau de chaînes | Les serveurs que l’assistant OpenAI-compatible peut utiliser, en préfixes d’URL : schéma, hôte, port et chemin doivent correspondre, le chemin sur une limite de `/` (`https://acme.openai.azure.com/openai/` autorise `…/openai/v1`, pas d’autres ressources). Tout autre serveur est refusé avec « Votre organisation n’autorise pas ce serveur d’IA. » | Tout serveur https |
| `AIServer` | Chaîne | Le serveur qu’utilise l’assistant OpenAI-compatible, rempli et verrouillé dans les Réglages, y compris pour les comptes déjà connectés. Refusé si `AllowedAIServers` ne l’autorise pas | Choisi par l’utilisateur |
| `AllowLocalAI` | Booléen | Un serveur sur cet ordinateur (`localhost`, `127.0.0.1`, `::1`, seules adresses qui peuvent utiliser http). `true` : autorisé même avec l’IA externe désactivée. `false` : refusé. Non défini : autorisé sauf si `AllowExternalAI` est géré à `false`, car un serveur local pourrait relayer vers le cloud | Voir le sens |
| `HiddenContentSources` | Tableau de chaînes | Les outils dont les notifications disent où quelque chose s’est passé, pas ce qui a été écrit ; `reminders` pour les rappels de l’utilisateur. S’ajoute au choix de l’utilisateur et le verrouille dans les Réglages | Aucun |
| `RefreshMinutes` | Entier | La fréquence de vérification des outils, de 1 à 60 minutes (une autre valeur est ignorée). Verrouillé dans les Réglages | 5 |
| `OpenInApps` | Booléen | Ouvrir les éléments Slack et Linear dans leur app quand elle est installée ; `false` ouvre toujours la page web | `true` |
| `ClaudeCodePath` | Chaîne | Le `claude` que Remora exécute, à la place de celui trouvé ou saisi dans le compte. Il doit toujours s’appeler `claude`, appartenir à l’utilisateur ou à root, et répondre comme Claude Code | Trouvé automatiquement |
| `AutomaticUpdates` | Booléen | `true` : interroge GitHub une fois par jour pour une nouvelle version, verrouillé. `false` : aucune vérification, même pas *Vérifier maintenant*, pour les parcs que vous redéployez vous-même | Désactivé ; l’utilisateur décide |
| `MicrosoftTenant` | Chaîne | Le tenant Microsoft Entra de l’organisation (`acme.onmicrosoft.com`, ou son identifiant), pour la connexion à Microsoft 365. Rempli et verrouillé dans le formulaire de connexion, y compris pour les comptes déjà connectés | Choisi par l’utilisateur |
| `MicrosoftClientID` | Chaîne | L’identifiant d’application (client) de l’app que vous avez inscrite dans Microsoft Entra pour Remora. Rempli et verrouillé dans le formulaire de connexion | Choisi par l’utilisateur |

`AIServer` et les sept dernières sont des réglages, pas des règles de confidentialité : elles ne verrouillent pas Réglages → Confidentialité.

Identifiants : `github`, `gitlab`, `slack`, `linear`, `jira`, `notion`, `microsoft`, `claude-code` (Claude Code), `claude` (API Claude),
`openai` (OpenAI-compatible). Pour choisir l’assistant, ne listez que celui-là dans `AllowedPlugins`.

Par exemple, Claude autorisé pour les revues de code et les tâches, jamais pour Slack, et seulement avec Sonnet et
Haiku :

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

Sous Windows, ce sont deux [valeurs de registre](#windows-strategie-de-groupe-ou-intune) `REG_MULTI_SZ`.

Ou l’assistant OpenAI-compatible sur la seule ressource Azure OpenAI de l’entreprise, réglée pour tous :

```xml
<key>AllowedPlugins</key>
<array>
    <string>github</string>
    <string>openai</string>
</array>
<key>AllowExternalAI</key><true/>
<key>AllowedAIServers</key>
<array>
    <string>https://acme.openai.azure.com/openai/</string>
</array>
<key>AIServer</key><string>https://acme.openai.azure.com/openai/v1</string>
```

Sans `AllowedAIServers` ni `AIServer`, l’utilisateur peut saisir n’importe quel serveur https : un parc géré qui
autorise `openai` devrait définir l’une des deux.

Un compte déjà connecté à un outil que la politique n’autorise plus cesse d’être actualisé à la synchronisation
suivante.

## Des fichiers prêts à importer

Pour ne rien saisir à la main, chaque version publiée joint **`Remora-policy-files.zip`** (aussi dans le dossier
[`policy/`](https://github.com/iGitScor/triage/tree/main/policy) du dépôt) :

- `Remora.mobileconfig` : le profil ci-dessous, prêt à envoyer (remplacez les identifiants et les UUID par les
  vôtres) ;
- `Remora.jamf.json` : un schéma Jamf Pro, pour que *Application & Custom Settings* affiche chaque clé en formulaire ;
- `Remora.admx` avec `Remora.adml` en `en-US` et `fr-FR` : les modèles de stratégie de groupe (chaque clé sous
  *Remora*, pour l’ordinateur ou l’utilisateur), importables aussi dans Intune ;
- `Remora.reg` : la même politique que le profil, pour `reg import` ;
- `policy.json` : la même pour Linux.

## Un profil complet

Ce profil autorise GitHub, GitLab et Linear, interdit l’IA externe et garde les avatars. Remplacez les deux UUID par
les vôtres (`uuidgen` dans le Terminal) et les identifiants par ceux de votre organisation.

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
    <key>PayloadDisplayName</key><string>Politique de confidentialité Remora</string>
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

Dans les MDM qui ne prennent que les réglages (une charge « Application & Custom Settings » dans Jamf, « Custom
Configuration » dans Kandji, « Fichier de préférences » dans Intune), indiquez `fr.igitscor.remora` comme domaine
et les clés comme liste de propriétés.

## Windows : stratégie de groupe ou Intune

L’app Windows lit ces valeurs sous **`HKEY_LOCAL_MACHINE\SOFTWARE\Policies\Remora`**,
puis sous `HKEY_CURRENT_USER\SOFTWARE\Policies\Remora` pour les stratégies par utilisateur (une valeur définie dans HKLM l’emporte) :

| Valeur | Type | Signification |
|---|---|---|
| `AllowedPlugins` | `REG_MULTI_SZ` | Les outils autorisés, un identifiant par ligne |
| `AllowExternalAI` | `REG_DWORD` | `1` pour autoriser l’IA externe, `0` pour l’interdire |
| `AllowRemoteImages` | `REG_DWORD` | `1` pour charger les avatars des outils autorisés, `0` sinon |
| `AutomaticUpdates` | `REG_DWORD` | `1` pour vérifier les mises à jour chaque jour (verrouillé), `0` pour les désactiver |
| `AllowedAIServers` | `REG_MULTI_SZ` | Les serveurs OpenAI-compatibles autorisés, un préfixe d’URL par ligne |
| `AIServer` | `REG_SZ` | Le serveur OpenAI-compatible, réglé et verrouillé |
| `AllowLocalAI` | `REG_DWORD` | `1` pour autoriser un serveur sur cet ordinateur, `0` pour le refuser |
| `AIExcludedSources` | `REG_MULTI_SZ` | Les outils dont les éléments ne parviennent jamais à l’assistant, un identifiant par ligne (`reminders` pour les rappels) ; verrouillé dans les Réglages |
| `AllowedAIModels` | `REG_MULTI_SZ` | Les modèles que l’assistant peut utiliser, un identifiant par ligne |
| `HiddenContentSources` | `REG_MULTI_SZ` | Les outils dont les notifications ne montrent pas ce qui a été écrit, un identifiant par ligne |
| `RefreshMinutes` | `REG_DWORD` | La fréquence de vérification des outils, de 1 à 60 |
| `OpenInApps` | `REG_DWORD` | `1` pour ouvrir Slack et Linear dans leur app, `0` pour la page web |
| `ClaudeCodePath` | `REG_SZ` | Le `claude.exe` (ou `claude.cmd`) que Remora exécute |
| `MicrosoftTenant` | `REG_SZ` | Le tenant Microsoft Entra, réglé et verrouillé |
| `MicrosoftClientID` | `REG_SZ` | L’identifiant client de l’app inscrite pour Remora, réglé et verrouillé |

Déployez-les par une préférence de stratégie de groupe (Configuration ordinateur → Préférences → Paramètres Windows →
Registre), un script de correction ou de configuration Intune, ou un fichier `.reg`. La même politique que le profil
ci-dessus, en PowerShell (en administrateur) :

```powershell
$key = 'HKLM:\SOFTWARE\Policies\Remora'
New-Item -Path $key -Force | Out-Null
New-ItemProperty -Path $key -Name AllowedPlugins -PropertyType MultiString -Value 'github','gitlab','linear' -Force
New-ItemProperty -Path $key -Name AllowExternalAI -PropertyType DWord -Value 0 -Force
New-ItemProperty -Path $key -Name AllowRemoteImages -PropertyType DWord -Value 1 -Force
```

Vérifiez avec `reg query HKLM\SOFTWARE\Policies\Remora`. Remora lit la politique au lancement puis à chaque
actualisation : un changement s’applique dans l’intervalle d’actualisation, sans redémarrage, et l’utilisateur voit
*Géré par votre organisation* dans Réglages → Confidentialité. Un utilisateur ne peut pas écrire sous
`HKLM\SOFTWARE\Policies`, donc pas assouplir ce qu’elle fixe.

Une erreur de saisie n’ouvre jamais rien. Les listes acceptent aussi un `REG_SZ` (`github, slack`), et les
interrupteurs un `REG_SZ` `0` ou `1`. Tout autre type, ou une valeur illisible, est appliqué de la façon la plus
stricte (aucun outil, pas d’IA externe, aucun serveur d’IA, pas de serveur local, pas d’avatars, pas de mises à jour ;
un `AIExcludedSources` ou un `AllowedAIModels` illisible coupe l’assistant), et Réglages → Confidentialité indique la
valeur à corriger.

## Linux : un fichier de politique

L’app Linux (en aperçu) lit les mêmes clés dans **`/etc/remora/policy.json`**, au lancement puis à chaque
actualisation. Déployez-le par Ansible, Puppet, Salt ou un paquet ; il doit appartenir à root et n’être modifiable que
par root (`0644`) :

```json
{
  "AllowedPlugins": ["github", "gitlab", "linear"],
  "AllowExternalAI": false,
  "AllowRemoteImages": true
}
```

Les listes sont des tableaux JSON, les interrupteurs `true` ou `false`, les textes des chaînes. La même règle vaut :
une valeur du mauvais type est appliquée de la façon la plus stricte et nommée dans Réglages → Confidentialité. Un
fichier qui n’est pas du JSON valide, n’appartient pas à root ou que d’autres peuvent modifier verrouille tout (aucun
outil, pas d’IA, pas d’avatars, pas de mises à jour) jusqu’à ce qu’il soit corrigé.

## Vérifier sur un Mac

```sh
# Le profil est installé :
sudo profiles show -type configuration | grep -A3 fr.igitscor.remora
# Les valeurs gérées que Remora lit :
defaults read /Library/Managed\ Preferences/fr.igitscor.remora
```

Puis ouvrez Remora → Réglages → Confidentialité : *Géré par votre organisation*, les outils autorisés, et les
interrupteurs IA et avatars verrouillés.

::: info Pourquoi `defaults write` ne suffit pas
Remora n’applique que les valeurs que macOS signale comme gérées (imposées) par un profil. Un `defaults write` de
l’utilisateur est ignoré pour toutes ces clés : un utilisateur ne peut pas assouplir la politique, et un test ne peut
pas la verrouiller par accident.
:::

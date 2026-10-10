---
title: Gérer la politique par MDM
description: Verrouiller les outils que Remora peut connecter, l’IA externe et le chargement des avatars par un profil de configuration (Jamf, Kandji, Intune, Mosyle, Apple Business Essentials). Les clés et un profil complet.
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
| `AllowExternalAI` | Booléen | Autoriser les extensions Claude à envoyer le contenu de la boîte à Anthropic | `false` |
| `AllowRemoteImages` | Booléen | Charger les avatars, depuis les outils autorisés seulement | `true` |
| `AIExcludedSources` | Tableau de chaînes | Les outils dont les éléments ne parviennent jamais à l’assistant (brief, résumés, tri) ; `reminders` pour les rappels de l’utilisateur. S’ajoute au choix de l’utilisateur et le verrouille dans les Réglages | Aucun |
| `AllowedAIModels` | Tableau de chaînes | Les modèles Claude que l’assistant peut utiliser. Un modèle absent de la liste (ou celui par défaut, s’il n’y est pas) est remplacé par le premier | Tous |
| `HiddenContentSources` | Tableau de chaînes | Les outils dont les notifications disent où quelque chose s’est passé, pas ce qui a été écrit ; `reminders` pour les rappels de l’utilisateur. S’ajoute au choix de l’utilisateur et le verrouille dans les Réglages | Aucun |
| `RefreshMinutes` | Entier | La fréquence de vérification des outils, de 1 à 60 minutes (une autre valeur est ignorée). Verrouillé dans les Réglages | 5 |
| `OpenInApps` | Booléen | Ouvrir les éléments Slack et Linear dans leur app quand elle est installée ; `false` ouvre toujours la page web | `true` |
| `ClaudeCodePath` | Chaîne | Le `claude` que Remora exécute, à la place de celui trouvé ou saisi dans le compte. Il doit toujours s’appeler `claude`, appartenir à l’utilisateur ou à root, et répondre comme Claude Code | Trouvé automatiquement |
| `AutomaticUpdates` | Booléen | `true` : interroge GitHub une fois par jour pour une nouvelle version, verrouillé. `false` : aucune vérification, même pas *Vérifier maintenant*, pour les parcs que vous redéployez vous-même | Désactivé ; l’utilisateur décide |

Les cinq dernières sont des réglages, pas des règles de confidentialité : elles ne verrouillent pas Réglages → Confidentialité.

Identifiants : `github`, `gitlab`, `slack`, `linear`, `notion`, `claude-code` (Claude Code), `claude` (API Claude).
Pour choisir entre Claude Code et l’API Claude, ne listez que l’un des deux dans `AllowedPlugins`.

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

Ces deux clés sont lues par l’app macOS ; l’app Windows n’a pas encore d’assistant.

Un compte déjà connecté à un outil que la politique n’autorise plus cesse d’être actualisé à la synchronisation
suivante.

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

L’app Windows lit les trois mêmes clés sous **`HKEY_LOCAL_MACHINE\SOFTWARE\Policies\Remora`** :

| Valeur | Type | Signification |
|---|---|---|
| `AllowedPlugins` | `REG_MULTI_SZ` | Les outils autorisés, un identifiant par ligne |
| `AllowExternalAI` | `REG_DWORD` | `1` pour autoriser l’IA externe, `0` pour l’interdire |
| `AllowRemoteImages` | `REG_DWORD` | `1` pour charger les avatars des outils autorisés, `0` sinon |
| `AutomaticUpdates` | `REG_DWORD` | `1` pour vérifier les mises à jour chaque jour (verrouillé), `0` pour les désactiver |

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

Vérifiez avec `reg query HKLM\SOFTWARE\Policies\Remora`. Remora lit la politique à son démarrage : l’utilisateur voit
*Géré par votre organisation* dans Réglages → Confidentialité après sa prochaine ouverture de session, ou après avoir
Une erreur de saisie n’ouvre jamais rien. `AllowedPlugins` accepte aussi une liste en `REG_SZ` (`github, slack`), et
les interrupteurs un `REG_SZ` `0` ou `1`. Tout autre type, ou une valeur illisible, est appliqué de la façon la
plus stricte (aucun outil, pas d’IA externe, pas d’avatars, pas de mises à jour), et Réglages → Confidentialité indique la valeur à
corriger.

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
l’utilisateur est ignoré pour ces trois clés : un utilisateur ne peut pas assouplir la politique, et un test ne peut
pas la verrouiller par accident.
:::

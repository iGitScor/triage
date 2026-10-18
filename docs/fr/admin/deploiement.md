---
title: Déployer Remora
description: Distribuer Remora sur Mac, PC Windows et bureaux Linux. Le DMG, l’installeur et leurs empreintes, Gatekeeper et le trousseau, les mises à jour, une copie signée.
---

# Déployer Remora

## La version publiée

Chaque version est publiée sur [GitHub Releases](https://github.com/iGitScor/triage/releases) sous le nom
**Remora.dmg**, avec son SHA-256 à côté. La dernière est toujours à :

```
https://github.com/iGitScor/triage/releases/latest/download/Remora.dmg
```

Le DMG contient `Remora.app`, la licence et les mentions des composants tiers. L’app demande macOS 14 ou ultérieur,
Apple Silicon ou Intel.

Vérifier un téléchargement :

```sh
shasum -a 256 -c Remora.dmg.sha256
gh attestation verify Remora.dmg --repo iGitScor/triage
```

La seconde ligne vérifie l’enregistrement signé par GitHub attestant que ce fichier précis a été construit par le
workflow de publication à partir du commit étiqueté du dépôt (provenance). Chaque version publie aussi un **SBOM**,
`Remora.dmg.cdx.json` et `Remora-Setup.exe.cdx.json` (CycloneDX), qui liste les composants tiers d’après les
fichiers de verrouillage ; il est attesté de la même façon. L’app Mac n’a aucun paquet tiers : son SBOM est court.

## Signature, Gatekeeper et trousseau

Les versions sont pour l’instant signées **ad hoc**, pas avec un Developer ID Apple, et ne sont pas notarisées (la
[politique de signature du code](./signature-du-code) dit ce qui change une fois la signature en place). Deux
conséquences :

- **Gatekeeper** bloque le premier lancement. Les utilisateurs font clic droit → Ouvrir, ou, sous macOS 15 et
  ultérieur, Réglages Système → Confidentialité et sécurité → Ouvrir quand même. Une fois par version.
- **Trousseau** : macOS reconnaît une app signée ad hoc à sa compilation exacte, donc après chaque mise à jour il
  demande une fois si Remora peut utiliser son élément. Les utilisateurs choisissent *Toujours autoriser*.

Pour éviter les deux, signez avec le Developer ID de votre organisation et faites notariser, comme ci-dessous. Une
copie signée garde le même identifiant (`fr.igitscor.remora`), donc la [politique MDM](./mdm) s’applique telle
quelle.

## La signer avec votre Developer ID

Sur un Mac avec Xcode et votre certificat *Developer ID Application* :

```sh
git clone https://github.com/iGitScor/triage && cd triage/macos
REMORA_SIGN_IDENTITY="Developer ID Application: Votre organisation (TEAMID)" REMORA_UNIVERSAL=1 ./scripts/build-app.sh
codesign --force --options runtime --sign "Developer ID Application: Votre organisation (TEAMID)" build/Remora.app
ditto -c -k --keepParent build/Remora.app Remora.zip
xcrun notarytool submit Remora.zip --keychain-profile <votre-profil> --wait
xcrun stapler staple build/Remora.app
```

Puis empaquetez `build/Remora.app` comme d’habitude (DMG, ou paquet pour le catalogue de votre MDM).

## Installer par MDM

Remora est un simple paquet d’app, sans installeur, agent de lancement ni extension système. Déployez-la dans
`/Applications` comme toute autre app, avec le profil de configuration. *Ouvrir à la connexion* reste un choix de
chaque utilisateur dans les réglages de Remora (il utilise les éléments de connexion de macOS).

## Windows (aperçu)

Chaque version publie aussi **Remora-Setup.exe** (avec son SHA-256, son SBOM et son attestation), toujours à
`https://github.com/iGitScor/triage/releases/latest/download/Remora-Setup.exe`. C’est un installeur NSIS qui installe pour l’utilisateur courant, dans
`%LOCALAPPDATA%\Remora`, sans droits d’administrateur ; installation silencieuse : `Remora-Setup.exe /S`.
Ses données sont dans `%LOCALAPPDATA%\fr.igitscor.remora` et ses jetons dans le Gestionnaire d’identification, une entrée par compte
(`fr.igitscor.remora`). Il n’est pas encore signé, donc SmartScreen prévient une fois (*Informations complémentaires* →
*Exécuter quand même*) ; pour l’éviter, signez-le avec le certificat de signature de code de votre organisation avant
de le déployer. La politique se règle dans le registre : voir [MDM](./mdm#windows-strategie-de-groupe-ou-intune).

Pour Intune, SCCM ou un autre outil de flotte, utilisez **Remora.msi** (même SHA-256, SBOM et attestation, à
`https://github.com/iGitScor/triage/releases/latest/download/Remora.msi`). Il installe pour tous les utilisateurs,
dans `C:\Program Files\Remora` ; chacun garde ses données, ses jetons et ses réglages.

- Installation silencieuse : `msiexec /i Remora.msi /qn` ; désinstallation : `msiexec /x Remora.msi /qn`, ou depuis
  *Applications*.
- Intune : ajoutez-le comme application métier (Intune lit le code produit), ou comme application Win32 détectée par
  le fichier `%ProgramFiles%\Remora\remora.exe`, en version égale ou supérieure à celle que vous déployez.
- Un MSI plus récent remplace celui installé, sans seconde copie.
- Installé ainsi, Remora ne se met pas à jour lui-même : vous déployez chaque version, et les Réglages indiquent
  *Votre organisation installe les nouvelles versions de Remora*.

L’app qu’il contient est signée quand l’installeur l’est ; le MSI lui-même n’est pas encore signé (voir
[Signature du code](./signature-du-code)).

## Linux (aperçu)

Chaque version publie aussi **Remora-x86_64.AppImage** et **Remora-amd64.deb**, chacun avec son SHA-256, son SBOM et
son attestation, toujours à `https://github.com/iGitScor/triage/releases/latest/download/Remora-x86_64.AppImage` et `https://github.com/iGitScor/triage/releases/latest/download/Remora-amd64.deb`. Le `.deb`
installe `remora` pour tout le système sous Ubuntu et Debian ; l’AppImage se lance de n’importe où et se met à jour
seule. Ses données sont dans `~/.local/share/fr.igitscor.remora`, lisibles par l’utilisateur seul, et ses jetons dans
le Secret Service (GNOME Keyring, KWallet ou KeePassXC). La politique est un fichier appartenant à root : voir
[MDM](./mdm#linux-un-fichier-de-politique).

## Mises à jour

Désactivées par défaut : rien n’appelle l’extérieur. Un utilisateur peut activer *Rechercher les mises à jour
automatiquement* (Réglages → Général) ; Remora interroge alors GitHub une fois par jour et installe une nouvelle
version, signée par la clé de publication, quand il le choisit. Réglez `AutomaticUpdates` dans la
[politique](./mdm) pour décider pour tous : vrai l’active et le verrouille, faux désactive entièrement les mises à
jour (même *Vérifier maintenant*) si vous redéployez chaque version vous-même. Le MSI les désactive de lui-même. Suivez les
[versions](https://github.com/iGitScor/triage/releases) (GitHub → Watch → Custom → Releases) dans tous les cas.

## La désinstaller

Quittez Remora et supprimez l’app. Ses données sont dans `~/Library/Application Support/Remora` et un élément du
trousseau de session (service `fr.igitscor.remora`) ; Réglages → Confidentialité → *Effacer les données locales…*
supprime les deux avant de désinstaller.

Sous Windows, désinstallez Remora depuis Paramètres → Applications ; l’option « supprimer les données de l’app » du
désinstalleur retire `%LOCALAPPDATA%\fr.igitscor.remora`. Les jetons restent dans le Gestionnaire d’identification,
sauf si *Effacer les données locales…* a été lancé avant.

Sous Linux, retirez le paquet (`sudo apt remove remora`) ou l’AppImage ; ses données sont dans
`~/.local/share/fr.igitscor.remora`, et *Effacer les données locales…* retire d’abord ses jetons du trousseau.

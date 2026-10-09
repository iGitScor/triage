---
title: Déployer Remora
description: Distribuer Remora sur vos Mac. Le DMG de chaque version et son empreinte, Gatekeeper et la demande du trousseau avec la signature actuelle, les mises à jour, et une copie signée avec votre Developer ID.
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
```

## Signature, Gatekeeper et trousseau

Les versions sont pour l’instant signées **ad hoc**, pas avec un Developer ID Apple, et ne sont pas notarisées. Deux
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

Chaque version publie aussi **Remora-Setup.exe** (avec son SHA-256), toujours à
`https://github.com/iGitScor/triage/releases/latest/download/Remora-Setup.exe`. C’est un installeur NSIS qui installe pour l’utilisateur courant, dans
`%LOCALAPPDATA%\Remora`, sans droits d’administrateur ; installation silencieuse : `Remora-Setup.exe /S`.
Ses données sont dans `%LOCALAPPDATA%\fr.igitscor.remora` et ses jetons dans une entrée du Gestionnaire d’identification
(`fr.igitscor.remora`). Il n’est pas encore signé, donc SmartScreen prévient une fois (*Informations complémentaires* →
*Exécuter quand même*) ; pour l’éviter, signez-le avec le certificat de signature de code de votre organisation avant
de le déployer. La politique se règle dans le registre : voir [MDM](./mdm#windows-strategie-de-groupe-ou-intune).

## Mises à jour

Remora ne se met pas à jour toute seule et ne vérifie pas les mises à jour : rien n’appelle l’extérieur. Suivez les
[versions](https://github.com/iGitScor/triage/releases) (GitHub → Watch → Custom → Releases) et redéployez.

## La désinstaller

Quittez Remora et supprimez l’app. Ses données sont dans `~/Library/Application Support/Remora` et un élément du
trousseau de session (service `fr.igitscor.remora`) ; Réglages → Confidentialité → *Effacer les données locales…*
supprime les deux avant de désinstaller.

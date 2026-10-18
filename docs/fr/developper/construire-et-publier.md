---
title: Construire et publier
description: Construire et lancer Remora pour macOS, la signer, publier une version en DMG et en installeur Windows, et déployer le site et cette documentation.
---

# Construire et publier

À la racine du dépôt, `make` liste un raccourci pour chaque commande de cette page et de celle sur Windows
(`make test`, `make check`, `make mac-demo`, `make docs`…) ;
[CONTRIBUTING.md](https://github.com/iGitScor/triage/blob/main/CONTRIBUTING.md) dit ce qu’une modification doit
remplir avant la relecture.

`make setup` installe aussi un hook de pré-commit qui formate ce que vous commitez (Biome, `swift format`,
`rustfmt`, ruff) ; `make format` formate tout et `make format-check` le vérifie, comme la CI.

## Fichiers partagés entre les dossiers

`macos/`, `desktop/`, `site/` et `docs/` se construisent chacun seul, sauf pour ces fichiers. Chaque lien est
vérifié, et le workflow Windows tourne aussi quand l’un de ses fichiers partagés change.

| Fichier partagé | Utilisé par | Tenu à jour par |
|---|---|---|
| `macos/scripts/translations_fr.py` | Le français des deux apps : `Localizable.strings` et `desktop/app/src/i18n/fr.json` | `make i18n` ; `make i18n-check` échoue quand une chaîne n’a pas de français ou qu’un fichier est périmé |
| `scripts/make-emoji.py` | Les tables d’emoji des deux apps (`Emoji.swift`, `emoji.rs`) | `make i18n-check` |
| `site/public/site/tokens.css` | Le site, la documentation (`docs/.vitepress/theme`) et l’app Windows (`main.ts`) | `scripts/tokens-check.py` (dans `make format-check`) pour les copies claire et sombre de la documentation ; `Theme.swift` sur Mac le reprend à la main |
| `macos/Resources/Logos/` | Les logos des outils de l’app Windows (`Logo.svelte`) | Importés directement ; le serveur de dev Windows ne peut lire que ce dossier et `site/public/site` |
| `macos/Resources/Info.plist` | La version, posée pour les deux apps par `make version` et lue par le site | `scripts/version.py --check` dans le workflow de publication |
| Le nombre de tests dans les README et la doc Windows | Compté dans le code | `scripts/test-counts.py` (le hook de pré-commit et `make format-check`) |

## Construire et lancer

Demande macOS 14 ou ultérieur et Xcode 16 (Swift 6).

```sh
cd macos
make test                                  # unit tests
make run                                   # build build/Remora.app (release) and launch it
open build/Remora.app --args --demo        # sample data, as a window, nothing saved
open build/Remora.app --args --window      # your real inbox in a window
```

Copiez `build/Remora.app` dans `/Applications` pour la garder, et activez *Ouvrir à la connexion* dans les Réglages.

## Signer les compilations locales

Remora garde tous ses secrets dans **un seul** élément du trousseau (service `fr.igitscor.remora`, compte
`secrets`), lu une fois par lancement. macOS ne laisse une app lire son élément sans demander que s’il la
reconnaît : pour un certificat délivré par Apple, il retient l’**identifiant d’équipe**, qui survit aux
recompilations ; pour une compilation auto-signée ou ad hoc, l’empreinte du binaire, qui change à chaque
compilation. Sinon, comptez **une** demande par nouvelle compilation (*Toujours autoriser*).

`scripts/build-app.sh` signe avec, dans l’ordre : `REMORA_SIGN_IDENTITY`, un certificat *Apple Development*,
*Remora Local Signing* (créé par `scripts/make-signing-cert.sh`), puis ad hoc, toujours avec le runtime renforcé.

**Aucune demande :** obtenez un certificat Apple Development gratuit. Xcode → Réglages → Comptes → ajoutez votre
identifiant Apple → Gérer les certificats → + → Apple Development. Puis `make run`.

`REMORA_UNIVERSAL=1 ./scripts/build-app.sh` compile pour Apple Silicon et Intel, comme les versions publiées.

## Publier

Les versions sont des tags valables pour tout le dépôt, et le numéro de version vit dans les sources : le
`Info.plist` macOS, le workspace Rust et les fichiers de paquet de l’interface Windows. `make version` l’affiche ;
une publication tient en une commande et un commit :

```sh
make release V=0.3.2 DRY=1   # les vérifications seules
make release V=0.3.2
```

Elle vérifie d’abord, pour que rien ne soit tagué si quelque chose bloquerait la publication : sur `main`, rien
de non commité, à égalité avec `origin/main`, toutes les exécutions de la CI vertes sur le commit, `make deps-check`
propre (il faut `cargo-deny`), et des changements sous `## Unreleased` dans `CHANGELOG.md`. Puis un seul commit
`chore: version 0.3.2` contient la version partout, le titre daté du changelog et les pages du site reconstruites
(elles affichent la version) ; le tag signé `v0.3.2` et le commit sont poussés ensemble.

[`.github/workflows/release.yml`](https://github.com/iGitScor/triage/blob/main/.github/workflows/release.yml)
vérifie d’abord, en quelques minutes, ce qui bloquerait de toute façon la publication : le tag correspond à la
version des sources, aucune vulnérabilité connue ni problème de licence (cargo-deny), et les pages du site commitées
sont celles qu’écrit `site/build.py`. Puis il construit les deux apps en parallèle avec les
mêmes contrôles que la CI : l’app Mac est testée, reçoit un numéro de build (`CFBundleVersion`, le numéro
d’exécution), est compilée en Universal, signée, et empaquetée en `Remora.dmg` avec les licences (`make -C macos dmg` : dmgbuild, figé par
empreinte, dispose la fenêtre sur `macos/dmg/background.tiff`, rendu depuis `site/og/dmg.html` par
`site/og/render.sh dmg`) ; l’app Windows passe la vérification des traductions, la vérification de types, clippy et
les tests avant la construction de `Remora-Setup.exe` et `Remora.msi`. Pendant ce temps, un job `ci-passed` attend les exécutions de la CI sur le commit tagué (Format,
macOS, Windows et Linux, site, dépendances) et échoue si l’une n’a pas réussi : taguez un commit dont la CI est passée,
et ne poussez pas sur `main` avant qu’elle le soit (un nouveau push annule les exécutions en cours). Ce n’est que si
tout réussit qu’un dernier job publie la version GitHub, avec chaque
fichier, son SHA-256 et son SBOM (CycloneDX) d’un coup. Chaque job de build signe les attestations de provenance et
de SBOM du fichier qu’il a construit (`gh attestation verify`). Les noms ne changent jamais, donc
`releases/latest/download/Remora.dmg` sert toujours la plus récente.

### Signature des versions

Les secrets décident de ce qui est signé ; sans eux (forks), l’app Mac est signée ad hoc et l’installeur ne l’est
pas, et les notes de chaque version le disent.

- **macOS** : `scripts/make-signing-cert.sh --release` crée *Remora Release Signing* (20 ans) et affiche les
  commandes pour l’enregistrer comme `MACOS_SIGNING_P12` et `MACOS_SIGNING_PASSWORD`. Gardez ce certificat : le
  trousseau reconnaît Remora grâce à lui, et un nouveau coûte une demande de plus à chaque utilisateur. Il n’est
  pas notarisé : pour cela, voir
  [Déployer Remora](/fr/admin/deploiement#la-signer-avec-votre-developer-id).
- **Windows** : SignPath signe `remora.exe`, puis l’installeur reconstruit autour
  (`tauri bundle --no-binary-patching`, pour que l’app signée ne soit pas réécrite), chacun après une approbation
  dans SignPath. Il faut le secret `SIGNPATH_API_TOKEN`, la variable `SIGNPATH_ORGANIZATION_ID`, et dans SignPath
  le projet `remora`, la politique `release-signing` et la configuration d’artefact `exe`. Voir la
  [politique de signature du code](/fr/admin/signature-du-code).
- **Mises à jour** (à activer dans les apps) : chaque version publie `latest.json`, qui liste les plateformes dont
  le fichier a été signé pour la mise à jour. Mac : `swift scripts/updates/make-key.swift` écrit la clé publique
  dans `Info.plist` et la clé privée dans un dossier pour le secret `UPDATE_SIGNING_KEY` ; le DMG n’est signé que
  si le certificat de publication est aussi défini, car l’app n’installe une mise à jour que si elle est signée
  comme elle. Windows : `npx tauri signer generate`, la clé publique dans `tauri.conf.json`
  (`plugins.updater.pubkey`), la clé privée et son mot de passe comme `TAURI_SIGNING_PRIVATE_KEY` et
  `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` ; chaque signature est liée à sa version, et faite dans une tâche à part
  par la CLI autonome de Tauri (épinglée par somme de contrôle), loin des paquets npm du build. Gardez les deux
  clés privées hors ligne : qui en détient une peut livrer une mise à jour à tous ceux qui les ont activées, et une
  nouvelle clé oblige les utilisateurs à mettre à jour une fois à la main.

## Captures d’écran

Les captures du site, de cette documentation et du README viennent de l’app elle-même :

```sh
macos/scripts/screenshots.sh
```

Le script construit une copie de démo à part, ouvre chaque onglet (`--demo --tab …`) en anglais et en français,
en clair et en sombre, capture la fenêtre par son identifiant (sans rien cliquer ni taper), rogne la barre de titre
et écrit des PNG et WebP dans `site/public/site/screens/`. Il faut l’enregistrement de l’écran pour votre terminal,
un écran déverrouillé, et `cwebp` (`brew install webp`).

## Le site et cette documentation

Les deux sont servis depuis `triage.iscor.me` par un seul Cloudflare Worker, avec des fichiers statiques
uniquement (`site/wrangler.jsonc`). Le texte des pages est dans `site/copy/en.json` et `fr.json`, leur balisage
dans `site/templates/`, assemblés par `site/build.py` ; cette documentation est dans `docs/` (VitePress) :

Le workflow de publication les déploie après chaque version. Depuis l’ordinateur d’un mainteneur, en secours :

```sh
make deploy
```

Il lance `python3 site/build.py` et s’arrête si `site/public` diffère alors de ce qui est commité (commitez les pages,
ou publiez avec `make release`), puis `npm --prefix docs run build` (documentation → `site/public/docs`) et
`npx wrangler deploy`. Il n’écrit rien de commité. Les cartes pour les réseaux sociaux et l’icône tactile se rendent
à part, avec `make og`, après une modification de `site/og/card.*` ; commitez les PNG. Le workflow du site échoue
quand les pages commitées ne sont pas celles qu’écrit `site/build.py`.

`npm --prefix docs run dev` sert la documentation en local avec rechargement à chaud. `make a11y` construit la
documentation et vérifie chaque page avec axe, en clair et en sombre (WCAG 2.1 AA), et `make links` que chaque lien
interne, menu des langues compris, mène à une page, comme le workflow Website à chaque modification.

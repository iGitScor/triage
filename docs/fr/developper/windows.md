---
title: Remora pour Windows et Linux
description: Remora pour Windows et Linux (aperçu), une app de zone de notification Tauri 2 en Rust et Svelte. Ce qu’elle fait, ce que le Mac a et qu’elle n’a pas encore, sa correspondance avec l’app macOS, et comment la construire.
---

# Remora pour Windows et Linux

Statut : **aperçu**, pour les collègues sous Windows, et sous Linux (avec le soutien de la communauté). Une app de
zone de notification [Tauri 2](https://tauri.app) : un cœur en Rust et une interface Svelte sur WebView2 (WebKitGTK
sous Linux). Elle reprend les règles et les extensions de macOS avec les mêmes données de test et les mêmes tests,
pour que les apps se comportent pareil. Chaque version publiée joint `Remora-Setup.exe` (installation par
utilisateur, sans droits d’administrateur), `Remora-x86_64.AppImage` (qui se met à jour seule) et `Remora-amd64.deb`.

Sous Linux : les jetons dans le Secret Service (GNOME Keyring, KWallet, KeePassXC), les données dans
`~/.local/share/fr.igitscor.remora` lisibles par vous seul, la politique dans `/etc/remora/policy.json`, une fenêtre
ouverte depuis le menu de la zone de notification (ou d’elle-même là où le bureau n’en a pas), des notifications qui
ouvrent leur élément, et sous Wayland un raccourci du système qui lance `remora --reminder` au lieu de Ctrl+Alt+R.

| Partie | Contenu | Tests |
|---|---|---|
| `crates/remora_core` | Domaine et règles : verbes, À moi / En attente, priorisation, règles de Terminé et de report, détection des changements, horloge et conseiller de report (raisons, retours, constats), préparation des relectures, assistant d’attente, classement personnel, liens entre outils, données et politique de l’assistant, politique de conformité | 111 |
| `crates/remora_plugins` | GitHub, GitLab, Slack, Linear, Jira et Notion ; l’assistant : Claude par l’API ou par Claude Code, ou un serveur OpenAI-compatible ; le client HTTP limité aux adresses déclarées par chaque extension | 123 |
| `crates/remora_app` | Stockage, le coffre du Gestionnaire d’identification, la politique du registre, le service de boîte de réception que pilote l’app (historique, apprentissage, brief, résumés et tri de l’assistant), et ce qu’affiche la zone de notification | 80 |
| `app/src-tauri` | Dessine l’icône et la fenêtre surgissante, les notifications, le lancement avec Windows, le raccourci de rappel, les avatars, les mises à jour, les commandes : se contente d’appeler `remora_app` | 4 |
| `app/src` | L’interface Svelte, en anglais et en français | types vérifiés, 41 |

## Ce que Windows a, et ce qu’il n’a pas encore

| | macOS | Windows |
|---|---|---|
| Sources : GitHub, GitLab, Slack, Linear, Jira, Notion | ✓ | ✓ |
| L’assistant : brief, résumés ✦ des groupes, tri des éléments reportés, par l’API Claude, Claude Code ou un serveur OpenAI-compatible | ✓ | ✓ |
| Raisons de report, retours suggérés (sans les week-ends), constats (boucles, embouteillages, sujets proches, éléments sans nouvelles) | ✓ | ✓ |
| Préparation des relectures (estimation sans fichiers de verrouillage, fichiers, zones sensibles, votre rythme) et sessions de relecture | ✓ | ✓ |
| Assistant d’attente : relecteurs suggérés, relances rédigées à copier | ✓ | ✓ |
| Relecteurs et approbations sur les lignes | ✓ | ✓ |
| Classement personnel (ce que vous traitez vite) | ✓ | ✓ |
| Réactions Slack (✅ vers Terminés, 👀 vers En cours, vos questions en attente) | ✓ | ✓ |
| Liens entre outils (une PR et son ticket) | Clés de ticket, mots et embeddings sur l’appareil | Clés de ticket et mots : moins de liens |
| Tri des messages par le sens | Mots-clés, puis un modèle de langue sur l’appareil | Mots-clés seulement : sans correspondance, un message va dans *À répondre* |
| Mises à jour (sur option), hors ligne et reconnexion, politique de confidentialité | ✓ | ✓ |
| Boutons dans les notifications (Terminé, Reporter) | ✓ | Pas encore : un clic ouvre l’élément |
| Réglages gérés | Toutes les clés de [MDM](/fr/admin/mdm) | Toutes les clés : dans le registre sous Windows, dans `/etc/remora/policy.json` sous Linux |
| Taille du texte | Réglages → Général | La mise à l’échelle de Windows |
| Nouveau rappel | Glisser le poisson de la barre des menus | Un raccourci global, Ctrl+Alt+R : on ne peut pas glisser une icône de la zone de notification |
| Terminé, Reporter sur une ligne | Balayer, ou le clavier | Des boutons sur chaque ligne |

## Même modèle, équivalents Windows

| | macOS | Windows |
|---|---|---|
| Jetons | Trousseau, un seul élément | Gestionnaire d’identification, une seule entrée |
| Données | `~/Library/Application Support/Remora` | `%LOCALAPPDATA%\fr.igitscor.remora` |
| Politique gérée | Profil de configuration, domaine `fr.igitscor.remora` | `HKLM\SOFTWARE\Policies\Remora` (stratégie de groupe, Intune) |
| TLS | Confiance du système | `native-tls` : le magasin de certificats Windows, donc les certificats racines de l’entreprise fonctionnent |
| Rappels | Faire glisser le poisson de la barre des menus | Un raccourci global (on ne peut pas faire glisser les icônes de la zone de notification) |

## Comment tout s’assemble

```mermaid
flowchart LR
  UI["Interface (Svelte)<br/>app/src"] -- "commands" --> Shell["Tray app<br/>app/src-tauri"]
  Shell -- "inbox-changed" --> UI
  Shell --> App["remora_app<br/>inbox service, storage, vault, policy"]
  App --> Plugins["remora_plugins<br/>guarded HTTP client"]
  App --> Core["remora_core<br/>rules"]
  Plugins --> Core
```

L’app actualise toutes les quelques minutes, et vérifie toutes les 30 secondes les reports et rappels arrivés à
échéance (Windows ne sait pas programmer des notifications à l’avance comme macOS). Les appels réseau se font sans
garder le verrou de la boîte de réception, donc l’interface n’attend jamais un outil lent.

## Construire et tester

Demande Rust ([rustup](https://rustup.rs/), qui installe la version de `desktop/rust-toolchain.toml`) et Node 22.

```sh
cd desktop
cargo test                         # the library crates, on any OS
cd app && npm ci
npm run tauri dev -- -- --demo     # sample data in a window
npm run check && npm run build     # type-check and build the interface
node scripts/preview.mjs           # every screen, English and French, light and dark
npm run tauri build                # the installer (on Windows)
```

La CI lance la vérification de types, clippy avec les avertissements traités comme des erreurs, tous les tests et
la construction de l’installeur sur `windows-latest`.

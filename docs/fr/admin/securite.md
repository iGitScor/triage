---
title: Synthèse sécurité
description: Une page pour l’évaluation sécurité de Remora par un fournisseur. Architecture, données traitées et destinations, stockage et chiffrement, IA, contrôle par MDM ou stratégie de groupe, signature et intégrité, mises à jour, dépendances, signalement des vulnérabilités, et limites actuelles.
---

# Synthèse sécurité

Les réponses qu’un questionnaire sécurité fournisseur demande en général, sur une page. Chaque réponse renvoie à
la page qui donne le détail. Elle couvre l’app macOS et, quand elle diffère, l’app Windows (en aperçu).

## Architecture

| Question | Réponse |
|---|---|
| De quoi s’agit-il ? | Une app de barre des menus (macOS) ou de zone de notification (Windows) qui lit les outils de travail de l’utilisateur et montre ce qui l’attend. |
| Y a-t-il un serveur, un cloud ou un compte éditeur ? | Non. L’app parle directement aux outils que l’utilisateur connecte. Rien ne passe par un tiers. |
| Télémétrie, statistiques, rapports de plantage, vérification de mises à jour ? | Aucun, sauf une vérification des mises à jour une fois que l’utilisateur ou la DSI les active. L’app n’ouvre aucune connexion en dehors de celles listées dans [Flux de données](./flux-de-donnees). |
| Écrit-elle dans les outils ? | Non. Elle ne fait que lire. Une réponse préparée est copiée dans le presse-papiers, l’utilisateur la colle lui-même. |
| Code source | Public, GPL-3.0-or-later : [github.com/iGitScor/triage](https://github.com/iGitScor/triage). |

## Données traitées

| Question | Réponse |
|---|---|
| Quelles données lit-elle ? | Les pull et merge requests de l’utilisateur, ses demandes de relecture, ses mentions et messages directs Slack, ses tâches Linear et Notion : titres, auteurs, statuts, texte des messages, liens. Les chemins et nombres de lignes d’une modification, jamais le code. |
| Où vont-elles ? | Uniquement vers l’outil d’où elles viennent, et vers Anthropic si l’organisation autorise Claude et que l’utilisateur le connecte. Les hôtes de chaque outil sont déclarés dans l’app et imposés : tout autre hôte est refusé. Voir [Flux de données](./flux-de-donnees). |
| Identifiants | Les jetons de l’utilisateur, saisis par lui, gardés dans le trousseau (macOS) ou le Gestionnaire d’identification (Windows). Jamais écrits dans des fichiers ou des journaux, jamais placés dans une URL. |
| Données personnelles | Les noms et avatars des personnes présentes dans ces éléments. Rien n’est collecté sur l’utilisateur au-delà de ce que ses outils contiennent déjà. |

## Stockage et chiffrement

| Question | Réponse |
|---|---|
| Où sont stockées les données ? | Sur l’ordinateur uniquement. macOS : `~/Library/Application Support/Remora/`, un dossier que seul l’utilisateur peut ouvrir (fichiers `0600`). Windows : `%LOCALAPPDATA%\fr.igitscor.remora`. Liste complète dans [Flux de données](./flux-de-donnees#ce-qui-est-stocke-sur-l-ordinateur). |
| Chiffrées au repos ? | Par le chiffrement du disque (FileVault, BitLocker), et le trousseau ou le Gestionnaire d’identification pour les jetons. Les fichiers eux-mêmes sont du JSON en clair. |
| Sauvegardes | Sur macOS, le cache de la boîte, les briefs et les résumés sont exclus de Time Machine et d’iCloud. Les réglages, l’état des éléments et les rappels sont sauvegardés. |
| Cache HTTP | Aucun : les réponses ne sont jamais écrites sur le disque. |
| Suppression | Réglages → Confidentialité → **Effacer les données locales** supprime les fichiers, les jetons et les notifications. Déconnecter un compte supprime son jeton et ses éléments. |

## IA

| Question | Réponse |
|---|---|
| L’IA est-elle utilisée par défaut ? | Uniquement sur l’appareil : le framework NaturalLanguage d’Apple sur macOS, des règles de mots-clés sur Windows. Rien n’est envoyé. |
| IA générative | Claude est facultatif et **désactivé par défaut**. L’organisation peut l’interdire, lui cacher certains outils et limiter les modèles. [Ce qui parvient à Claude](./flux-de-donnees#ce-qui-parvient-a-claude) liste chaque champ envoyé. |
| Injection de prompt | Les éléments de la boîte sont envoyés comme des données balisées auxquelles Claude ne doit pas obéir ; ses réponses s’affichent en texte brut et n’agissent jamais d’elles-mêmes. |
| Claude Code | Remora n’exécute qu’un programme nommé `claude` que seul l’utilisateur (ou le système) peut modifier, vérifie que c’est Claude Code, et le lance sans outils, sans plugins et sans session enregistrée. |

## Contrôle d’accès

| Question | Réponse |
|---|---|
| La DSI peut-elle restreindre l’app ? | Oui. macOS : un profil de configuration (MDM). Windows : stratégie de groupe ou Intune, sous `HKLM\SOFTWARE\Policies\Remora`. Voir [Gérer la politique](./mdm). |
| Que peut-on imposer ? | Les outils autorisés, l’IA externe, les avatars. Sur macOS aussi les outils cachés à l’assistant, les modèles autorisés, le contenu masqué des notifications, la fréquence d’actualisation, l’ouverture dans les apps et le chemin de Claude Code. |
| Et si une valeur de la politique est erronée ? | Elle est appliquée le plus strictement possible : une liste illisible n’autorise rien, un interrupteur illisible est désactivé. |
| Transport | HTTPS uniquement, avec le TLS du système. Le HTTP en clair est refusé, sauf vers `localhost`. Les redirections vers un autre hôte sont refusées : un jeton ne les suit jamais. |

## Intégrité et mises à jour

| Question | Réponse |
|---|---|
| Comment l’app est-elle distribuée ? | Par GitHub Releases : `Remora.dmg` et `Remora-Setup.exe`, à des URL fixes. Voir [Déployer Remora](./deploiement). |
| Comment vérifier un téléchargement ? | Chaque fichier a son SHA-256, un SBOM CycloneDX et une provenance de build signée : `gh attestation verify Remora.dmg --repo iGitScor/triage` prouve qu’il a été construit par le workflow de publication à partir du source étiqueté. |
| Signature du code | **Pas encore** : l’app Mac est signée ad hoc et non notarisée, et l’installeur Windows n’est pas signé. L’utilisateur confirme le premier lancement. Vous pouvez construire et signer votre propre copie avec votre Developer ID. Ce qui sera signé, et par qui : [Politique de signature du code](./signature-du-code). |
| Mises à jour | Désactivées par défaut. Une fois activées (Réglages → Général, ou `AutomaticUpdates`), Remora interroge GitHub une fois par jour et installe une nouvelle version quand l’utilisateur le choisit. Chaque téléchargement doit porter la signature de la clé de publication pour cette version (Ed25519 sous macOS, minisign de Tauri sous Windows), et sous macOS la nouvelle app doit être signée par le même certificat. `AutomaticUpdates` à faux désactive entièrement les mises à jour, pour les parcs que vous redéployez vous-même. |
| Dépendances | L’app macOS n’utilise aucun paquet tiers. Les crates Rust et paquets npm de l’app Windows sont figés par des fichiers de verrouillage, vérifiés chaque semaine (vulnérabilités et licences) et listés dans son SBOM. |
| Tests | Chaque modification exécute en CI les tests des deux apps, dont les contrôles des destinations, des redirections, des liens et de la politique. |

## Signalement des vulnérabilités

Signalez-les en privé par [un avis de sécurité GitHub](https://github.com/iGitScor/triage/security/advisories/new) ;
la politique est dans [SECURITY.md](https://github.com/iGitScor/triage/blob/main/SECURITY.md). Les signalements
reçoivent une réponse sous quelques jours ouvrés ; les correctifs sortent dans une nouvelle version avec un avis
publié. Seule la dernière version reçoit les correctifs de sécurité.

## Limites actuelles

- Pas de signature Developer ID d’Apple ni de notarisation, et pas d’installeur Windows signé : Gatekeeper et
  SmartScreen avertissent au premier lancement, et macOS redemande l’accès au trousseau après chaque mise à jour.
- Mises à jour désactivées par défaut : tant que l’utilisateur ou la DSI ne les active pas, garder les postes à jour leur revient.
- Sur Windows, les fichiers locaux sont du JSON en clair lisible par le compte de l’utilisateur et les
  administrateurs, et la politique couvre moins de réglages que sur macOS.

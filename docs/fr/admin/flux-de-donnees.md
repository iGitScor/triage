---
title: Flux de données
description: Chaque flux réseau de Remora (GitHub, GitLab, Slack, Linear, Claude, avatars), ce que chacun envoie, ce qui reste sur le Mac et où, et comment c’est appliqué.
---

# Flux de données

Cette page liste chaque flux. Elle décrit l’app macOS ; l’app Windows suit le même modèle (adresses déclarées, client
contrôlé, politique) avec les équivalents Windows : Gestionnaire d’identification, `%LOCALAPPDATA%\fr.igitscor.remora`, et une
politique sous `HKLM\SOFTWARE\Policies\Remora` (stratégie de groupe ou Intune).

## Ce qui quitte le Mac

| Flux | Destination | Ce qui est envoyé | Par défaut |
|---|---|---|---|
| GitHub | `api.github.com`, `github.com`, `avatars.githubusercontent.com`, ou l’adresse GitHub Enterprise | Le jeton de l’utilisateur ; des lectures de ses pull requests, demandes de relecture, et des chemins et nombres de lignes des fichiers modifiés | Autorisé |
| GitLab | L’adresse GitLab configurée | Le jeton ; des lectures de ses merge requests, approbations, pipelines et chemins des fichiers modifiés. Le texte des diffs dans les réponses n’est jamais décodé ni stocké | Autorisé |
| Slack | `slack.com` | Le jeton utilisateur ; des recherches de ses mentions et messages directs | Autorisé |
| Linear | `api.linear.app`, `public.linear.app` | La clé d’API ; des lectures des tickets assignés et des notifications | Autorisé |
| Notion (pas encore disponible) | `api.notion.com` | Le jeton ; des requêtes de bases | Autorisé |
| Claude Code | Anthropic, via le programme local `claude` | Titres, contextes, auteurs et statuts des éléments (brief, résumés, tri) | **Désactivé** |
| API Claude | `api.anthropic.com` | Comme Claude Code | **Désactivé** |
| Avatars | Les hébergeurs d’images des outils autorisés et connectés seulement | Une requête d’image | Autorisé |

Les données des outils **reviennent** sur le Mac et y restent. **Pas de télémétrie, pas d’analytics, pas de rapports
de plantage**, pas de vérification de mise à jour, aucun autre accès réseau. Polices et logos sont intégrés à l’app,
jamais téléchargés. Remora n’écrit jamais dans les outils : les messages rédigés sont copiés dans le presse-papiers.

## Ce qui reste sur le Mac

| Données | Emplacement |
|---|---|
| Cache, états des éléments, rappels, préférences, historique des reports, briefs et résumés | Fichiers JSON dans `~/Library/Application Support/Remora/`, protégés par FileVault comme le reste des fichiers de l’utilisateur |
| Jetons | Un seul élément du trousseau de session (service `fr.igitscor.remora`, compte `secrets`) |
| Ce que l’utilisateur traite vite ou reporte | `learning.json`, pour le classement sur ce Mac seulement |
| Préparation des relectures | Chemins et nombres de lignes ; le code n’est jamais stocké |
| Notifications | Locales (`UNUserNotificationCenter`) |

**Apprentissage sur l’appareil** : le tri des messages et la comparaison des sujets utilisent le framework
NaturalLanguage d’Apple (plongements de phrases et de mots fournis avec macOS) ; le classement et les conseils de
report, de petites statistiques sur l’historique local. Rien n’est envoyé.

Réglages → Confidentialité → **Effacer les données locales…** supprime tout cela, jetons compris.

## Comment c’est appliqué

1. **Destinations déclarées.** Chaque extension déclare ses sorties (adresses, description, IA externe ou non) dans
   son manifeste. Un test vérifie que chacune le fait.
2. **Un seul passage.** L’app construit les extensions à un seul endroit. Elle refuse celles que la politique
   n’autorise pas, et donne à chacune un client HTTP limité à ses adresses déclarées plus celle du compte. Toute
   autre adresse échoue avec « Bloqué : … n’est pas une destination autorisée », domaines imitateurs compris
   (`api.linear.app.evil.com` est refusé) ; c’est testé.
3. **IA externe désactivée par défaut.** Les extensions Claude sont refusées tant que l’IA externe n’est pas
   autorisée ; brief, résumés et tri n’apparaissent qu’ensuite.
4. **Avatars** chargés seulement depuis les outils autorisés et connectés : ni Gravatar ni autre tiers.
5. **Réglages → Confidentialité** affiche les destinations de chaque compte connecté et s’il est autorisé.

L’organisation peut verrouiller tout cela : [Gérer la politique par MDM](./mdm).

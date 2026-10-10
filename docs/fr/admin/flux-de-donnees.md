---
title: Flux de données
description: Chaque flux réseau de Remora (GitHub, GitLab, Slack, Linear, Claude, avatars), ce que chacun envoie, exactement ce qui parvient à Claude, ce qui est stocké sur l’ordinateur et où, et comment c’est appliqué.
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
| Notion | `api.notion.com` | Le jeton ; l’identité de l’utilisateur, puis des requêtes des bases choisies pour les tâches qui lui sont assignées | Autorisé |
| Claude Code | Anthropic, via le programme local `claude` | Quelques champs de chaque élément : voir [Ce qui parvient à Claude](#ce-qui-parvient-a-claude) | **Désactivé** |
| API Claude | `api.anthropic.com` | La clé d’API, et les mêmes champs que Claude Code | **Désactivé** |
| Avatars | Les hébergeurs d’images des outils autorisés et connectés seulement | Une requête d’image | Autorisé |

Les données des outils **reviennent** sur le Mac et y restent. **Pas de télémétrie, pas d’analytics, pas de rapports
de plantage**, pas de vérification de mise à jour, aucun autre accès réseau. Polices et logos sont intégrés à l’app,
jamais téléchargés. Remora n’écrit jamais dans les outils : les messages rédigés sont copiés dans le presse-papiers.

## Ce qui parvient à Claude

Seulement si l’IA externe est autorisée et que l’utilisateur a connecté Claude. Pour chaque élément :

- son verbe (*À répondre*, *À relire*…), son **titre**, son contexte (dépôt, canal ou clé de ticket), son auteur, ses
  statuts (vérifications, approbations, taille du changement) et son âge, plus un identifiant interne pour que la
  réponse puisse renvoyer à l’élément ;
- pour un message **Slack**, le titre est la **première ligne du message** (la suite, affichée en aperçu dans la
  boîte, n’est pas envoyée) ;
- pour le **tri** de l’onglet Reportés, aussi la date de retour, la raison du report choisie par l’utilisateur
  (*En attente de quelqu’un*, *Pas motivé*…) et le nombre de reports.

Le brief envoie jusqu’à 120 éléments, un résumé de groupe les éléments de ce groupe, le tri jusqu’à 80 éléments
reportés. Jamais envoyés : les jetons, les liens, le code, la suite d’un message, les notes, les fichiers.

- **À la connexion**, Claude rédige un premier brief de la boîte, qui sert aussi de test. Quand le *Brief de toute
  la boîte* est désactivé (Réglages → Général → Assistant), le test utilise un seul élément fictif : aucun contenu
  de la boîte ne quitte l’ordinateur avant que l’utilisateur demande un résumé.
- Les éléments sont écrits par d’autres, donc un message pourrait essayer de donner des ordres à Claude. Remora
  les envoie comme un bloc de données délimité et demande à Claude de ne jamais suivre les instructions qu’ils
  contiennent ; les réponses s’affichent en texte brut, et les suggestions de tri qui masquent ou ouvrent des
  éléments ne sont jamais cochées d’avance.
- **Masquer le contenu des messages** (Réglages → Général → Notifications) ne s’applique qu’aux notifications :
- Avec Claude Code, Remora lance `claude` sans outils, sans extensions, hooks ni serveurs MCP, et sans rien
  ligne de commande, que d’autres processus peuvent lire. Les données vont là où ce Claude Code est configuré pour les envoyer
  (normalement Anthropic). Réglages → Confidentialité indique quel `claude` est lancé.

## Ce qui est stocké sur l’ordinateur

### macOS

| Données | Emplacement |
|---|---|
| Les comptes (noms, adresses, réglages ; pas de jetons) | `accounts.json` |
| La dernière copie des éléments de chaque outil : titres, contextes, auteurs, statuts, texte des messages Slack, liens | `cache.json` |
| Terminé, Épingler, les reports et leurs notes, les rappels | `states.json`, `reminders.json`, `snooze-history.json` |
| Ce que l’utilisateur traite vite ou reporte, pour le classement sur ce Mac seulement (mots des titres, auteurs) | `learning.json` |
| Le dernier brief et les résumés de groupes | `brief.json`, `bundle-summaries.json` |
| Les préférences | `preferences.json` |
| Jetons et clés d’API | Un seul élément du trousseau de session : service `fr.igitscor.remora`, compte `secrets` |
| Rappels programmés et notifications reçues (titres) | Les notifications de macOS (`UNUserNotificationCenter`) |

résumés sont exclus des sauvegardes Time Machine et iCloud, puisque Remora peut les récupérer à nouveau. La préparation des relectures
ne garde que des chemins et des nombres de lignes ; le code n’est jamais stocké. Remora ne garde **aucun cache
HTTP** : les requêtes ne sont pas écrites sur le disque, et le cache laissé par les versions précédentes dans
`~/Library/Caches/fr.igitscor.remora` et `~/Library/HTTPStorages` est supprimé au lancement.

Réglages → Confidentialité → **Effacer les données locales…** supprime tout cela : les fichiers JSON, tous les
éléments du trousseau enregistrés par Remora (ou par son ancien nom, Perch), les notifications programmées et
reçues, et l’ancien cache HTTP. Déconnecter un compte retire son jeton, ses éléments et ses notifications.

### Windows

| Données | Emplacement |
|---|---|
| Les comptes, la dernière copie des éléments de chaque outil (comme sur macOS, texte des messages Slack compris), les états, les rappels, les préférences | `accounts.json`, `cache.json`, `states.json`, `reminders.json`, `preferences.json` dans `%LOCALAPPDATA%\fr.igitscor.remora` |
| Jetons | Une entrée du Gestionnaire d’identification : `fr.igitscor.remora`, utilisateur `secrets` |

Les fichiers sont du JSON en clair, lisibles par le compte Windows de l’utilisateur (et les administrateurs) ;
BitLocker les chiffre avec le reste du disque. Réglages → Confidentialité → **Effacer les données locales**
supprime les fichiers et les jetons ; les notifications déjà dans le centre de notifications y restent jusqu’à ce
qu’elles soient fermées. L’option « supprimer les données de l’app » du désinstalleur retire aussi le dossier.

**Apprentissage sur l’appareil** : sur macOS, le tri des messages et la comparaison des sujets utilisent le
framework NaturalLanguage d’Apple (plongements de phrases et de mots fournis avec macOS) ; le classement et les
conseils de report, de petites statistiques sur l’historique local. L’app Windows trie avec des règles de mots-clés
seulement. Rien n’est envoyé.

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

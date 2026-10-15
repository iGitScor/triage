---
title: Flux de données
description: Chaque flux réseau de Remora, ce qu’il envoie, ce qui parvient à l’assistant, ce qui est stocké sur l’ordinateur, et comment c’est appliqué.
---

# Flux de données

Cette page liste chaque flux, pour les deux apps : elles ont les mêmes flux, adresses déclarées, client contrôlé et
politique. Quand Windows diffère, elle le dit ; ses équivalents sont le Gestionnaire d’identification,
`%LOCALAPPDATA%\fr.igitscor.remora`, et une politique sous `HKLM\SOFTWARE\Policies\Remora` (stratégie de groupe ou
Intune).

## Ce qui quitte l’ordinateur

| Flux | Destination | Ce qui est envoyé | Par défaut |
|---|---|---|---|
| GitHub | `api.github.com`, `github.com`, `avatars.githubusercontent.com`, ou l’adresse GitHub Enterprise | Le jeton de l’utilisateur ; une lecture de ses pull requests, demandes de relecture et tickets assignés, avec les chemins et nombres de lignes des fichiers modifiés | Autorisé |
| GitLab | L’adresse GitLab configurée | Le jeton ; des lectures de ses merge requests et de ses To-Dos en attente, puis une requête GraphQL pour leurs approbations, pipelines, et les chemins et nombres de lignes des fichiers modifiés. Le texte des diffs dans les réponses n’est jamais décodé ni stocké | Autorisé |
| Azure DevOps | L’organisation configurée (`dev.azure.com/…`) ou la collection Azure DevOps Server | Le jeton d’accès personnel ; son identité (`connectionData`), des lectures des pull requests actives qu’il a créées ou relit, de leurs évaluations de stratégie, des chemins des fichiers modifiés par le dernier push de celles qu’il relit, et une requête WIQL puis une lecture des éléments de travail ouverts qui lui sont assignés. Aucun contenu de fichier n’est lu | Autorisé |
| Slack | `slack.com` | Le jeton utilisateur ; des recherches de ses mentions, messages directs, messages dans des fils et derniers messages, puis les réponses d’au plus 10 de ces fils (`conversations.replies`, permissions d’historique en lecture seule) ; les messages auxquels l’utilisateur a réagi (`reactions.list`), les réactions d’au plus 15 messages récents (`reactions.get`, chacune gardée 10 minutes) et les emoji propres à l’espace de travail (`emoji.list`, une fois par jour) | Autorisé |
| Linear | `api.linear.app`, `public.linear.app` | La clé d’API ; des lectures des tickets assignés et des notifications | Autorisé |
| Bitbucket | `api.bitbucket.org` (Cloud), ou l’hôte Data Center indiqué | Le jeton d’API avec l’e-mail de l’utilisateur (Cloud), ou un jeton d’accès HTTP (Data Center) ; l’identité de l’utilisateur, ses pull requests ouvertes et celles qu’il relit (Cloud : cherchées dans les 25 dépôts de l’espace de travail mis à jour le plus récemment ; Data Center : le tableau de bord), et l’état des builds de chacune. Ni code ni diff n’est lu | Autorisé |
| Jira | L’hôte Jira indiqué (Cloud : `*.atlassian.net` ; ou un hôte Data Center) | Le jeton d’API avec l’e-mail de l’utilisateur (Cloud), ou un jeton d’accès personnel (Data Center) ; l’identité de l’utilisateur, une recherche des tickets qui lui sont assignés, et une des commentaires des 7 derniers jours sur les tickets qu’il suit, lus pour y trouver ses mentions ; sur Cloud, si l’utilisateur active *Mentions dans Confluence*, une recherche Confluence sur le même site (`/wiki/rest/api/search`, CQL `mention = currentUser()`) des pages et commentaires des 7 derniers jours qui le mentionnent | Autorisé |
| Microsoft 365 | `login.microsoftonline.com`, `graph.microsoft.com` | Une connexion dans le navigateur au tenant de l’organisation, avec l’app qu’elle a inscrite (`MicrosoftTenant`, `MicrosoftClientID`) ; puis un jeton de renouvellement, gardé dans le trousseau ou le coffre, renouvelle un jeton d’accès de courte durée. Avec lui, des lectures Graph : l’identité de l’utilisateur, ses courriels signalés, les courriels non lus de sa boîte des 7 derniers jours (seuls ceux qui lui sont adressés sont gardés), les invitations des 14 prochains jours sans réponse, et le dernier message de ses conversations Teams | Autorisé |
| Notion | `api.notion.com` | Le jeton ; l’identité de l’utilisateur, les propriétés des bases choisies (pour écarter les tâches terminées), puis des requêtes de ces bases pour les tâches ouvertes qui lui sont assignées | Autorisé |
| Claude Code | Anthropic, via le programme local `claude` | Quelques champs de chaque élément : voir [Ce qui parvient à l’assistant](#ce-qui-parvient-a-claude) | **Désactivé** |
| API Claude | `api.anthropic.com` | La clé d’API, et les mêmes champs que Claude Code | **Désactivé** |
| OpenAI-compatible | Le serveur choisi dans le compte ou par `AIServer` : OpenAI, Azure OpenAI, Mistral…, ou un serveur sur cet ordinateur | La clé d’API s’il y en a une, et les mêmes champs que Claude Code. Les conditions du fournisseur (conservation, entraînement) s’appliquent à ce qu’il reçoit | **Désactivé** ; un serveur sur cet ordinateur est autorisé sauf si l’organisation le refuse (`AllowLocalAI`) |
| Avatars | Les hébergeurs d’images des outils autorisés et connectés seulement, en https ; les avatars GitLab, Jira Cloud et Bitbucket Cloud aussi depuis Gravatar (`secure.gravatar.com`, `www.gravatar.com`) et les images par défaut d’Atlassian (`*.atl-paas.net`), et les emoji propres à un espace Slack depuis `emoji.slack-edge.com` | Une requête d’image, jamais avec un jeton | Autorisé |
| Mises à jour | `github.com`, et les serveurs de téléchargement de GitHub (`*.githubusercontent.com`) | Une requête de `latest.json` par jour, et la nouvelle version quand l’utilisateur l’installe. Aucun identifiant, jeton ni contenu | **Désactivé** (`AutomaticUpdates`) |

Les données des outils **reviennent** sur l’ordinateur et y restent. **Pas de télémétrie, pas d’analytics, pas de rapports
de plantage**, pas de vérification de mise à jour sauf si elles sont activées, aucun autre accès réseau. Polices et logos sont intégrés à l’app,
jamais téléchargés. Remora n’écrit jamais dans les outils : les messages rédigés sont copiés dans le presse-papiers.

## Ce qui parvient à l’assistant {#ce-qui-parvient-a-claude}

Seulement si l’utilisateur a connecté un assistant et que la politique l’autorise : l’IA externe pour Claude ou un
serveur OpenAI-compatible ailleurs, `AllowLocalAI` pour un serveur sur cet ordinateur. Claude et les serveurs
OpenAI-compatibles reçoivent les mêmes champs. Pour chaque élément :

- son verbe (*À répondre*, *À relire*…), son **titre**, son contexte (dépôt, canal ou clé de ticket), son auteur, ses
  statuts (vérifications, approbations, taille du changement) et son âge, plus un identifiant interne pour que la
  réponse puisse renvoyer à l’élément ;
- pour un élément **Slack**, ce que ses réactions veulent dire et combien (« you reacted: done », « others reacted:
  ack ×2 »), jamais qui a réagi ;
- pour un message **Slack**, le titre est la **première ligne du message** (la suite, affichée en aperçu dans la
  boîte, n’est pas envoyée) ;
- pour le **tri** de l’onglet Reportés, aussi la date de retour, la raison du report choisie par l’utilisateur
  (*En attente de quelqu’un*, *Pas motivé*…) et le nombre de reports.

Le brief envoie jusqu’à 120 éléments, un résumé de groupe les éléments de ce groupe, le tri jusqu’à 80 éléments
reportés. Jamais envoyés : les jetons, les liens, le code, la suite d’un message, les notes, les fichiers.

- **À la connexion**, l’assistant rédige un premier brief de la boîte, qui sert aussi de test. Quand le *Brief de toute
  la boîte* est désactivé (Réglages → Général → Assistant), le test utilise un seul élément fictif : aucun contenu
  de la boîte ne quitte l’ordinateur avant que l’utilisateur demande un résumé.
- Les éléments sont écrits par d’autres, donc un message pourrait essayer de donner des ordres à l’assistant. Remora
  les envoie comme un bloc de données délimité et demande au modèle de ne jamais suivre les instructions qu’ils
  contiennent ; les réponses s’affichent en texte brut, et les suggestions de tri qui masquent ou ouvrent des
  éléments ne sont jamais cochées d’avance.
- **Envoyer à l’assistant** (Réglages → Général) tient les éléments d’un outil entièrement à l’écart de
  l’assistant ; l’organisation peut faire de même et limiter les modèles avec des
  [clés MDM](./mdm#les-cles) (`AIExcludedSources`, `AllowedAIModels`).
- Avec l’API Claude, si le modèle choisi refuse un brief ou un tri, Anthropic peut répondre avec un autre de ses
  modèles (repli côté serveur). Remora ne le demande qu’avec Opus, Sonnet et Fable, et jamais quand `AllowedAIModels`
  est réglée : la réponse vient alors toujours d’un modèle autorisé, ou échoue.
- **Masquer le contenu des messages** (Réglages → Général → Notifications) ne s’applique qu’aux notifications et à
  la tâche en cours du menu de la barre des menus ou de la zone de notification :
  les titres de cet outil peuvent toujours parvenir à l’assistant : c’est le rôle d’*Envoyer à l’assistant*.
- Avec Claude Code, Remora lance `claude` sans outils, sans extensions, hooks ni serveurs MCP, et sans rien
  enregistrer dans l’historique de sessions, avec un effort faible comme par l’API. Le contenu de la boîte passe par son entrée standard, jamais par la
  ligne de commande, que d’autres processus peuvent lire. Les données vont là où ce Claude Code est configuré pour les envoyer
  (normalement Anthropic). Sur macOS, Réglages → Confidentialité indique quel `claude` est lancé.

## Ce qui est stocké sur l’ordinateur

### macOS

| Données | Emplacement |
|---|---|
| Les comptes (noms, adresses, réglages ; pas de jetons) | `accounts.json` |
| La dernière copie des éléments de chaque outil : titres, contextes, auteurs, statuts, texte des messages Slack, liens | `cache.json` |
| Terminé, Épingler, les reports et leurs notes, les rappels | `states.json`, `reminders.json`, `snooze-history.json` |
| Ce que l’utilisateur traite vite ou reporte, pour le classement sur ce Mac seulement (mots des titres, auteurs) | `learning.json` |
| La durée des relectures chronométrées face à leur estimation (deux nombres chacune, ni nom ni titre) | `review-times.json` |
| Le dernier brief et les résumés de groupes | `brief.json`, `bundle-summaries.json` |
| Les préférences | `preferences.json` |
| Jetons et clés d’API | Un seul élément du trousseau de session : service `fr.igitscor.remora`, compte `secrets` |
| Rappels programmés et notifications reçues (titres) | Les notifications de macOS (`UNUserNotificationCenter`) |
| Seulement si l’utilisateur active Réglages → Confidentialité → **Retrouver les éléments de la boîte dans Spotlight** (désactivé par défaut) : le titre de chaque élément, son outil et son contexte, jamais les aperçus, les personnes ni les liens ; rien d’un outil dont le contenu est masqué | L’index Spotlight de ce Mac (Core Spotlight) |

Les fichiers JSON sont dans `~/Library/Application Support/Remora/`, un dossier que seul l’utilisateur peut ouvrir
(0700, fichiers 0600) ; FileVault les chiffre avec le reste du disque. Le cache de la boîte, les briefs et les
résumés sont exclus des sauvegardes Time Machine et iCloud, puisque Remora peut les récupérer à nouveau. La préparation des relectures
ne garde que des chemins et des nombres de lignes ; le code n’est jamais stocké. Remora ne garde **aucun cache
HTTP** : les requêtes ne sont pas écrites sur le disque, et le cache laissé par les versions précédentes dans
`~/Library/Caches/fr.igitscor.remora` et `~/Library/HTTPStorages` est supprimé au lancement.

Réglages → Confidentialité → **Effacer les données locales…** supprime tout cela : les fichiers JSON, tous les
éléments du trousseau enregistrés par Remora (ou par son ancien nom, Perch), les notifications programmées et
reçues, ce qu’il a mis dans Spotlight, et l’ancien cache HTTP. Un fichier qu’il ne peut pas supprimer est nommé, avec son dossier, au lieu d’être
compté comme effacé. Déconnecter un compte retire son jeton, ses éléments, ses notifications
et ses entrées Spotlight ; de même pour un outil que la politique bloque ou dont le contenu est masqué.

### Windows

| Données | Emplacement |
|---|---|
| Les mêmes données que sur macOS : les comptes, la dernière copie des éléments de chaque outil (texte des messages Slack compris), les états, les rappels, l’historique des reports, ce que l’utilisateur traite vite, la durée des relectures, le dernier brief et les résumés, les préférences | `accounts.json`, `cache.json`, `states.json`, `reminders.json`, `snooze-history.json`, `learning.json`, `review-times.json`, `brief.json`, `bundle-summaries.json`, `preferences.json` dans `%LOCALAPPDATA%\fr.igitscor.remora` |
| Jetons | Une entrée du Gestionnaire d’identification par compte : `fr.igitscor.remora`, utilisateur l’identifiant du compte, et un index des identifiants (utilisateur `accounts`) |

Les fichiers sont du JSON en clair, lisibles par le compte Windows de l’utilisateur (et les administrateurs) ;
BitLocker les chiffre avec le reste du disque. Réglages → Confidentialité → **Effacer les données locales**
supprime les fichiers et les jetons ; les notifications déjà dans le centre de notifications y restent jusqu’à ce
qu’elles soient fermées. L’option « supprimer les données de l’app » du désinstalleur retire aussi le dossier.

### Linux (aperçu)

Les mêmes fichiers que sous Windows, dans `$XDG_DATA_HOME/fr.igitscor.remora` (en général `~/.local/share`), un
dossier lisible par l’utilisateur seul (`0700`, fichiers `0600`). Les jetons sont dans le Secret Service (GNOME
Keyring, KWallet ou KeePassXC), mêmes entrées que sous Windows, qui les garde chiffrés au repos ;
sans trousseau actif, Remora le dit au lieu de démarrer sans jetons. Politique :
[`/etc/remora/policy.json`](./mdm#linux-un-fichier-de-politique).

**Apprentissage sur l’appareil** : sur macOS, le tri des messages et la comparaison des sujets utilisent le
framework NaturalLanguage d’Apple (plongements de phrases et de mots fournis avec macOS) ; le classement et les
conseils de report, de petites statistiques sur l’historique local. L’app Windows et Linux trie avec des règles de mots-clés
seulement. Rien n’est envoyé.

**La commande `remora`** ([Liens et automatisation](../guide/liens#la-commande-remora)) lit le compte et les titres,
contextes et liens des éléments dans l’app ouverte, et peut enregistrer un rappel. Elle passe par un socket que seul
l’utilisateur peut ouvrir (`remora.sock`, `0600`, dans le dossier temporaire de l’utilisateur sur macOS et
`$XDG_RUNTIME_DIR` sous Linux ; sous Windows un tube nommé `\\.\pipe\remora-<utilisateur>` accessible au seul
utilisateur, qui refuse les clients distants), jamais le réseau. Les titres des outils sous *Masquer le contenu des
messages* (ceux de l’utilisateur ou `HiddenContentSources`) en sont exclus ; après l’effacement, elle trouve une boîte
vide.

## Comment c’est appliqué

1. **Destinations déclarées.** Chaque extension déclare ses sorties (adresses, description, IA externe ou non) dans
   son manifeste. Un test vérifie que chacune le fait.
2. **Un seul passage.** L’app construit les extensions à un seul endroit. Elle refuse celles que la politique
   n’autorise pas, et donne à chacune un client HTTP limité à ses adresses déclarées plus celle du compte. Toute
   autre adresse échoue avec « Bloqué : … n’est pas une destination autorisée », domaines imitateurs compris
   (`api.linear.app.evil.com` est refusé) ; c’est testé.
3. **IA externe désactivée par défaut.** Un assistant qui envoie du contenu hors de l’ordinateur (Claude, ou un
   serveur OpenAI-compatible ailleurs) est refusé tant que l’IA externe n’est pas autorisée ; brief, résumés et tri
   n’apparaissent qu’ensuite. Un serveur sur cet ordinateur suit `AllowLocalAI` à la place.
4. **Avatars** chargés seulement depuis les outils autorisés et connectés, les images Gravatar de GitLab, de Jira et de
   Bitbucket, les images par défaut d’Atlassian et les emoji des espaces Slack : aucun autre tiers. Une redirection
   vers un autre hôte est refusée, et les images restent en mémoire seulement, jamais dans un cache sur disque, sans
   cookies.
5. **Réglages → Confidentialité** affiche les destinations de chaque compte connecté et s’il est autorisé.

L’organisation peut verrouiller tout cela : [Gérer la politique par MDM](./mdm).

## Ajouter une extension

Une nouvelle extension doit déclarer ses sorties ; rien d’autre n’est nécessaire pour l’application des règles. Le
traitement sur l’appareil est préféré à tout service externe. Voir [Écrire une extension](/fr/developper/extensions).

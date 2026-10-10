---
title: Connecter vos outils
description: Pas à pas, connecter GitHub (et Enterprise), GitLab (et auto-hébergé), Slack et Linear à Remora, et Claude en option. Quel jeton, quelle permission, et ce que Remora lit.
---

# Connecter vos outils

Ouvrez la boîte, puis la roue dentée → **Réglages** → **Sources**. Chaque outil a un bouton qui ouvre la bonne page
pour créer un jeton, et les étapes à suivre. Les jetons sont gardés dans le trousseau de votre Mac, jamais dans un
fichier.

Remora ne fait que lire. Elle ne publie, ne commente, n’approuve et ne modifie jamais rien dans vos outils. Ce que
chaque connexion peut joindre est listé dans [Flux de données](/fr/admin/flux-de-donnees).

## GitHub

Affiche les pull requests que vous avez ouvertes et les relectures qu’on vous demande. Une demande de relecture
quitte la boîte une fois que vous avez relu, comme sur GitHub.

1. Réglages → Sources → **GitHub**. Gardez `https://github.com`, ou saisissez l’adresse de votre GitHub
   Enterprise.
2. Cliquez sur **Créer un jeton** : GitHub ouvre la page des jetons classiques avec `repo` et `read:org` déjà
   cochés. Choisissez une expiration, générez, copiez.
3. Collez le jeton dans Remora et cliquez sur **Connecter**.

::: info Organisations avec SSO
Si votre organisation utilise l’authentification unique SAML, cliquez sur **Configure SSO** à côté du jeton sur
GitHub et autorisez-le pour l’organisation, sinon ses dépôts restent invisibles.
:::

## GitLab

Affiche vos merge requests, celles que vous relisez, leurs approbations et pipelines.

1. Réglages → Sources → **GitLab**, avec `https://gitlab.com` ou l’adresse de votre GitLab. Elle doit être en
   https : Remora n’envoie pas de jeton en http simple.
2. Créez un jeton d’accès personnel avec la portée `read_api` (GitLab → votre avatar → Modifier le profil → Jetons
   d’accès).
3. Collez-le et cliquez sur **Connecter**.

*Modifications demandées* nécessite GitLab 16.10 ou ultérieur ; les versions plus anciennes affichent tout le reste.

## Slack

Affiche les mentions, messages directs et réponses dans vos fils des derniers jours. Une mention est un élément
par message ; une conversation directe est un seul élément, donc un nouveau message la fait revenir même après
l’avoir terminée. Un fil où vous avez écrit est aussi un élément, quand quelqu’un a répondu après vous, même sans
vous mentionner (les 10 fils les plus récents).

1. Réglages → Sources → Slack → **Créer l’app Slack**. Slack s’ouvre avec une app nommée Remora, déjà configurée
   avec les permissions en lecture seule nécessaires, en votre nom : `search:read`, ainsi que `channels:history`
   et `groups:history` pour lire les fils où vous avez écrit.
2. Choisissez votre espace de travail, puis **Next** → **Create**.
3. Sur la page de l’app : **Install App** → *Install to your workspace* → **Allow**.
4. Copiez le **User OAuth Token** (il commence par `xoxp-`) et collez-le dans Remora.

::: warning « Request to install »
Certains espaces de travail exigent qu’un administrateur approuve les nouvelles apps. Slack affiche alors
**Request to install** : une fois l’app approuvée, reprenez à l’étape 3.
:::

Les éléments Slack s’ouvrent dans l’appli Slack, sur la conversation. ⌥-clic ouvre le message exact sur le web.

Une app créée avant les réponses dans les fils n’a que `search:read` : les Réglages l’indiquent à côté du compte.
Ajoutez les deux permissions d’historique à l’app (*OAuth & Permissions* → *User Token Scopes*), réinstallez-la et
connectez à nouveau le nouveau jeton : le compte est mis à jour, rien n’est perdu.

## Linear

Affiche les tickets qui vous sont assignés (ni terminés ni annulés), avec leur priorité et leur échéance, plus les
mentions et commentaires non lus de votre boîte Linear.

1. Réglages → Sources → Linear → **Créer une clé API** : Linear ouvre Settings → Security & access.
2. Sous **Personal API keys**, créez une clé nommée Remora. La lecture suffit.
3. Collez-la et cliquez sur **Connecter**.

Les tickets *Urgent* et *High* sont signalés ; *Low* et le Backlog sont affichés mais pas comptés. Les tickets en
retard ou à rendre aujourd’hui déclenchent une notification.

## Notion

Les tâches ouvertes qui vous sont assignées, dans les bases Notion que vous choisissez (sur Mac).

1. Réglages → Sources → **Notion** → **Créer un jeton** : dans Notion, **New token**, nommez-le Remora, gardez la
   capacité *Notion API*, choisissez une expiration. Copiez-le (Notion ne l’affiche qu’une fois) et collez-le.
2. Collez les liens de vos bases de tâches, séparés par des virgules (ouvrez la base en pleine page, puis ••• →
   Copier le lien). Le jeton voit ce que vous voyez : rien à partager.
3. Si vos tâches utilisent d’autres noms, réglez la **Propriété « assigné à »** (la propriété personnes qui dit
   à qui est la tâche) et les **statuts terminés**. Une tâche est terminée quand son *Statut* en fait partie, ou
   quand une case nommée *Done* ou *Complete* est cochée.

Pas de bouton **New token** ? Votre espace ne laisse que les propriétaires créer des jetons : demandez à l’un
d’eux, ou collez le secret d’une connexion interne créée par un propriétaire, et ajoutez **votre e-mail Notion**
pour que Remora trouve vos tâches.

## Claude (facultatif)

Claude ajoute un court brief de quoi traiter d’abord, des résumés en deux phrases des listes chargées, et une aide
pour trier ce que vous reportez sans cesse. Il reste désactivé tant que l’IA externe n’est pas autorisée, dans
Réglages → Confidentialité ou par votre organisation. Voir [L’assistant](./assistant).

- **Claude Code** (recommandé) : utilise le Claude Code installé et connecté sur votre Mac, sur votre abonnement
  Claude. Réglages → Sources, section *Assistant* → Claude Code → **Connecter**. L’installeur d’Anthropic est le
  plus simple : il n’a pas besoin de Node.js. Les installations par Homebrew, npm (nvm, volta), bun, mise ou asdf
  sont trouvées aussi ; sinon, collez le résultat de `which claude` dans *Chemin de claude*. Réglages →
  Confidentialité indique quel `claude` Remora lance.
- **API Claude** : une clé d’API de la Claude Console. Réglages → Sources, section *Assistant* → Claude API.

## Plusieurs comptes

Chaque outil peut être connecté plusieurs fois : deux espaces Slack, GitHub et GitHub Enterprise… Nommez chaque
compte en le connectant (« GitLab boulot », « Slack client »), ou renommez-le ensuite avec le crayon. Quand
plusieurs comptes d’un même outil sont connectés, les éléments affichent le nom du compte.

Chaque liste contient les 50 éléments les plus récents. Quand un outil en a davantage (60 demandes de relecture,
par exemple), un ⓘ en bas de la boîte et une ligne à côté du compte dans les Réglages le signalent : le reste est
dans l’outil.

## En cas de problème

Le bas de la boîte et le compte dans Réglages affichent l’erreur ; les derniers éléments restent visibles en
attendant.

| Vous voyez | Que faire |
|---|---|
| Une erreur d’authentification | Le jeton a expiré ou a été révoqué : créez-en un nouveau et collez-le avec le crayon. |
| *Bloqué : … n’est pas une destination autorisée.* | Remora a refusé de joindre une adresse que l’outil ne déclare pas. Vérifiez l’adresse saisie. |
| Rien d’une organisation GitHub | Autorisez le jeton pour le SSO (voir plus haut). |
| *Non autorisé par votre politique de confidentialité.* | L’outil est désactivé dans Réglages → Confidentialité, ou par votre organisation : demandez à votre DSI. |

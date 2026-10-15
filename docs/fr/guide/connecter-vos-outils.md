---
title: Connecter vos outils
description: Connecter GitHub, GitLab, Slack, Linear, Jira, Notion et Microsoft 365 à Remora, et l’assistant. Quel jeton, quelle permission, et ce que Remora lit.
---

# Connecter vos outils

Ouvrez la boîte, puis la roue dentée → **Réglages** → **Sources**. Chaque outil a un bouton qui ouvre la bonne page
pour créer un jeton, et les étapes à suivre. Les jetons sont gardés dans le trousseau de votre Mac ou le Gestionnaire
d’identification de Windows, jamais dans un fichier.

Remora ne fait que lire. Elle ne publie, ne commente, n’approuve et ne modifie jamais rien dans vos outils. Ce que
chaque connexion peut joindre est listé dans [Flux de données](/fr/admin/flux-de-donnees).

## GitHub

Affiche les pull requests que vous avez ouvertes, les relectures qu’on vous demande, et les tickets ouverts qui
vous sont assignés (dans *À faire*). Une demande de relecture quitte la boîte une fois que vous avez relu, comme sur
GitHub.

1. Réglages → Sources → **GitHub**. Gardez `https://github.com`, ou saisissez l’adresse de votre GitHub
   Enterprise.
2. Cliquez sur **Créer un jeton** : GitHub ouvre un **jeton à granularité fine** nommé Remora. Choisissez son
   propriétaire (vous, ou votre organisation) et **All repositories**, et donnez-lui **Pull requests** et **Issues** en
   lecture seule (GitHub ajoute *Metadata*) ; Remora ne demande rien d’autre et n’écrit rien. Choisissez une
   expiration, générez, copiez.
3. Collez le jeton dans Remora et cliquez sur **Connecter**.

Un jeton à granularité fine voit les dépôts d’un seul propriétaire : pour ceux de plusieurs organisations, connectez
GitHub une fois par organisation, ou utilisez plutôt un **jeton classique** avec `repo` et `read:org` (plus large, et il
peut écrire). Une organisation peut devoir approuver un jeton à granularité fine avant qu’il fonctionne.

::: info Organisations avec SSO
Si votre organisation utilise l’authentification unique SAML, cliquez sur **Configure SSO** à côté du jeton sur
GitHub et autorisez-le pour l’organisation, sinon ses dépôts restent invisibles.
:::

## GitLab

Affiche vos merge requests, celles que vous relisez, leurs approbations et pipelines, et vos autres To-Dos : un
ticket qui vous est assigné va dans *À faire*, une mention dans *Mentions*. Marquez un To-Do terminé dans GitLab et
il quitte aussi Remora.

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
vous mentionner (les 10 fils les plus récents). Quand vous avez écrit en dernier, votre question attend dans *En
attente* et votre réponse termine la conversation : voir
[Quand une conversation Slack est terminée](./boite-de-reception#quand-une-conversation-slack-est-terminee).

1. Réglages → Sources → Slack → **Créer l’app Slack**. Slack s’ouvre avec une app nommée Remora, déjà configurée
   avec les permissions en lecture seule nécessaires, en votre nom : `search:read`, `channels:history` et
   `groups:history` pour lire les fils où vous avez écrit, et `reactions:read` et `emoji:read` pour les réactions.
2. Choisissez votre espace de travail, puis **Next** → **Create**.
3. Sur la page de l’app : **Install App** → *Install to your workspace* → **Allow**.
4. Copiez le **User OAuth Token** (il commence par `xoxp-`) et collez-le dans Remora.

::: warning « Request to install »
Certains espaces de travail exigent qu’un administrateur approuve les nouvelles apps. Slack affiche alors
**Request to install** : une fois l’app approuvée, reprenez à l’étape 3.
:::

Les éléments Slack s’ouvrent dans l’app Slack, sur la conversation. ⌥-clic (Maj-clic sous Windows) ouvre le
message exact sur le web.

Une app créée plus tôt peut manquer de certaines de ces permissions : les Réglages l’indiquent à côté du compte.
Ajoutez celles qui manquent à l’app (*OAuth & Permissions* → *User Token Scopes*), réinstallez-la et connectez à
nouveau le nouveau jeton : le compte est mis à jour, rien n’est perdu. Sans `emoji:read`, les réactions marchent
toujours ; un emoji propre à l’espace de travail s’affiche alors `:nom:`.

## Linear

Affiche les tickets qui vous sont assignés (ni terminés ni annulés), avec leur priorité et leur échéance, plus les
mentions et commentaires non lus de votre boîte Linear.

1. Réglages → Sources → Linear → **Créer une clé API** : Linear ouvre Settings → Security & access.
2. Sous **Personal API keys**, créez une clé nommée Remora. La lecture suffit.
3. Collez-la et cliquez sur **Connecter**.

Les tickets *Urgent* et *High* sont signalés ; *Low* et le Backlog sont affichés mais pas comptés. Les tickets en
retard ou à rendre aujourd’hui déclenchent une notification.

## Jira

Affiche les tickets qui vous sont assignés et ne sont pas terminés, avec leur priorité, leur échéance et leur
statut, plus les commentaires de la dernière semaine qui vous mentionnent, sur les tickets que vous suivez (Jira vous
fait suivre ceux que vous créez, qui vous sont assignés ou que vous commentez). Fonctionne avec Jira Cloud et Jira
Data Center.

- **Jira Cloud** : indiquez l’**Hôte** (`votre-entreprise.atlassian.net`), puis **Créer un jeton** : Atlassian ouvre
  vos jetons d’API. Créez-en un nommé Remora, collez-le, et indiquez l’**E-mail** avec lequel vous vous connectez à
  Jira.
- **Jira Data Center** : indiquez l’hôte de votre Jira, puis dans Jira ouvrez votre profil → **Jetons d’accès
  personnels**, créez-en un, collez-le, et laissez l’e-mail vide.

Les tickets *Highest* (ou *Blocker*, *Critical*) sont urgents et *High* (ou *Major*) importants ; *Low* et *Lowest*
sont affichés mais pas comptés. Les tickets en retard ou à rendre aujourd’hui déclenchent une notification. Une
mention sur un ticket que vous ne suivez pas n’apparaît pas.

Sur Jira Cloud, activez **Mentions dans Confluence** pour voir aussi les pages et commentaires Confluence de la
dernière semaine qui vous mentionnent (sauf ceux que vous avez modifiés en dernier), avec le même jeton, rangés comme
toute mention : *À répondre*, *À lire* ou *Terminé*.

## Microsoft 365

**Aperçu** : construit et testé sur les réponses documentées par Microsoft, pas encore essayé avec une vraie
organisation. Si quelque chose ne marche pas, [ouvrez une issue](https://github.com/iGitScor/triage/issues) avec le
message que Remora affiche.

Affiche les courriels Outlook que vous avez signalés (dans *À faire*, avec leur échéance), les courriels non lus de
la dernière semaine qui vous sont adressés, pas seulement en copie (rangés comme un message direct), les invitations
des deux prochaines semaines sans réponse (dans *À répondre*, jusqu’au début de la réunion) et vos conversations
Teams non lues de la dernière semaine. Comptes professionnels et scolaires.

Remora se connecte par votre navigateur, avec une app que votre organisation inscrit une fois dans Microsoft Entra :
il n’y a pas de jeton à coller. Votre service informatique vous donne le **Tenant** et l’**Identifiant client**, ou les
règle pour vous (ils s’affichent alors verrouillés). Cliquez sur **Se connecter avec Microsoft 365**, connectez-vous
dans le navigateur, et revenez dans Remora. Si la connexion expire ou est révoquée, le compte vous demande de vous
reconnecter.

**Pour la DSI : inscrire l’app.** Dans Microsoft Entra → Inscriptions d’applications → **Nouvelle inscription** :
nommez-la Remora, locataire unique. Sous **Authentification**, ajoutez la plateforme **Applications mobiles et de
bureau** avec l’URI de redirection `http://localhost`, et autorisez les flux de clients publics : Remora ne garde aucun
secret et reçoit la réponse sur un port local (code d’autorisation avec PKCE). Sous **Autorisations d’API**, ajoutez
les autorisations déléguées Microsoft Graph `User.Read`, `Mail.Read`, `Calendars.Read`, `Chat.Read` et
`offline_access`, et accordez le consentement administrateur si les utilisateurs ne peuvent pas consentir eux-mêmes.
Communiquez ensuite le tenant et l’identifiant d’application (client), ou réglez-les avec `MicrosoftTenant` et
`MicrosoftClientID` ([MDM](/fr/admin/mdm)).

## Notion

Les tâches ouvertes qui vous sont assignées, dans les bases Notion que vous choisissez.

1. Réglages → Sources → **Notion** → **Créer un jeton** : dans Notion, **New token**, nommez-le Remora, gardez la
   capacité *Notion API*, choisissez une expiration. Copiez-le (Notion ne l’affiche qu’une fois) et collez-le.
2. Collez les liens de vos bases de tâches, séparés par des virgules (ouvrez la base en pleine page, puis ••• →
   Copier le lien). Le jeton voit ce que vous voyez : rien à partager.
3. Si vos tâches utilisent d’autres noms, réglez la **Propriété « assigné à »** (la propriété personnes qui dit
   à qui est la tâche) et les **statuts terminés**. Une tâche est terminée quand son *Statut* en fait partie, ou
   quand une case nommée *Done* ou *Complete* est cochée. Un statut du groupe *Terminé* de Notion compte aussi, quel
   que soit son nom. Notion écarte lui-même les tâches terminées : elles ne prennent jamais la place des tâches
   ouvertes.

Pas de bouton **New token** ? Votre espace ne laisse que les propriétaires créer des jetons : demandez à l’un
d’eux, ou collez le secret d’une connexion interne créée par un propriétaire, et ajoutez **votre e-mail Notion**
pour que Remora trouve vos tâches.

## L’assistant (facultatif)

L’assistant ajoute un court brief de quoi traiter d’abord, des résumés en deux phrases des listes chargées, et une aide
pour trier ce que vous reportez sans cesse. Il reste désactivé tant que l’IA externe n’est pas autorisée, dans
Réglages → Confidentialité ou par votre organisation (sauf un serveur sur votre ordinateur). Voir
[L’assistant](./assistant).

- **Claude Code** (recommandé) : utilise le Claude Code installé et connecté sur votre ordinateur, sur votre
  abonnement Claude. Réglages → Sources, section *Assistant* → Claude Code → **Connecter**. L’installeur d’Anthropic
  est le plus simple : il n’a pas besoin de Node.js. Sur Mac, les installations par Homebrew, npm (nvm, volta), bun,
  mise ou asdf sont trouvées aussi, et Réglages → Confidentialité indique quel `claude` Remora lance. Sinon, collez le
  résultat de `which claude` (`where.exe claude` sous Windows) dans *Chemin de claude*.
- **API Claude** : une clé d’API de la Claude Console. Réglages → Sources, section *Assistant* → Claude API.
- **OpenAI-compatible** : OpenAI, Azure OpenAI, Mistral, ou un serveur sur votre ordinateur comme Ollama ou
  LM Studio. Réglages → Sources, section *Assistant* → OpenAI-compatible : l’adresse du serveur jusqu’à `/v1`, la
  clé d’API et l’identifiant du modèle de votre fournisseur. Un serveur sur votre ordinateur n’a pas besoin de clé
  et fonctionne avec l’IA externe désactivée.

## Plusieurs comptes

Chaque outil peut être connecté plusieurs fois : deux espaces Slack, GitHub et GitHub Enterprise… Nommez chaque
compte en le connectant (« GitLab boulot », « Slack client »), ou renommez-le ensuite avec le crayon. Quand
plusieurs comptes d’un même outil sont connectés, les éléments affichent le nom du compte.

Quand un jeton expire ou est révoqué, le compte l’indique et propose **Reconnecter…** : collez un nouveau jeton, et
le compte garde ses Terminé, reports et épingles. Pour remplacer un jeton avant, utilisez **Mettre à jour le jeton…**
sur le compte. Sur Mac, connecter à nouveau le même compte (même outil, serveur et
utilisateur) fait de même.

Chaque liste contient les 50 éléments les plus récents. Quand un outil en a davantage (60 demandes de relecture,
par exemple), un ⓘ en bas de la boîte et une ligne à côté du compte dans les Réglages le signalent : le reste est
dans l’outil.

## En cas de problème

Le bas de la boîte et le compte dans Réglages affichent l’erreur ; les derniers éléments restent visibles en
attendant. Hors ligne, le bas de la boîte indique seulement **Hors ligne** : Remora attend la connexion et actualise
dès son retour. Une limite de requêtes est attendue sans compter comme une erreur.

| Vous voyez | Que faire |
|---|---|
| *… doit être reconnecté* | Le jeton a expiré ou a été révoqué : créez-en un nouveau, puis **Reconnecter…** sur le compte dans Réglages. |
| *Serveur injoignable* | Seul le serveur de cet outil ne répond pas : vérifiez son adresse, ou connectez-vous à votre VPN s’il en faut un. |
| *Bloqué : … n’est pas une destination autorisée.* | Remora a refusé de joindre une adresse que l’outil ne déclare pas. Vérifiez l’adresse saisie. |
| Rien d’une organisation GitHub | Autorisez le jeton pour le SSO (voir plus haut). |
| *Non autorisé par votre politique de confidentialité.* | L’outil est désactivé dans Réglages → Confidentialité, ou par votre organisation : demandez à votre DSI. |

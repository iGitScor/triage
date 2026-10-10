---
title: L’assistant
description: Les fonctions d’IA facultatives de Remora (brief, résumés de liste, tri des reports), connecter Claude Code, l’API Claude ou un serveur OpenAI-compatible, ce qui est envoyé, et comment limiter les jetons consommés.
---

# L’assistant

Tout Remora fonctionne sans IA. L’assistant (Claude, ou un modèle derrière une API OpenAI-compatible) est un plus
facultatif, **désactivé par défaut**, qui écrit de courts textes
sur votre boîte :

- **Brief** (✦ en haut de la boîte) : trois phrases et les éléments à traiter d’abord.
- **Résumés de liste** (✦ sur l’en-tête d’un verbe, à partir de deux éléments) : de quoi parle le groupe, en deux
  phrases.
- **Tri** (dans l’onglet *Reportés*) : garder, reprogrammer, laisser tomber ou faire maintenant, élément par
  élément. Vous validez chaque changement ; seules les reprogrammations sont cochées d’avance.

Briefs et résumés sont rédigés dans la langue de votre Mac.

## L’activer

1. Réglages → Confidentialité → **Autoriser l’IA externe** (inutile pour un serveur sur votre ordinateur). Si votre organisation gère Remora, ce réglage
   peut être verrouillé : voir [DSI et conformité](/fr/admin/).
2. Réglages → Sources, section *Assistant*, connectez au choix :
   - **Claude Code** (recommandé) : utilise le Claude Code installé et connecté sur votre Mac, sur votre abonnement
     Claude. Pas de clé d’API. Si Remora ne trouve pas `claude`, renseignez *Chemin de claude*. Remora n’exécute qu’un
     programme nommé `claude` que vous seul pouvez modifier, et vérifie que c’est Claude Code avant de lui envoyer quoi que ce soit.
   - **API Claude** : une clé d’API de la Claude Console, facturée sur ce compte.
   - **OpenAI-compatible** : OpenAI, Azure OpenAI, Mistral, ou un serveur sur votre ordinateur (Ollama, LM Studio).
     Indiquez l’adresse du serveur jusqu’à `/v1`, la clé d’API et l’identifiant du modèle ; le modèle des résumés est
     facultatif. OpenAI et Azure sont pris en charge ; un petit modèle local peut ne pas suivre le format de réponse
     attendu, et Remora dit alors avoir reçu une réponse inattendue. Votre organisation peut régler le serveur pour vous.

## Ce qui est envoyé

Pour chaque élément : son verbe, son **titre**, son contexte (dépôt, canal, clé de ticket), son auteur, ses statuts
et son âge. Le titre d’un message Slack est la **première ligne du message** ; la suite n’est pas envoyée. Le tri
envoie aussi la date de retour de chaque élément reporté, la raison choisie et le nombre de reports. Jamais vos
jetons, liens, code, notes, ni la suite d’un message. Pour tenir un outil à l’écart de l’assistant, désactivez-le dans
Réglages → Général → **Envoyer à l’assistant** (*Masquer le contenu des messages* ne concerne que les notifications). Avec Claude Code, Remora le lance sans outils, sans extensions et sans rien enregistrer
dans l’historique de vos sessions. La liste exacte est dans
[Flux de données](/fr/admin/flux-de-donnees#ce-qui-parvient-a-claude).

## Limiter les jetons consommés

- Un brief ou un résumé est **réutilisé** pendant 30 minutes par défaut (Réglages → Général → Assistant →
  **Réutiliser un brief ou un résumé pendant**), et un résumé de liste seulement tant que la liste ne change pas.
- **Brief de toute la boîte** peut être désactivé pour ne garder que les résumés de liste, plus courts et faits avec
  un modèle plus léger (`claude-haiku-5-5` par défaut).
- Rien n’est généré avant que vous cliquiez sur ✦, à part un premier brief quand vous connectez Claude (si le brief
  de toute la boîte est désactivé, le test utilise un seul élément fictif).

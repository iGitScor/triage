---
title: L’assistant
description: Les fonctions d’IA facultatives de Remora (brief, résumés, tri des reports), comment connecter l’assistant, ce qui est envoyé, et limiter les jetons.
---

# L’assistant

Tout Remora fonctionne sans IA. L’assistant (Claude, ou un modèle derrière une API OpenAI-compatible) est un plus
facultatif, **désactivé par défaut**, qui écrit de courts textes sur votre boîte :

- **Brief** (✦ en haut de la boîte) : trois phrases et les éléments à traiter d’abord, à partir d’*À moi*,
  de vos éléments épinglés et de la tâche en cours. Il est gardé tant que votre boîte ne change pas ; après un Terminé, un report ou une arrivée, ↻ en rédige un nouveau.
- **Résumés de liste** (✦ sur l’en-tête d’un verbe, à partir de deux éléments) : de quoi parle le groupe, en deux
  phrases.
- **Tri** (dans l’onglet *Reportés*) : garder, reprogrammer (à 60 jours au plus), laisser tomber ou faire
  maintenant, élément par élément. Vous validez chaque changement ; seules les reprogrammations sont cochées d’avance.

Briefs et résumés sont rédigés dans la langue de votre système.

## L’activer

1. Réglages → Confidentialité → **Autoriser l’IA externe** (inutile pour un serveur sur votre ordinateur). Si votre
   organisation gère Remora, ce réglage peut être verrouillé : voir [DSI et conformité](/fr/admin/).
2. Réglages → Sources, section *Assistant*, connectez au choix :
   - **Claude Code** (recommandé) : utilise le Claude Code installé et connecté sur votre ordinateur, sur votre
     abonnement Claude. Pas de clé d’API. Si Remora ne trouve pas `claude`, renseignez *Chemin de claude*. Remora
     n’exécute qu’un programme nommé `claude` (sur Mac, que vous seul pouvez modifier), et vérifie que c’est Claude
     Code avant de lui envoyer quoi que ce soit.
   - **API Claude** : une clé d’API de la Claude Console, facturée sur ce compte. Le brief utilise
     `claude-sonnet-5-5` et les résumés `claude-haiku-5-5`, sauf si vous en choisissez d’autres ; un modèle
     qu’Anthropic ne propose pas est signalé à la connexion.
   - **OpenAI-compatible** : OpenAI, Azure OpenAI, Mistral, ou un serveur sur votre ordinateur (Ollama, LM Studio).
     Indiquez l’adresse du serveur jusqu’à `/v1`, la clé d’API et l’identifiant du modèle ; le modèle des résumés est
     facultatif. OpenAI et Azure sont pris en charge ; un petit modèle local peut ne pas suivre le format de réponse
     attendu, et Remora dit alors avoir reçu une réponse inattendue. Votre organisation peut régler le serveur pour
     vous.

## Ce qui est envoyé

Pour chaque élément : son verbe, son **titre**, son contexte (dépôt, canal, clé de ticket), son auteur, ses statuts
et son âge. Le titre d’un message Slack est la **première ligne du message** ; la suite n’est pas envoyée. Le tri
envoie aussi la date de retour de chaque élément reporté, la raison choisie et le nombre de reports. Jamais vos
jetons, liens, code, notes, ni la suite d’un message. Pour tenir un outil à l’écart de l’assistant, désactivez-le
dans Réglages → Général → **Envoyer à l’assistant** (*Masquer le contenu des messages* ne concerne que les
notifications). Avec Claude Code, Remora le lance sans outils, sans extensions et sans rien enregistrer dans
l’historique de vos sessions. La liste exacte est dans
[Flux de données](/fr/admin/flux-de-donnees#ce-qui-parvient-a-claude).

## Limiter les jetons consommés

- Un brief ou un résumé est **réutilisé** pendant 30 minutes par défaut (Réglages → Général → Assistant →
  **Réutiliser un brief ou un résumé pendant**), et un résumé de liste seulement tant que la liste ne change pas.
- **Brief de toute la boîte** peut être désactivé pour ne garder que les résumés de liste, plus courts et faits avec
  un modèle plus léger (`claude-haiku-5-5` par défaut avec Claude).
- Rien n’est généré avant que vous cliquiez sur ✦, à part un premier brief quand vous connectez l’assistant (si le brief
  de toute la boîte est désactivé, le test utilise un seul élément fictif).
- Réglages → Général → Assistant montre ce que l’assistant a consommé **aujourd’hui** et **ce mois-ci** : les requêtes,
  et les jetons tels que Claude, Claude Code ou le serveur les ont indiqués (un serveur qui n’en indique pas ne compte
  que les requêtes). Ce relevé reste sur cet ordinateur, pour les 31 derniers jours, et **Effacer les données
  locales** le supprime. Aucun prix : Remora ne sait pas ce que facture votre offre.
- **Limite par jour** (désactivée par défaut) plafonne les requêtes quotidiennes ; un brief, un résumé ou un tri en est
  une. Au-delà, l’assistant le dit et attend le lendemain. Votre organisation peut la fixer (`AIDailyLimit`).

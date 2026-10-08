---
title: L’assistant (Claude)
description: Les fonctions Claude facultatives de Remora (brief, résumés de liste, tri des reports), connecter Claude Code ou l’API Claude, ce qui est envoyé, et comment limiter les jetons consommés.
---

# L’assistant (Claude)

Tout Remora fonctionne sans IA. Claude est un plus facultatif, **désactivé par défaut**, qui écrit de courts textes
sur votre boîte :

- **Brief** (✦ en haut de la boîte) : trois phrases et les éléments à traiter d’abord.
- **Résumés de liste** (✦ sur l’en-tête d’un verbe, à partir de deux éléments) : de quoi parle le groupe, en deux
  phrases.
- **Tri** (dans l’onglet *Reportés*) : garder, reprogrammer, laisser tomber ou faire maintenant, élément par
  élément. Vous validez chaque changement.

Briefs et résumés sont rédigés dans la langue de votre Mac.

## L’activer

1. Réglages → Confidentialité → **Autoriser l’IA externe (Claude)**. Si votre organisation gère Remora, ce réglage
   peut être verrouillé : voir [DSI et conformité](/fr/admin/).
2. Réglages → Sources, section *Assistant*, connectez au choix :
   - **Claude Code** (recommandé) : utilise le Claude Code installé et connecté sur votre Mac, sur votre abonnement
     Claude. Pas de clé d’API. Si Remora ne trouve pas `claude`, renseignez *Chemin de claude*.
   - **API Claude** : une clé d’API de la Claude Console, facturée sur ce compte.

## Ce qui est envoyé

Les **titres, contextes** (dépôt, canal, clé de ticket), **auteurs et statuts** des éléments. Pas vos jetons, pas le
code, pas des conversations entières. Avec Claude Code, Remora le lance sans outils, sans extensions et sans rien
enregistrer dans l’historique de vos sessions.

## Limiter les jetons consommés

- Un brief ou un résumé est **réutilisé** pendant 30 minutes par défaut (Réglages → Général → Assistant →
  **Réutiliser un brief ou un résumé pendant**), et un résumé de liste seulement tant que la liste ne change pas.
- **Brief de toute la boîte** peut être désactivé pour ne garder que les résumés de liste, plus courts et faits avec
  un modèle plus léger (`claude-haiku-5-5` par défaut).
- Rien n’est généré avant que vous cliquiez sur ✦, à part un court brief de test quand vous connectez Claude.

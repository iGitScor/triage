---
title: Questions et dépannage
description: Réponses sur la confidentialité de Remora, la demande du trousseau, macOS qui refuse d’ouvrir l’app, des éléments manquants, les notifications, Windows, et l’effacement de vos données.
---

# Questions et dépannage

## Mes données passent-elles par un serveur Remora ?

Il n’y a pas de serveur Remora. L’app parle directement aux outils connectés, garde ses données dans
`~/Library/Application Support/Remora` et vos jetons dans le trousseau. Pas de télémétrie, pas d’analytics, pas de
rapports de plantage. Chaque flux est listé dans [Flux de données](/fr/admin/flux-de-donnees).

## macOS dit qu’il ne peut pas vérifier l’app, ou refuse de l’ouvrir

Remora n’est pas encore notarisée par Apple. Clic droit sur Remora dans Applications → **Ouvrir**. Sous macOS 15 ou
ultérieur, essayez de l’ouvrir une fois, puis Réglages Système → Confidentialité et sécurité → **Ouvrir quand
même**. macOS ne demande qu’une fois par version.

## Pourquoi macOS demande-t-il le mot de passe du trousseau ?

Remora garde tous vos jetons dans un seul élément du trousseau. macOS le demande une fois par nouvelle version d’une
app qui n’est pas signée par un développeur Apple enregistré. Choisissez **Toujours autoriser** : il ne redemandera
pas avant la prochaine mise à jour.

## Un élément attendu n’est pas là

- Il est peut-être dans *Terminés* (il revient s’il y a du nouveau) ou *Reportés*.
- Les éléments *À lire* sont en bas de *À moi* ; la recherche trouve tout.
- GitHub : une demande de relecture disparaît une fois relue. Les organisations avec SSO demandent d’autoriser le
  jeton.
- Slack : seuls les derniers jours de mentions et messages directs sont récupérés.
- Regardez le bas de la boîte et Réglages → Sources : une erreur sur ce compte ?

## Je ne reçois pas de notifications

Réglages Système → Notifications → Remora : autorisez-les. Puis Réglages → Général → Notifications dans Remora. La
première synchronisation d’un nouveau compte est volontairement silencieuse.

## Remora existe-t-elle sur Windows ?

Une version Windows arrive, avec les mêmes règles et le même modèle de conformité. Son cœur et les quatre sources
sont prêts ; l’app dans la zone de notification suit. Voir [Remora pour Windows](/develop/windows) (en anglais).

## Comment tout effacer ?

Réglages → Confidentialité → **Effacer les données locales…** déconnecte tous les comptes, supprime le cache,
l’historique et les réglages, et retire les jetons du trousseau. Ensuite, glissez Remora à la Corbeille.

## Combien ça coûte ?

Rien. Remora est un logiciel libre sous licence GPL-3.0-or-later ; le code source est sur
[GitHub](https://github.com/iGitScor/triage). Questions et bugs : [GitHub issues](https://github.com/iGitScor/triage/issues).

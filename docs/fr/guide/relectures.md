---
title: Les relectures
description: Préparer une relecture (estimation, fichiers, zones sensibles), les sessions de relecture au clavier, les liens entre pull requests et tickets Linear, et l’assistant d’attente pour vos merge requests.
---

# Les relectures

## Préparer la relecture

Sous chaque demande de relecture, Remora indique ce qu’elle va demander : **« ~6 min · 4 fichiers · tests ✓ ·
Authentification »**.

- Une **estimation** selon la taille de la modification.
- Si la modification **touche des tests** (✓).
- Les **zones** qui méritent de l’attention, lues dans les chemins des fichiers : *Migrations*,
  *Authentification*, *Données personnelles*, *Infra/CI*, *Dépendances* ou *Lockfile seul*. Quand elles ne
  tiennent pas, le reste se replie en « +N ».

Cliquez sur la ligne pour voir les plus gros fichiers. Remora ne lit que les chemins et le nombre de lignes : elle
ne télécharge, ne stocke et n’envoie jamais le code.

## Sessions de relecture

**Démarrer une session**, sur l’en-tête *À relire*, parcourt vos relectures une par une, les plus rapides
d’abord :

| Touche | Action |
|---|---|
| <kbd>⏎</kbd> | Ouvrir la modification dans le navigateur |
| <kbd>D</kbd> | Terminé, suivante |
| <kbd>S</kbd> | Reporter, suivante |
| <kbd>→</kbd> | Passer, suivante |

## Des liens entre outils

Quand une pull request et un ticket Linear ont des titres proches (« Fix the CSV export » et « ENG-42 CSV export
broken »), Remora les relie, même si personne ne l’a fait : chacun affiche l’autre sous forme de pastille (la clé du
ticket, ou le dépôt et le numéro), et un clic l’ouvre. La comparaison se fait sur votre Mac.

## En attente des relecteurs

<Screenshot name="waiting" alt="L’onglet En attente : une merge request sans relecteur avec des suggestions, une autre en attente depuis deux jours avec une relance rédigée" />

Pour vos merge requests dans *En attente*, Remora aide quand elles coincent :

- **Pas encore de relecteur** : des suggestions, d’abord celles de l’outil (GitHub), puis ceux qui relisent
  d’habitude dans ce dépôt. **Demander une relecture** rédige un court message.
- **Relecteurs silencieux depuis un jour ou plus** : **Rédiger une relance** écrit un rappel aimable qui dit depuis
  combien de temps elle attend, et qu’elle est petite ou au vert quand c’est le cas.

Les messages sont rédigés sur votre Mac à partir de modèles et copiés dans le presse-papiers. Remora n’envoie jamais
rien : vous collez le message où vous voulez.

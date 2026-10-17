---
title: Liens et automatisation
description: Ouvrir Remora, un onglet ou le panneau de rappel depuis Raccourcis, Raycast, Alfred, un Stream Deck ou un script, avec les liens remora:// ; lire le compte et la liste avec la commande remora.
---

# Liens et automatisation

Remora répond aux liens `remora://`, sur Mac, Windows et Linux. Raccourcis, Raycast, Alfred, un Stream Deck, PowerToys
ou un script peuvent s’en servir pour ouvrir la boîte ou le panneau de rappel, ou pour actualiser.

| Lien | Ce qu’il fait |
|---|---|
| `remora://open` | Ouvre la boîte |
| `remora://open?tab=waiting` | Ouvre la boîte sur un onglet : `in-progress`, `my-turn`, `waiting`, `snoozed` ou `done` |
| `remora://remind?text=Appeler%20Erin&at=2026-10-14T09:00` | Ouvre le panneau de rappel avec le texte et l’heure remplis ; c’est vous qui l’enregistrez |
| `remora://refresh` | Actualise vos outils tout de suite |

`text` est coupé à 200 caractères. `at` est une heure de votre ordinateur (`2026-10-14T09:00`) ou avec son décalage
(`2026-10-14T09:00:00+02:00`) ; sans heure, ou avec une heure déjà passée, vous la choisissez dans le panneau.

Un lien ne marque jamais un élément comme terminé, ne le reporte pas et ne supprime rien : ce qui ouvre un lien ne
fait que vous montrer quelque chose, et vous décidez. Un lien inconnu est ignoré.

Essayez depuis un terminal : `open "remora://open?tab=waiting"` sur Mac, `start remora://refresh` sous Windows,
`xdg-open remora://open` sous Linux (le `.deb` enregistre les liens ; l’AppImage le fait à son lancement).

## La commande `remora`

Un lien ne peut pas répondre. Pour une barre d’état (SketchyBar, tmux, Waybar), un lanceur ou un script qui veut le
compte ou la liste, la commande `remora` interroge l’app ouverte :

| Commande | Ce qu’elle affiche |
|---|---|
| `remora count` | Ce qui vous attend, le nombre affiché dans la barre des menus |
| `remora list --tab waiting` | Les éléments d’un onglet, un par ligne : `in-progress`, `my-turn` (par défaut), `waiting`, `snoozed` ou `done` |
| `remora remind "Appeler Erin" --at 15:00` | Enregistre un rappel : `15:00` est le prochain 15 h, ou une date comme dans un lien |

`--json` affiche la réponse en JSON (identifiant, source, type, titre, contexte et lien de chaque élément, et pour un
message `why`, la raison de son rangement ; `bot` quand une app ou un bot l’a écrit), pour Raycast ou Alfred. Un outil coché dans **Masquer le contenu des messages** garde ses titres : ses éléments arrivent
avec `"title": null` et `"hidden": true` (*(content hidden)* dans la liste), seulement où et de quel type (`why` garde
son code, pas les mots). Comme les
liens, la commande ne marque jamais un élément comme terminé, ne le reporte pas et ne supprime rien ; un rappel est la
seule chose qu’elle enregistre. Elle ne parle qu’à l’app, par un socket (un tube nommé sous Windows) que seul votre
compte utilisateur peut ouvrir : rien ne passe par le réseau, et Remora fermé, elle le dit (code de sortie 3).

La mettre dans votre PATH :

- **Mac** : `sudo ln -s /Applications/Remora.app/Contents/Helpers/remora /usr/local/bin/remora`
- **Linux** : le `.deb` installe `/usr/bin/remora`, qui est aussi la commande ; avec l’AppImage,
  `Remora.AppImage count`.
- **Windows** : l’app elle-même, `%LOCALAPPDATA%\Remora\Remora.exe count` ; ajoutez ce dossier à votre PATH pour
  taper `remora count`.

Les agents d’IA (Claude Desktop, Claude Code, Gemini CLI) utilisent la même commande, en `remora mcp` : voir
[Remora et vos agents](/fr/guide/agents).

Une commande de script Raycast, par exemple, est un fichier avec quelques commentaires en tête :

```sh
#!/bin/bash
# @raycast.schemaVersion 1
# @raycast.title Remora count
# @raycast.mode inline
# @raycast.refreshTime 5m
remora count
```

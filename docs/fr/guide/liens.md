---
title: Liens et automatisation
description: Ouvrir Remora, un onglet ou le panneau de rappel depuis Raccourcis, Raycast, Alfred, un Stream Deck ou un script, avec les liens remora://.
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

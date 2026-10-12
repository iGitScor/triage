---
title: Accessibilité
description: La déclaration d’accessibilité de Remora. La norme visée (WCAG 2.1 AA), ce que prennent en charge les apps Mac et Windows (VoiceOver, Narrateur, clavier, animations réduites, contraste), comment c’est vérifié, les manques connus, et comment signaler un problème.
---

# Accessibilité

Remora vise le **niveau AA des WCAG 2.1** dans les deux apps, le site et cette documentation. Aujourd’hui, elle est
**partiellement conforme** : une grande partie des apps s’utilise avec un lecteur d’écran et au clavier, mais les
manques connus ci-dessous demeurent. Cette déclaration est revue à chaque version.

## Ce que les apps prennent en charge

| | macOS | Windows (aperçu) |
|---|---|---|
| Lecteur d’écran | VoiceOver lit chaque élément comme un bouton, avec son outil, son verbe, son heure et ses réactions ; le rotor des actions propose Terminé, Reporter, Épingler, Commencer et les éléments liés. L’icône de la barre des menus dit combien d’éléments vous attendent. | Le Narrateur entend chaque élément et ses actions, ce qu’une actualisation a apporté, l’échec d’une action, et le titre de chaque écran. |
| Clavier | ↑ ↓ ou J K, ⏎, E (terminé), S (reporter), P (épingler), ⌘F, ⌘Z, Échap ; la frise du temps et chaque panneau s’utilisent sans pointeur, et les panneaux gardent le focus. | Les mêmes touches avec Entrée et Ctrl, → ← vers les boutons d’un élément ; un seul arrêt de Tab par liste ; les onglets aux flèches ; les panneaux gardent le focus et le rendent à leur fermeture. |
| Gestes de glisser | Chaque balayage ou glisser a un bouton, un menu ou une touche : un nouveau rappel aussi, depuis le menu du poisson. | Aucun n’est nécessaire : chaque ligne a des boutons, et un nouveau rappel se crée avec Ctrl+Alt+R. |
| Animations | Réduire les animations : rien ne glisse, ne rebondit ni ne tourne. | Effets d’animation désactivés : de même. |
| Contraste et couleur | Augmenter le contraste fonce le texte et les bordures ; l’état des relecteurs et des vérifications a un symbole et un libellé, pas seulement une couleur. | Les thèmes de contraste donnent une bordure aux pastilles, puces et cartes ; les états ont un libellé. |
| Taille du texte | Réglages → Général : par défaut, grande ou plus grande ; aucun texte sous 11 pt. | La mise à l’échelle de Windows. |
| Langue | Anglais et français, chaque écran et chaque libellé lu. | De même. |

## Comment c’est vérifié

- **Site et documentation** : chaque page, en clair et en sombre, selon les WCAG 2.1 A et AA avec axe, à chaque
  changement en CI (`make a11y` en local).
- **Apps** : des tests d’interface couvrent le clavier et le focus (onglets, panneaux, menus) ; les libellés lus
  sont relus en revue de code.
- **Pas encore** : un audit par un tiers, ni un passage complet avec VoiceOver et le Narrateur sur chaque écran
  avant chaque version, ni un essai de l’app Linux avec Orca. Les vérifications automatiques ne trouvent qu’une partie des problèmes : les manques
  ci-dessous viennent de la lecture du code.

## Manques connus

- **Windows** : le focus peut se perdre quand une confirmation remplace son bouton ; la fenêtre ne s’agrandit pas et
  le texte ne grossit pas au-delà de la mise à l’échelle de Windows.
- **macOS** : les champs du formulaire de connexion sont nommés par leur texte indicatif ; les éléments du brief ne
  s’ouvrent qu’au clic.

Chaque manque est suivi, avec sa correction, dans la feuille de route du projet.

## Signaler un problème

Ouvrez un ticket sur [GitHub](https://github.com/iGitScor/triage/issues/new/choose) en précisant l’app, l’écran et
la technologie d’assistance utilisés. Les problèmes d’accessibilité sont traités comme des bugs, corrigés dans la
version suivante quand c’est possible. Un ticket est public ; pour ce que vous préférez garder privé, utilisez [un
avis privé](https://github.com/iGitScor/triage/security/advisories/new).

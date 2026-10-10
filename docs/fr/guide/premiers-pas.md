---
title: Premiers pas
description: Installer Remora sur votre Mac, l’ouvrir la première fois, connecter un premier outil et prendre ses repères dans la boîte de réception de la barre des menus.
---

# Premiers pas

Remora vit dans la barre des menus de votre Mac. Elle rassemble ce qui a besoin de vous dans GitHub, GitLab, Slack
et Linear, et le range par ce que vous avez à faire. Pas de compte à créer, pas de serveur : tout se passe sur
votre Mac.

## Installer

1. [Téléchargez Remora.dmg](https://github.com/iGitScor/triage/releases/latest/download/Remora.dmg). Il faut
   macOS 14 (Sonoma) ou ultérieur.
2. Ouvrez le DMG et glissez **Remora** sur **Applications**.
3. Ouvrez Remora depuis Applications. Remora n’est pas encore notarisée par Apple, donc la première fois :
   - sous macOS 14, clic droit sur Remora → **Ouvrir**, puis **Ouvrir** à nouveau ;
   - sous macOS 15 ou ultérieur, essayez de l’ouvrir une fois, puis Réglages Système → Confidentialité et
     sécurité → **Ouvrir quand même**.

   macOS ne demande qu’une fois par version.

Le poisson apparaît dans la barre des menus. Remora n’a pas d’icône dans le Dock : c’est une app de barre des
menus.

::: tip Ouvrir à la connexion
Réglages → Général → **Ouvrir à la connexion**, pour retrouver Remora chaque matin.
:::

## Sous Windows (aperçu)

1. [Téléchargez Remora-Setup.exe](https://github.com/iGitScor/triage/releases/latest/download/Remora-Setup.exe). Il faut Windows 10 ou 11.
2. Lancez-le. Il installe Remora pour votre utilisateur seulement, sans droits d’administrateur.
3. L’installeur n’est pas encore signé : si Windows dit avoir protégé votre PC, cliquez sur **Informations
   complémentaires** → **Exécuter quand même**.

Remora se place dans la zone de notification de la barre des tâches (cliquez sur **^** si elle est masquée, et
glissez-la près de l’horloge pour la garder visible). Un clic ouvre la boîte ; un clic droit, le menu. Les icônes de
la zone de notification ne se tirent pas sous Windows : un nouveau rappel se crée avec **Ctrl+Alt+R**, de partout.
Réglages → Général → **Lancer au démarrage de Windows** la démarre à l’ouverture de session.

La version Windows a ce qu’a le Mac : les cinq outils, le report avec raisons et constats, la préparation et les
sessions de relecture, l’assistant d’attente et l’assistant Claude. Elle trie les messages par mots-clés seulement, et
ses notifications n’ont pas encore de boutons ; la liste complète est dans
[Remora pour Windows](/fr/developper/windows#ce-que-windows-a-et-ce-qu-il-n-a-pas-encore).

## Connecter un premier outil

Cliquez sur le poisson, puis sur la roue dentée (Réglages) → **Sources**, et choisissez un outil. Chacun a un
bouton qui ouvre la bonne page de l’outil pour créer un jeton, et des étapes numérotées ;
[Connecter vos outils](./connecter-vos-outils) détaille chacun.

La première synchronisation est silencieuse : connecter un outil ne vous noie jamais sous les notifications.
Ensuite, Remora vérifie toutes les 5 minutes (Réglages → Général → **Vérifier toutes les**), au réveil du Mac,
et quand vous ouvrez la boîte.

La première fois qu’elle enregistre un jeton, macOS demande si Remora peut utiliser son élément du trousseau.
Choisissez **Toujours autoriser**.

## Prendre ses repères

<Screenshot name="myturn" alt="La boîte : onglets, recherche, puis les éléments rangés par verbe" />

- **Le poisson** dans la barre des menus indique combien de choses ont besoin de vous. Réglages → Général → **Un
  compteur par source** affiche un compteur par outil, dans l’ordre du plus pressant.
- **Clic** sur le poisson pour ouvrir la boîte. **Clic droit** pour Actualiser, Nouveau rappel, Réglages et
  Quitter.
- **Tirez le poisson vers le bas** pour créer un rappel : voir [Reporter et rappels](./reporter-et-rappels#rappels).
- Les **onglets** : *À moi* (quelqu’un attend après vous), *En attente* (vous attendez les autres), *Reportés*
  et *Terminés*.
- Un **élément** ouvre son lien au clic : dans l’appli Slack ou Linear si elle est installée, sinon dans le
  navigateur. Ses boutons l’épinglent, le reportent ou le marquent terminé.

Ensuite : [la boîte de réception](./boite-de-reception), et comment Remora décide où va chaque chose.

## Essayer sans rien connecter

Remora a un mode démo avec des données d’exemple, ouvert dans une fenêtre. Rien n’est enregistré :

```sh
open /Applications/Remora.app --args --demo
```

## Langues

Remora parle français et anglais et suit la langue de votre Mac. Pour une autre langue pour Remora seulement :
Réglages Système → Général → Langue et région → Applications → **+** → Remora.

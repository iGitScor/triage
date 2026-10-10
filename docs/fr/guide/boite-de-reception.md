---
title: La boîte de réception
description: Comment Remora range ce qui a besoin de vous. À moi et En attente, les verbes (À répondre, À relire, À corriger…), Terminé, Épingler, priorité, recherche, gestes et notifications.
---

# La boîte de réception

## Quatre onglets

| Onglet | Ce qu’il contient |
|---|---|
| **À moi** | Quelqu’un attend après vous : une question, une demande de relecture, une correction, une tâche, un rappel revenu, votre merge request une fois approuvée, bloquée ou en échec. |
| **En attente** | Vous attendez les autres : vos merge requests en relecture, vos brouillons. |
| **Reportés** | Ce que vous avez mis de côté. Voir [Reporter et rappels](./reporter-et-rappels). |
| **Terminés** | Ce que vous avez terminé, jusqu’à ce qu’il y ait du nouveau. |

Le compteur de la barre des menus ne compte que *À moi*, sans ce qui est discret (*À lire*, priorité basse).
Réglages → Général → **Compteur** peut tout compter, ou rien.

## Des verbes, pas des applis

Dans *À moi*, les éléments sont rangés par ce que vous avez à faire, quel que soit l’outil d’où ils viennent :

| Verbe | Par exemple |
|---|---|
| **Rappels** | Un rappel que vous avez créé, arrivé à échéance |
| **À répondre** | Une question sur Slack, une mention dans un commentaire Linear |
| **À relire** | Une demande de relecture sur GitHub ou GitLab |
| **À corriger** | Votre merge request avec des modifications demandées ou des tests en échec |
| **Prêtes à fusionner** | Votre merge request, approuvée et au vert |
| **À faire** | Un ticket Linear qui vous est assigné |
| **À lire** | Une info, une annonce. Affiché en dernier, jamais compté |
| **En attente des autres** | Dans l’onglet *En attente* |

Les messages sont rangés dans *À répondre* ou *À lire* sur votre Mac : d’abord par des règles de mots-clés en
français et en anglais (« pourrais-tu… », « can you… », « pour info », « FYI »), puis par le modèle de langue
embarqué d’Apple. Pas d’IA générative, rien n’est envoyé. Dans le doute, Remora choisit *À répondre* : mieux vaut
une question à écarter qu’une question manquée. Les messages des applications Slack (Google Agenda, Jira…) vont
Sur Windows, seules les règles de mots-clés s’appliquent (le modèle de langue est celui d’Apple) : un message sans
l’un de ces mots va dans *À répondre*.

## Terminé, Épingler

- **Terminé** masque un élément jusqu’à ce qu’il change : un commit, une approbation, un commentaire, une réponse.
  Il revient alors tout seul dans *À moi*. **Tout effacer**, dans l’onglet Terminés, vide la liste ; les éléments
  effacés reviennent quand même s’il y a du nouveau.
- **Épingler** garde un élément en haut de *À moi*, quoi qu’il arrive ensuite.

## Ce qui passe en premier

Dans chaque verbe, Remora place d’abord :

1. ce qui est en retard ou à faire aujourd’hui ;
2. la priorité de l’outil (Linear *Urgent*, puis *High*…). La priorité basse et le Backlog de Linear sont
   affichés, pas comptés ;
3. l’échéance la plus proche ;
4. l’activité la plus récente.

Après une vingtaine de vos actions, Remora apprend aussi, sur votre Mac, ce que vous traitez vite et ce que vous
repoussez, et s’en sert pour départager. Une petite étoile sur un élément dit pourquoi il est remonté.

Un verbe chargé montre ses cinq premiers éléments, et **Afficher N de plus** pour le reste. La boîte ne montre
jamais plus que ce qu’on peut saisir d’un coup d’œil.

## Recherche, gestes, clavier

- **La recherche** filtre tous les onglets par titre, contexte (dépôt, canal, clé de ticket), auteur et aperçu.
- **Deux doigts sur le trackpad** : vers la droite pour terminer, vers la gauche pour reporter.
- **Clic droit sur un élément** pour toutes les actions : Ouvrir, Ouvrir dans le navigateur, Copier le lien,
  Épingler, Reporter…, Marquer comme terminé.
- **Clavier** : ↑ ↓ ou J K pour passer d’un élément à l’autre, ⏎ pour ouvrir (⌥⏎ dans le navigateur), E pour
  terminer, S pour reporter, P pour épingler. ⌘F pour chercher (ou tapez directement), Échap pour quitter la
  recherche, ⌘R pour actualiser, ⌘Z pour annuler. Le bouton clavier en bas les rappelle.
- **VoiceOver** lit chaque élément comme un seul bouton ; le rotor des actions propose Marquer comme terminé,
  Reporter, Épingler, Commencer et les éléments liés. L’icône de la barre des menus dit combien d’éléments vous
  attendent, ou la tâche en cours.

## Notifications

Remora vous prévient quand quelque chose de nouveau a besoin de vous (une relecture, une mention, une tâche) et quand
votre merge request change de statut (approuvée, modifications demandées, tests en échec). Chaque notification a
des boutons : **Terminé**, **Reporter d’1 heure** ou **Demain 9:00**, sans ouvrir Remora. Réglages → Général →
coupe chaque type.

**Masquer le contenu des messages** liste vos outils connectés (et vos rappels) : pour ceux que vous cochez, les
notifications disent toujours ce qui s’est passé et où (*Mentions · #general*), mais pas ce qui a été écrit.
Pratique pour Slack pendant un partage d’écran.

La première synchronisation d’un nouveau compte ne notifie jamais : connecter un outil ne vous noie pas.

## Réglages → Général

| Réglage | Effet |
|---|---|
| Vérifier toutes les | La fréquence d’actualisation (5 minutes par défaut) |
| Ouvrir dans les applications installées | Slack et Linear s’ouvrent dans leur appli ; ⌥-clic ouvre la page web |
| Ouvrir à la connexion | Lancer Remora avec votre Mac |
| Compteur | *À moi*, toute ma boîte, ou rien |
| Un compteur par source | Un compteur par outil dans la barre des menus, dans l’ordre du verbe le plus pressant |
| Pastille citron quand quelque chose m’attend | Met en valeur l’icône quand *À moi* n’est pas vide |
| Thème | Système, clair ou sombre |

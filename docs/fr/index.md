---
layout: home
title: Documentation
description: Utiliser Remora, la boîte de réception dans la barre des menus du Mac pour GitHub, GitLab, Slack et Linear ; la déployer et la verrouiller côté DSI.

hero:
  name: Remora
  text: Tout ce qui a besoin de vous. Rien de plus.
  tagline: Une seule boîte de réception dans la barre des menus pour les relectures, merge requests, Slack et Linear, rangée par ce que vous avez à faire. Elle tourne sur votre Mac et ne parle qu’aux outils autorisés.
  image:
    src: /favicon.svg
    alt: Remora
  actions:
    - theme: brand
      text: Premiers pas
      link: /fr/guide/premiers-pas
    - theme: alt
      text: Connecter vos outils
      link: /fr/guide/connecter-vos-outils
    - theme: alt
      text: DSI et conformité
      link: /fr/admin/

features:
  - icon: '<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M9 14 4 9l5-5"/><path d="M4 9h10.5a5.5 5.5 0 0 1 0 11H11"/></svg>'
    title: Rangé par verbes
    details: À répondre, À relire, À corriger, Prêtes à fusionner, À faire. Les messages sont classés sur votre Mac, en français et en anglais, sans IA générative.
    link: /fr/guide/boite-de-reception
  - icon: '<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M20 14.5A8 8 0 1 1 9.5 4a6.5 6.5 0 0 0 10.5 10.5z"/></svg>'
    title: Reporter avec une raison
    details: Chaque raison propose un retour adapté, et les rappels se tirent directement de la barre des menus.
    link: /fr/guide/reporter-et-rappels
  - icon: '<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><circle cx="12" cy="13" r="8"/><path d="M12 9v4l2.5 2.5M10 2h4"/></svg>'
    title: Des relectures préparées
    details: Une estimation et ce que touche chaque modification, des sessions au clavier, et des relances rédigées pour vos merge requests.
    link: /fr/guide/relectures
  - icon: '<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 3 4.5 6v5.5c0 4.6 3.2 8.3 7.5 9.5 4.3-1.2 7.5-4.9 7.5-9.5V6z"/><path d="m9 12 2 2 4-4"/></svg>'
    title: Seulement les destinations autorisées
    details: Chaque extension déclare ses adresses et tout le reste est bloqué. L’IA externe est désactivée par défaut, et la DSI peut verrouiller la politique par MDM.
    link: /fr/admin/flux-de-donnees
  - icon: '<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="4" y="3" width="16" height="18" rx="2"/><path d="M9 7h2M13 7h2M9 11h2M13 11h2M9 15h2M13 15h2"/></svg>'
    title: Prête pour votre DSI
    details: Un profil de configuration pour les outils, l’IA et les avatars, un DMG à chaque version, et la liste complète de ce qui quitte le Mac.
    link: /fr/admin/mdm
  - icon: '<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M16 18l6-6-6-6M8 6l-6 6 6 6"/></svg>'
    title: Une extension par outil
    details: Un manifeste, une récupération et des tests sur fixtures. Le formulaire, le logo et le contrôle de conformité viennent avec.
    link: /fr/developper/extensions
---

<div class="home-shots">
  <Screenshot name="myturn" alt="L’onglet À moi : réponses, relectures avec leur estimation, vos merge requests et tâches" caption="À moi : ce que l’on attend de vous." />
  <Screenshot name="waiting" alt="L’onglet En attente : relecteurs suggérés et relance rédigée" caption="En attente : ce que vous attendez des autres." />
  <Screenshot name="snoozed" alt="L’onglet Reportés : ce qui revient lundi, et un conseil" caption="Reportés : de retour au bon moment." />
</div>

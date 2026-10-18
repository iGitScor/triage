---
title: Politique de signature du code
description: Comment les versions de Remora sont signées aujourd’hui et une fois la signature en place, qui approuve une signature, et l’engagement de confidentialité.
---

# Politique de signature du code

Signature gratuite du code sous Windows fournie par [SignPath.io](https://about.signpath.io), certificat de la
[SignPath Foundation](https://signpath.org).

Cette page est le seul endroit qui dit comment les versions sont signées ; les autres pages y renvoient. Les notes de
chaque version disent aussi comment ses propres fichiers ont été signés.

## Ce qui est signé

| Plateforme | Fichiers | Aujourd’hui | Une fois en place |
|---|---|---|---|
| macOS | `Remora.app` dans `Remora.dmg` | **Ad hoc**, sans notarisation. Gatekeeper demande la première fois, et le trousseau redemande une fois après chaque mise à jour, car macOS reconnaît une app ad hoc à son build exact. | Le certificat propre à Remora : macOS reconnaît chaque mise à jour comme la même app, et le trousseau cesse de demander. Ce n’est toujours pas un Developer ID d’Apple : Gatekeeper demande la première fois tant que l’app n’est pas notarisée. |
| Windows | `Remora-Setup.exe`, `Remora.msi` et le `Remora.exe` qu’ils installent | **Non signés.** SmartScreen avertit la première fois (*Informations complémentaires* → *Exécuter quand même*). | SignPath, avec le certificat de la SignPath Foundation. Windows affiche *SignPath Foundation* comme éditeur. Le MSI demande sa propre configuration d’artefact SignPath (`msi`). |
| Linux | `Remora-x86_64.AppImage`, `Remora-amd64.deb` | **Non signés**, comme la plupart des AppImage ; vérifiez le SHA-256 et la provenance de build ci-dessous. | Pas de changement prévu. |

Pour éviter ces demandes dès aujourd’hui, une organisation peut construire et signer sa propre copie avec son
Developer ID ou son certificat de signature de code : voir [Déployer Remora](./deploiement).

Seul le [workflow de publication](https://github.com/iGitScor/triage/blob/main/.github/workflows/release.yml) signe,
et seulement pour un tag de version poussé sur [le dépôt](https://github.com/iGitScor/triage). Rien de construit
ailleurs, y compris sur l’ordinateur d’un développeur, n’est signé avec ces certificats. Chaque version porte aussi
une provenance de build signée qui relie chaque fichier au commit dont il est issu (`gh attestation verify`, voir
[Déployer Remora](./deploiement)).

## Équipe et rôles

| Rôle | Qui |
|---|---|
| Contributeurs et relecteurs | Les [mainteneurs du dépôt](https://github.com/iGitScor/triage/graphs/contributors). Les contributions extérieures sont relues avant d’être fusionnées. |
| Approbateurs | [@iGitScor](https://github.com/iGitScor). Chaque demande de signature Windows est approuvée à la main dans SignPath. |

Tous les comptes de ces rôles utilisent l’authentification à plusieurs facteurs sur GitHub et SignPath.

## Confidentialité

Ce programme ne transmet aucune information à d’autres systèmes en réseau, sauf à la demande expresse de
l’utilisateur ou de la personne qui l’installe ou l’exploite. Remora ne parle qu’aux outils que vous connectez, et à
un assistant IA si vous en activez un : voir [Flux de données](./flux-de-donnees).

## Signaler un problème

Un fichier signé avec ces certificats que vous n’avez pas obtenu sur la
[page des versions](https://github.com/iGitScor/triage/releases), ou une signature qui ne se vérifie pas : signalez-le
en privé comme l’explique [SECURITY.md](https://github.com/iGitScor/triage/blob/main/SECURITY.md).

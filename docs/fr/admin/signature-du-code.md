---
title: Politique de signature du code
description: Comment les versions de Remora sont signées. Sous Windows, par SignPath avec le certificat de la SignPath Foundation ; sous macOS, avec le certificat propre à Remora, pas encore notarisé. Qui approuve une signature, ce qui est signé, et l’engagement de confidentialité.
---

# Politique de signature du code

Signature gratuite du code sous Windows fournie par [SignPath.io](https://about.signpath.io), certificat de la
[SignPath Foundation](https://signpath.org).

Cette signature est en cours de mise en place : d’ici là, les notes de chaque version disent si ses fichiers sont signés.

## Ce qui est signé

| Plateforme | Fichiers | Signé par |
|---|---|---|
| Windows | `Remora-Setup.exe` et le `Remora.exe` qu’il installe | SignPath, avec le certificat de la SignPath Foundation. Windows affiche *SignPath Foundation* comme éditeur. |
| macOS | `Remora.app` dans `Remora.dmg` | Le certificat propre à Remora : macOS reconnaît chaque mise à jour comme la même app, et le trousseau cesse de demander. Ce n’est pas un Developer ID d’Apple : l’app n’est pas encore notarisée, et Gatekeeper demande toujours la première fois. |

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

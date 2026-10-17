---
title: Remora et vos agents
description: Laissez Claude Desktop, Claude Code, Gemini CLI ou un autre agent d’IA de votre ordinateur demander à Remora ce qui vous attend, par MCP, sans leur donner vos jetons.
---

# Remora et vos agents

Les agents d’IA peuvent demander à Remora ce qui vous attend : ce qui demande votre attention, ce que vous attendez des
autres, combien d’éléments il y a. Ils le demandent par **MCP** (Model Context Protocol), le standard que parlent la
plupart des agents, et obtiennent la réponse de l’app Remora qui tourne sur votre ordinateur. Ils ne reçoivent jamais
vos jetons GitHub, Jira, Slack ou Teams, et rien ne passe par le réseau : l’agent et Remora se parlent sur votre
ordinateur.

## Ce qu’un agent peut faire

| Outil | Ce qu’il donne à l’agent |
|---|---|
| `waiting_on_me` | Les éléments d’un onglet, comme dans la barre des menus : *À moi* par défaut, ou *En cours*, *En attente*, *Reportés*, *Terminé*. Chaque élément a son outil, son type (*À répondre*, *À relire*…), son titre, son emplacement et son lien |
| `count` | Ce qui demande votre attention maintenant, le nombre de la barre des menus, et combien dans chaque onglet |
| `remind` | Ajoute un rappel dans Remora, sur cet ordinateur, quand vous demandez à votre agent de vous rappeler quelque chose |

Un agent **ne peut pas** marquer un élément comme terminé, le reporter, ni rien écrire dans vos outils : Remora ne
fait que les lire. Un outil coché dans **Masquer le contenu des messages** garde aussi ses titres pour les agents.
Remora indique à l’agent que les titres sont écrits par d’autres personnes et sont des données, pas des consignes :
un message ne peut pas piloter votre agent.

## L’installer

Remora doit être ouvert. Indiquez à votre agent la commande `remora` avec `mcp` :

| | Mac | Windows | Linux |
|---|---|---|---|
| Commande | `/Applications/Remora.app/Contents/Helpers/remora` | `%LOCALAPPDATA%\Remora\Remora.exe` | `remora` (le `.deb`), ou l’AppImage |
| Argument | `mcp` | `mcp` | `mcp` |

**Claude Desktop** : dans Réglages → Développeur → Modifier la configuration, ajoutez Remora à
`claude_desktop_config.json`, puis relancez Claude :

```json
{
  "mcpServers": {
    "remora": { "command": "/Applications/Remora.app/Contents/Helpers/remora", "args": ["mcp"] }
  }
}
```

**Claude Code** : `claude mcp add remora -- /Applications/Remora.app/Contents/Helpers/remora mcp`

**Gemini CLI** : la même entrée `mcpServers`, dans `~/.gemini/settings.json`.

Demandez ensuite, par exemple : « Qu’est-ce qui m’attend dans Remora ? », « Qu’est-ce que je dois aux autres ce
matin ? », « Rappelle-moi d’appeler Erin à 15 h ».

## Ce que Remora garde

Chaque demande d’un agent est gardée sur votre ordinateur, avec le nom que l’agent donne et l’heure, pour que vous
sachiez qui a lu quoi. Ce nom est celui que l’agent annonce : tout programme lancé sous votre compte peut se faire
passer pour n’importe quel agent. **Effacer les données locales** le supprime avec le reste. Rien n’est envoyé
nulle part.

## En entreprise

La DSI peut désactiver tout cela avec `AllowLocalAPI` : Remora n’ouvre alors aucune socket, la commande `remora` et
les agents reçoivent un message indiquant que l’organisation l’a désactivé, et rien sur l’ordinateur hormis l’app ne
peut lire la boîte. Voir [Gérer la politique (MDM)](/fr/admin/mdm).

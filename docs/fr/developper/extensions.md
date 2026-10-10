---
title: Écrire une extension
description: Ajouter un outil à Remora. Le contrat SourcePlugin, le manifeste qui génère le formulaire de réglages et déclare les sorties réseau, la conversion en InboxItem, les tests sur fixtures avec StubHTTP, les traductions et le portage Windows.
---

# Écrire une extension

Chaque intégration est une extension dans `macos/Sources/RemoraPlugins/<Name>/`. Les extensions ne dépendent que
de `RemoraCore`. Une extension, c’est deux choses : un **manifeste** (ce qu’elle est, ce que l’utilisateur
renseigne, où vont ses données) et une **récupération**, `fetch` (transformer l’API de l’outil en `InboxItem`). Le formulaire de
réglages, le logo dans la barre des menus, le panneau Confidentialité et le contrôle de conformité viennent tous du
manifeste.

## Le contrat

```swift
public protocol SourcePlugin: Sendable {
    static var manifest: PluginManifest { get }
    init(config: PluginConfig, http: HTTPClient) throws
    func fetch() async throws -> SourceSnapshot   // identity + items
}
```

`http` est un `GuardedHTTPClient` limité aux adresses que déclare le manifeste, plus celle du compte pour les outils
auto-hébergés. Une requête vers toute autre adresse échoue avec « Bloqué : … n’est pas une destination autorisée ».

## Une extension minimale

```swift
import Foundation
import RemoraCore

/// Open tickets assigned to you in Acme Desk.
public struct AcmeDeskPlugin: SourcePlugin {
    public static let manifest = PluginManifest(
        id: "acmedesk",                              // stable: stored in accounts and MDM policies
        name: "Acme Desk",
        symbol: "lifepreserver",                     // an SF Symbol, used where there's no logo
        summary: "Tickets assigned to you.",
        fields: [.token("API key", help: "Acme Desk → Profile → API keys.")],
        setupSteps: ["Click “Create a key”, name it Remora (read only), and paste it below."],
        setupLabel: "Create a key",
        setupURL: { _ in URL(string: "https://desk.acme.example/profile/keys") },
        egress: Egress(hosts: ["api.desk.acme.example"], description: "Reads tickets assigned to you."),
        logo: "acmedesk"                             // Resources/Logos/acmedesk.svg
    )

    private let accountID: UUID
    private let token: String
    private let http: HTTPClient

    public init(config: PluginConfig, http: HTTPClient) throws {
        accountID = config.accountID
        token = try config.required("token")         // fail early on a missing field
        self.http = http
    }

    public func fetch() async throws -> SourceSnapshot {
        var request = URLRequest(url: URL(string: "https://api.desk.acme.example/v1/me/tickets")!)
        request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        let (data, _) = try await http.send(request)
        let tickets = try JSONDecoder().decode(Tickets.self, from: data)
        return SourceSnapshot(identity: tickets.me, items: tickets.open.map { $0.item(accountID: accountID) })
    }
}
```

Ensuite :

1. Ajoutez-la à `PluginRegistry.sources` (`Sources/RemoraPlugins/PluginRegistry.swift`).
2. Ajoutez son logo dans `Resources/Logos/`, en SVG monochrome ([Simple Icons](https://simpleicons.org) a la
   plupart des outils, sous CC0), et créditez-le dans `Resources/LICENCES.txt` s’il vient d’ailleurs.
3. Ajoutez des tests (voir plus bas), et lancez `make test`.

## Conversion en éléments de la boîte

- **`id`** doit rester stable d’une actualisation à l’autre : `"\(accountID.uuidString)/<remote id>"`. Terminé,
  Épingler et Reporter s’appuient dessus.
- **`bundle`** : choisissez le *type* signalé par l’outil (`.reviews`, `.mentions`, `.directMessages`, `.authored`,
  `.tasks`). `VerbClassifier` le fait ensuite passer à un verbe (*À répondre*, *À corriger*…). N’ajoutez un groupe
  à `InboxBundle.swift` que pour une demande d’un genre vraiment nouveau.
- **`date`** est la dernière activité significative. Elle sert au tri, et un changement fait revenir les éléments
  terminés.
- **`needsAction`** : quelqu’un attend l’utilisateur. Il déclenche les notifications d’arrivée et le compteur de la
  barre des menus.
- **`priority`** et **`due`** quand l’outil les fournit : ils ordonnent le groupe.
- **`badges`** : des pastilles de statut avec un `Tone`. Un badge avec `notify` s’annonce la première fois qu’il
  apparaît (*Approuvée*, *Tests en échec*). Gardez des identifiants de badge stables (`approved`, `checks.failing`,
  `priority.urgent`…).
- **`changes`** (chemins de fichiers et nombres de lignes) active la préparation des relectures. Ne stockez jamais
  de code.
- **`url`** s’ouvre dans le navigateur ; **`appURL`** (`slack://…`, `linear://…`) ouvre l’app de bureau quand elle
  est installée.

## Options du manifeste

| Champ | Rôle |
|---|---|
| `fields` | Le formulaire de connexion. `.token(…)` et les champs avec `isSecret: true` vont dans le trousseau ; les autres dans `accounts.json`. `.host(default)` pour les outils auto-hébergés. |
| `setupSteps`, `setupLabel`, `setupURL` | Le guide numéroté et le bouton en haut du formulaire. `setupURL` reçoit la configuration, il peut donc pointer vers l’adresse propre de l’utilisateur. |
| `egress` | Les adresses et une description d’une phrase, affichées dans Réglages → Confidentialité et appliquées. `externalAI: true` pour les extensions qui envoient le contenu de la boîte à un fournisseur d’IA : elles restent désactivées tant que ce n’est pas autorisé. |
| `isComingSoon` | Listée avec un badge *Bientôt*, impossible à connecter. |
| `logo` | Le SVG de `Resources/Logos`, affiché dans les compteurs de la barre des menus. |

Une extension qui s’appuie sur un programme local utilise `CommandRunner` (voir `ClaudeCodePlugin`). Les extensions
d’assistant implémentent `AssistantPlugin` (`brief`, `digest`, `triage`) et vont dans `PluginRegistry.assistants`.

## Tests

Les extensions sont testées sur des fixtures JSON, jamais sur le réseau, avec `StubHTTP`
(`Tests/RemoraPluginsTests`) :

```swift
@Test func ticketsAssignedToMe() async throws {
    let http = StubHTTP(routes: ["/v1/me/tickets": #"{"me": "alice", "open": [{"id": "T-1", "title": "Printer on fire"}]}"#])
    let plugin = try AcmeDeskPlugin(config: config(["token": "k"]), http: http)
    let snapshot = try await plugin.fetch()
    #expect(snapshot.identity == "alice")
    #expect(snapshot.items.map(\.title) == ["Printer on fire"])
}
```

`ComplianceTests` vérifie que chaque manifeste du registre déclare ses sorties réseau : une nouvelle extension qui
n’en déclare pas fait échouer la CI. Utilisez des personnes fictives (alice, bob…) et des domaines fictifs
(`acme.example`) dans les fixtures.

## Traductions

Les textes affichés à l’utilisateur passent par `L("…")`. Ajoutez le français de chaque nouveau texte dans
`macos/scripts/translations_fr.py` ; `python3 scripts/make-strings.py` régénère `Localizable.strings` et liste les
textes encore sans traduction.

## Le portage Windows

L’app Windows a les mêmes extensions en Rust (`windows/crates/remora_plugins`), avec les mêmes requêtes,
conversions et fixtures : portez-y aussi l’extension, ou ouvrez une issue. Voir [Remora pour Windows](./windows).

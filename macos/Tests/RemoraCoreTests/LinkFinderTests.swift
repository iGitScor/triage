import Foundation
import NaturalLanguage
import Testing

@testable import RemoraCore

struct LinkFinderTests {
    func item(_ id: String, _ plugin: String, _ title: String, context: String = "") -> InboxItem {
        var item = makeItem(id, bundle: .tasks)
        item.pluginID = plugin
        item.title = title
        item.context = context
        return item
    }

    @Test func sharedTicketKeyLinksAcrossTools() {
        let issue = item("1", "linear", "Close Dotfile cases", context: "CE-1380")
        let pr = item("2", "github", "CE-1380: close cases on account closure")
        let links = LinkFinder().links([issue, pr])
        #expect(links["1"] == ["2"] && links["2"] == ["1"])
    }

    /// A refresh where nothing changed compares no title again; a renamed item, only its own pairs. The links
    /// stay the same.
    @Test func onlyNewOrChangedTitlesAreComparedAgain() {
        final class Counting: SimilarityModel, @unchecked Sendable {
            var asked = 0
            func similar(_ a: String, _ b: String) -> Bool {
                asked += 1
                return a.contains("export") && b.contains("export")
            }
        }
        let model = Counting()
        let finder = LinkFinder(similarity: [model])
        var items = [
            item("1", "linear", "CSV export is slow"), item("2", "github", "Speed up the CSV export"),
            item("3", "slack", "Lunch on Friday?"),
        ]
        let first = finder.links(items)
        #expect(model.asked == 3, "three pairs across tools")
        #expect(finder.links(items) == first && model.asked == 3, "nothing changed: nothing compared")
        items[2].title = "Team lunch moved"
        #expect(finder.links(items) == first)
        #expect(model.asked == 5, "the renamed item's two pairs only")
    }

    @Test func sameToolIsNeverLinked() {
        let a = item("1", "github", "CE-1380 part 1")
        let b = item("2", "github", "CE-1380 part 2")
        #expect(LinkFinder().links([a, b]).isEmpty)
    }

    @Test func prefixesAndTagsAreStripped() {
        #expect(LinkFinder.normalized("[SOAK] [WIP] chore(deps): Bump axios") == "Bump axios")
        #expect(LinkFinder.normalized("feat!: New search") == "New search")
    }

    @Test(.enabled(if: NLEmbedding.wordEmbedding(for: .english) != nil))
    func distinctiveWordsLinkButOrdinaryOnesDoNot() {
        let finder = LinkFinder(similarity: [
            KeywordSimilarity(), RareTokenSimilarity(),
            EmbeddingSimilarity(threshold: EmbeddingSimilarity.linkThreshold),
        ])
        let axiosIssue = item("1", "linear", "Update axios to address security vulnerabilities")
        let axiosPR = item("2", "github", "chore(deps): bump axios to 1.7.4")
        let fakerIssue = item("3", "linear", "Update @faker-js/faker to address security vulnerabilities")
        let addressPR = item("4", "github", "[SOAK] feat(api): add merchant address to virtual account- #13924")
        let links = finder.links([axiosIssue, axiosPR, fakerIssue, addressPR])
        #expect(links["1"] == ["2"])
        #expect(links["3"] == nil, "sharing the ordinary word “address” is not a link")
    }

    /// A first name or a typo shared by two unrelated titles is no link; a product or library name still is.
    @Test(.enabled(if: NLEmbedding.wordEmbedding(for: .english) != nil))
    func typosAndNamesAreNotDistinctive() {
        let rare = RareTokenSimilarity()
        #expect(rare.isTypo("recieve") && !rare.isTypo("nanoid") && !rare.isTypo("dotfile"))
        #expect(!rare.similar("Payments recieve the wrong currency", "Did you recieve the new office badges?"))
        #expect(!rare.similar("Onboarding call with Sacha on Monday", "Sacha shared photos of the team offsite"))
        #expect(rare.similar("Migrate Dotfile cases to the new queue", "Dotfile webhook fails since Monday"))
        #expect(rare.similar("Upgrade nanoid in checkout", "nanoid collisions in order ids"))
    }

    /// Without both on-device vocabularies (macOS fetches them on demand), ordinary words never link: only words with a
    /// digit, a dash or an underscore do. Runs everywhere, with made-up vocabularies.
    @Test func withoutBothVocabulariesOnlyTechnicalWordsLink() {
        let english: (String) -> Bool = { ["address", "review", "update", "form", "invoice"].contains($0) }
        for rare in [
            RareTokenSimilarity(vocabularies: [], expected: 2),
            RareTokenSimilarity(vocabularies: [english], expected: 2),
        ] {
            #expect(!rare.similar("Update the address form", "New address on the invoice"))
            #expect(!rare.similar("Réunion produit demain", "Compte rendu de la réunion"), "French unknown, not rare")
            #expect(rare.similar("Fix ENG-1234 crash", "eng-1234 still crashes"))
            #expect(rare.similar("Bump left_pad to 2.0", "left_pad breaks the build"))
        }
        // With both, a word neither knows is distinctive again.
        let complete = RareTokenSimilarity(vocabularies: [english, { _ in false }], expected: 2)
        #expect(complete.similar("Upgrade nanoid in checkout", "nanoid collisions in order ids"))
        #expect(!complete.similar("Update the address form", "New address on the invoice"))
    }

    /// Two tools mentioning UTF-8 are not about the same thing; a Linear key typed in lower case in Slack is.
    @Test func versionsDoNotLinkButALowerCaseKnownKeyDoes() {
        let issue = item("1", "linear", "Close Dotfile cases", context: "CE-1380")
        let slack = item("2", "slack", "can someone look at ce-1380?")
        let pr = item("3", "github", "Read files as utf-8")
        let message = item("4", "teams", "Export breaks on UTF-8 names")
        let links = LinkFinder(similarity: []).links([issue, slack, pr, message])
        #expect(links["1"] == ["2"] && links["2"] == ["1"])
        #expect(links["3"] == nil && links["4"] == nil)
    }

    /// More similar items than links shown: the closest are kept, not the first ones listed. Same case as Windows.
    @Test func theClosestSimilarItemsAreKept() {
        let links = LinkFinder().links([
            item("a", "github", "Export invoices to CSV for accounting"),
            item("b1", "linear", "Export invoices weekly accounting team report"),
            item("b4", "notion", "Invoices accounting export broken today"),
            item("b3", "jira", "Export invoices accounting monthly"), item("b2", "slack", "Export invoices accounting"),
        ])
        #expect(links["a"] == ["b2", "b3", "b4"], "all the same words, then three of four, then three of five")
    }
}

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

    /// ML-10: a refresh where nothing changed compares no title again; a renamed item, only its own pairs. The links
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

    /// ML-21: two tools mentioning UTF-8 are not about the same thing; a Linear key typed in lower case in Slack is.
    @Test func versionsDoNotLinkButALowerCaseKnownKeyDoes() {
        let issue = item("1", "linear", "Close Dotfile cases", context: "CE-1380")
        let slack = item("2", "slack", "can someone look at ce-1380?")
        let pr = item("3", "github", "Read files as utf-8")
        let message = item("4", "teams", "Export breaks on UTF-8 names")
        let links = LinkFinder(similarity: []).links([issue, slack, pr, message])
        #expect(links["1"] == ["2"] && links["2"] == ["1"])
        #expect(links["3"] == nil && links["4"] == nil)
    }
}

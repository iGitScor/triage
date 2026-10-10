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
            KeywordSimilarity(), RareTokenSimilarity(), EmbeddingSimilarity(threshold: 0.9),
        ])
        let axiosIssue = item("1", "linear", "Update axios to address security vulnerabilities")
        let axiosPR = item("2", "github", "chore(deps): bump axios to 1.7.4")
        let fakerIssue = item("3", "linear", "Update @faker-js/faker to address security vulnerabilities")
        let addressPR = item("4", "github", "[SOAK] feat(api): add merchant address to virtual account- #13924")
        let links = finder.links([axiosIssue, axiosPR, fakerIssue, addressPR])
        #expect(links["1"] == ["2"])
        #expect(links["3"] == nil, "sharing the ordinary word “address” is not a link")
    }
}

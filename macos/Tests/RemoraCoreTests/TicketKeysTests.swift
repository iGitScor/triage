import Testing

@testable import RemoraCore

/// ML-21: versions and encodings are not ticket keys, so they link nothing.
struct TicketKeysTests {
    @Test func versionsAndEncodingsAreNotKeys() {
        let text =
            "utf-8 UTF-8 sha-256 SHA-256 covid-19 node-20 NODE-20 Q4-2025 ubuntu-22 vue-router-4 xCE-12 SHA-256abc"
        #expect(TicketKeys().keys(text).isEmpty)
    }

    @Test func capitalKeysStartingAWordCount() {
        #expect(TicketKeys().keys("[CE-1380] fix, see /browse/ENG-7. and PAY2-3") == ["CE-1380", "ENG-7", "PAY2-3"])
    }

    @Test func knownPrefixesMatchInAnyCase() {
        #expect(TicketKeys().keys("blocked on ce-1380").isEmpty)
        #expect(TicketKeys(known: ["CE"]).keys("blocked on ce-1380") == ["CE-1380"])
    }

    @Test func aCVECountsWholeNeverByItsYear() {
        #expect(TicketKeys().keys("Patch cve-2024-3094 and CVE-2024-6387") == ["CVE-2024-3094", "CVE-2024-6387"])
        #expect(TicketKeys().keys("CVE-2024 review").isEmpty)
    }

    @Test func knownPrefixesComeFromContextsThatAreKeys() {
        var linear = makeItem("1", bundle: .tasks)
        linear.context = "CE-1380"
        var github = makeItem("2", bundle: .tasks)
        github.context = "acme/app #12"
        #expect(TicketKeys(knownIn: [linear, github]).known == ["CE"])
    }
}

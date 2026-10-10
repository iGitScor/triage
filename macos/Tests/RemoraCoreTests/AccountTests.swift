import Foundation
import Testing

@testable import RemoraCore

struct AccountTests {
    @Test func accountsSavedBeforeNamesStillDecode() throws {
        let json = #"{"id": "00000000-0000-0000-0000-000000000001", "pluginID": "slack", "settings": {}}"#
        let account = try JSONDecoder().decode(Account.self, from: Data(json.utf8))
        #expect(account.name == nil)
    }
}

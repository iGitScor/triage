import Foundation
import Testing
@testable import RemoraCore

/// Same cases as Windows (`text.rs`).
struct ReadableTests {
    @Test(arguments: [
        (":loudspeaker: _1 minute until this event:_", "📢 1 minute until this event:"),
        ("*Guests:* You _(organizer)_", "Guests: You (organizer)"),
        (":sparkles: Add passkeys :tada::skin-tone-3:", "✨ Add passkeys 🎉"),
        ("Fix `null` ids in **export**", "Fix null ids in export"),
        ("~old~ new, ~~gone~~", "old new, gone"),
        ("*_both_*", "both"),
        ("*a* *b* *c*", "a b c"),
        ("Keep my_var_name and 2*3*4", "Keep my_var_name and 2*3*4"),
        ("* a bullet", "* a bullet"),
        ("Meet at 10:15:30, :custom_party: ok", "Meet at 10:15:30, :custom_party: ok"),
        ("> quoted\nreply", "quoted\nreply"),
        ("```\nlet x = 1\n```", "let x = 1\n"),
    ])
    func readsAsPeopleDo(text: String, expected: String) {
        #expect(Readable.text(text) == expected)
    }
}

/// Counts take the right form in each language.
struct PluralTests {
    @Test func oneIsSingularAndFrenchCountsZeroToo() {
        #expect(AppLanguage.isSingular(1, french: false) && !AppLanguage.isSingular(0, french: false) && !AppLanguage.isSingular(2, french: false))
        #expect(AppLanguage.isSingular(1, french: true) && AppLanguage.isSingular(0, french: true) && !AppLanguage.isSingular(2, french: true))
    }

    @Test func theFormFollowsTheCount() {
        // Tests run in English: the keys are the text.
        #expect(L("%d file", plural: "%d files", 1) == "1 file")
        #expect(L("%d file", plural: "%d files", 3) == "3 files")
    }
}

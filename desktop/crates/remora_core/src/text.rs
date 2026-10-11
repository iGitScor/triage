//! Text from the tools as people read it: emoji codes as emoji, and no formatting marks (`*bold*`, `_italic_`,
//! `~strike~`, `` `code` ``, `**bold**`, a quote's `>`). Every plugin's titles and Slack's messages go through it,
//! so the inbox and the sorting rules read the same words. Same rules on macOS (`Readable.swift`).

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::{Captures, Regex};

use crate::emoji;

static EMOJI: LazyLock<HashMap<&str, &str>> = LazyLock::new(|| emoji::TABLE.iter().copied().collect());
static CODE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r":([a-z0-9_+\-]*[a-z][a-z0-9_+\-]*):").expect("valid pattern"));
static SKIN_TONE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r":skin-tone-[2-6]:").expect("valid pattern"));
static FENCE: LazyLock<Regex> = LazyLock::new(|| Regex::new("```\n?").expect("valid pattern"));
static INLINE_CODE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"`([^`\n]+)`").expect("valid pattern"));
static QUOTE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^>[ \t]?").expect("valid pattern"));
/// A mark opens after the start or a non-word character and before a non-space, and closes before the end or a
/// non-word character, on one line: `snake_case`, `2*3*4` and `* bullet` keep theirs. Pairs before single marks.
static MARKS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [r"\*\*", "__", "~~", r"\*", "_", "~"]
        .iter()
        .map(|mark| {
            Regex::new(&format!(r"(^|[^\p{{L}}\p{{N}}]){mark}(\S(?:[^\n]*?\S)??){mark}($|[^\p{{L}}\p{{N}}])"))
                .expect("valid pattern")
        })
        .collect()
});

pub fn readable(text: &str) -> String {
    let mut result = emoji(text);
    result = FENCE.replace_all(&result, "").into_owned();
    result = INLINE_CODE.replace_all(&result, "$1").into_owned();
    // A match uses the character after it, which can start the next one: again until nothing changes, which also
    // takes a mark inside another (`*_both_*`).
    for _ in 0..4 {
        let before = result.clone();
        for mark in MARKS.iter() {
            result = mark.replace_all(&result, "$1$2$3").into_owned();
        }
        if result == before {
            break;
        }
    }
    QUOTE.replace_all(&result, "").into_owned()
}

/// `:tada:` → 🎉. Codes not in the table (a workspace's own emoji) stay as written; skin tones are dropped.
pub fn emoji(text: &str) -> String {
    if !text.contains(':') {
        return text.to_string();
    }
    let text = SKIN_TONE.replace_all(text, "");
    CODE.replace_all(&text, |c: &Captures| EMOJI.get(&c[1]).map_or_else(|| c[0].to_string(), |e| e.to_string()))
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Same cases as macOS (`ReadableTests`).
    #[test]
    fn reads_as_people_do() {
        let cases = [
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
        ];
        for (text, expected) in cases {
            assert_eq!(readable(text), expected, "{text}");
        }
    }
}

//! What the interface shows the user when something goes wrong, or wants a word: a stable code and its values,
//! translated there by code (`MESSAGES` in i18n.ts), never by matching the English sentence. `text` is the English,
//! for logs, tests and the fallback; a sentence written elsewhere (a server, the system) has the code `text` and is
//! shown as it is.

use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub code: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub text: String,
}

/// A sentence written elsewhere, or one the dictionary has as is.
pub const TEXT: &str = "text";

impl Message {
    pub fn new(code: &str, args: Vec<String>, text: impl Into<String>) -> Self {
        Message { code: code.into(), args, text: text.into() }
    }

    pub fn is_text(&self) -> bool {
        self.code == TEXT
    }
}

impl From<String> for Message {
    fn from(text: String) -> Self {
        Message { code: TEXT.into(), args: vec![], text }
    }
}

impl From<&str> for Message {
    fn from(text: &str) -> Self {
        text.to_string().into()
    }
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

impl std::error::Error for Message {}

/// Compared with a sentence, a message is its English text: what tests and logs read.
impl PartialEq<&str> for Message {
    fn eq(&self, other: &&str) -> bool {
        self.text == *other
    }
}

impl PartialEq<String> for Message {
    fn eq(&self, other: &String) -> bool {
        &self.text == other
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_message_travels_as_its_code_values_and_english() {
        let message = Message::new("missingField", vec!["token".into()], "“token” is required.");
        assert_eq!(
            serde_json::to_value(&message).unwrap(),
            serde_json::json!({ "code": "missingField", "args": ["token"], "text": "“token” is required." })
        );
        let said: Message = "Offline".into();
        assert!(said.is_text() && said.args.is_empty() && said.text == "Offline" && format!("{said}") == "Offline");
    }
}

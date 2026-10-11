//! What an assistant's reply can say, whoever wrote it (AI-15): the Claude API, Claude Code and an OpenAI-compatible
//! server each implement `AssistantReply` for their raw answer, and `decode_reply` turns any of them into the brief,
//! a summary or triage suggestions, or into an `AssistantFailure` that names the cause in the same words for all.
//! Transport errors (status, timeout, retries, a program that fails before answering) stay with each plugin.
//! A port of the macOS `AssistantReply.swift`.

use crate::PluginError;
use serde::de::DeserializeOwned;

/// Why an assistant gave no usable answer, provider-neutral. The English is the key the interface translates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssistantFailure {
    /// It stopped at its length limit: half a JSON is unusable.
    CutOff,
    Declined,
    UnexpectedAnswer,
    /// It couldn't fit its answer to the requested schema.
    NoStructuredAnswer,
    /// It ran out of steps (turns) before answering.
    TooManySteps,
    SpendingLimit,
    /// The provider's own message, already worded by the plugin ("Claude Code: …").
    Failed(String),
}

impl AssistantFailure {
    /// The one place these causes are worded. Same text on macOS.
    pub fn message(&self) -> String {
        match self {
            Self::CutOff => CUT_OFF,
            Self::Declined => DECLINED,
            Self::UnexpectedAnswer => UNEXPECTED_ANSWER,
            Self::NoStructuredAnswer => NO_STRUCTURED_ANSWER,
            Self::TooManySteps => TOO_MANY_STEPS,
            Self::SpendingLimit => SPENDING_LIMIT,
            Self::Failed(message) => message,
        }
        .to_string()
    }
}

// Constants, so the French dictionary check (`make-i18n.py`) finds them.
const CUT_OFF: &str = "The assistant’s answer was cut off. Try again, or with fewer items.";
const DECLINED: &str = "The assistant declined this request.";
const UNEXPECTED_ANSWER: &str = "The assistant returned an unexpected answer.";
const NO_STRUCTURED_ANSWER: &str =
    "The assistant couldn’t answer in the expected form. Try again, or with fewer items.";
const TOO_MANY_STEPS: &str = "The assistant stopped before answering. Try again.";
const SPENDING_LIMIT: &str = "The assistant reached its spending limit.";

/// Shown like any other plugin error.
impl From<AssistantFailure> for PluginError {
    fn from(failure: AssistantFailure) -> Self {
        PluginError::Api(failure.message())
    }
}

/// A provider's raw reply: its answer as JSON, or why there is none.
pub trait AssistantReply {
    fn answer(self) -> Result<serde_json::Value, AssistantFailure>;
}

/// Reads the JSON text an assistant wrote (the Claude API's text block, an OpenAI message's content).
pub fn json_text(text: Option<String>) -> Result<serde_json::Value, AssistantFailure> {
    text.and_then(|t| serde_json::from_str(&t).ok()).ok_or(AssistantFailure::UnexpectedAnswer)
}

/// The brief, a summary or triage from any assistant's reply: an answer that doesn't fit `T` is unexpected.
pub fn decode_reply<T: DeserializeOwned>(reply: impl AssistantReply) -> Result<T, AssistantFailure> {
    serde_json::from_value(reply.answer()?).map_err(|_| AssistantFailure::UnexpectedAnswer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    struct Reply(Result<serde_json::Value, AssistantFailure>);

    impl AssistantReply for Reply {
        fn answer(self) -> Result<serde_json::Value, AssistantFailure> {
            self.0
        }
    }

    #[derive(Deserialize, Debug, PartialEq)]
    struct Output {
        summary: String,
    }

    #[test]
    fn decodes_an_answer_or_says_why_not() {
        let ok = Reply(Ok(serde_json::json!({ "summary": "s" })));
        assert_eq!(decode_reply::<Output>(ok), Ok(Output { summary: "s".into() }));
        let wrong_shape = Reply(Ok(serde_json::json!({ "focus": [] })));
        assert_eq!(decode_reply::<Output>(wrong_shape), Err(AssistantFailure::UnexpectedAnswer));
        assert_eq!(decode_reply::<Output>(Reply(Err(AssistantFailure::CutOff))), Err(AssistantFailure::CutOff));
        assert_eq!(json_text(Some("not json".into())), Err(AssistantFailure::UnexpectedAnswer));
        assert_eq!(json_text(None), Err(AssistantFailure::UnexpectedAnswer));
    }

    #[test]
    fn every_cause_is_shown_as_a_plugin_error() {
        assert_eq!(
            PluginError::from(AssistantFailure::CutOff),
            PluginError::Api("The assistant’s answer was cut off. Try again, or with fewer items.".into())
        );
        assert_eq!(
            PluginError::from(AssistantFailure::Failed("Claude Code: boom".into())),
            PluginError::Api("Claude Code: boom".into())
        );
    }
}

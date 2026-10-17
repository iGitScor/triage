//! What an assistant's reply can say, whoever wrote it: the Claude API, Claude Code and an OpenAI-compatible
//! server each implement `AssistantReply` for their raw answer, and `decode_reply` turns any of them into the brief,
//! a summary or triage suggestions, or into an `AssistantFailure` that names the cause in the same words for all.
//! Transport errors (status, timeout, retries, a program that fails before answering) stay with each plugin.
//! A port of the macOS `AssistantReply.swift`.

use crate::PluginError;
use remora_core::TokenUsage;
use serde::de::DeserializeOwned;
use std::future::Future;
use std::sync::{Arc, Mutex};

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
    /// The tokens it used, when the provider says.
    fn usage(&self) -> Option<TokenUsage> {
        None
    }
}

tokio::task_local! {
    static METER: AssistantMeter;
}

/// Adds up the tokens the answers within one request report (a retry, Claude Code's older-version rerun). The app
/// measures each request with one; a plugin reports through `decode_reply`, outside of one it's dropped. As the
/// macOS `AssistantMeter`.
#[derive(Clone, Default)]
pub struct AssistantMeter(Arc<Mutex<Option<TokenUsage>>>);

impl AssistantMeter {
    /// Runs one request with this meter counting.
    pub async fn measure<F: Future>(&self, request: F) -> F::Output {
        METER.scope(self.clone(), request).await
    }

    /// None when no answer said what it used.
    pub fn tokens(&self) -> Option<TokenUsage> {
        *self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn report(usage: Option<TokenUsage>) {
        let Some(usage) = usage else { return };
        let _ = METER.try_with(|meter| {
            let mut counted = meter.0.lock().unwrap_or_else(|e| e.into_inner());
            let total = counted.unwrap_or_default();
            *counted = Some(TokenUsage { input: total.input + usage.input, output: total.output + usage.output });
        });
    }
}

/// Reads the JSON text an assistant wrote (the Claude API's text block, an OpenAI message's content).
pub fn json_text(text: Option<String>) -> Result<serde_json::Value, AssistantFailure> {
    text.and_then(|t| serde_json::from_str(&t).ok()).ok_or(AssistantFailure::UnexpectedAnswer)
}

/// The brief, a summary or triage from any assistant's reply: an answer that doesn't fit `T` is unexpected. What it
/// used is counted either way: a cut-off answer was still paid for.
pub fn decode_reply<T: DeserializeOwned>(reply: impl AssistantReply) -> Result<T, AssistantFailure> {
    AssistantMeter::report(reply.usage());
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

    /// What every answer within a request reports adds up; outside of a request it's dropped.
    #[tokio::test]
    async fn the_meter_adds_up_what_its_request_reports() {
        let meter = AssistantMeter::default();
        meter
            .measure(async {
                AssistantMeter::report(Some(TokenUsage { input: 10, output: 2 }));
                AssistantMeter::report(None);
                AssistantMeter::report(Some(TokenUsage { input: 5, output: 1 }));
            })
            .await;
        AssistantMeter::report(Some(TokenUsage { input: 99, output: 99 }));
        assert_eq!(meter.tokens(), Some(TokenUsage { input: 15, output: 3 }));
        assert_eq!(AssistantMeter::default().tokens(), None, "nothing reported");
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

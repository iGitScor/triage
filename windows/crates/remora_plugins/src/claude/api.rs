//! Asks Claude, through an Anthropic API key, for a short brief: what matters now and why. A port of the macOS
//! `ClaudePlugin.swift`.

use super::prompt::{self, Schema};
use crate::{decode, AssistantPlugin, HttpClient, PluginConfig, PluginError, Request};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use remora_core::{Brief, ConfigField, Egress, InboxItem, PluginManifest, SnoozedItem, TriageSuggestion};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

pub const ID: &str = "claude";
pub const DEFAULT_MODEL: &str = "claude-opus-5-5";
pub const DEFAULT_DIGEST_MODEL: &str = "claude-haiku-5-5";
const ENDPOINT: &str = "https://api.anthropic.com/v1/messages";

/// The answer isn't streamed, so nothing arrives until it's complete: allow as long as Anthropic's own SDKs do
///. Other requests keep the client's 30 s.
pub const TIMEOUT: Duration = Duration::from_secs(600);

const TOO_SLOW: &str = "Claude took too long to answer. Try again later.";
const DECLINED: &str = "Claude declined this request.";
const CUT_OFF: &str = "Claude’s answer was cut off. Try again, or with fewer items.";
const UNEXPECTED: &str = "Claude returned an unexpected answer.";

/// The same words as the macOS app, which the shared French dictionary translates.
pub fn manifest() -> PluginManifest {
    let model = |key: &str, label: &str, default: &str| ConfigField {
        key: key.into(),
        label: label.into(),
        placeholder: String::new(),
        default_value: default.into(),
        is_secret: false,
        is_optional: false,
        help: None,
    };
    PluginManifest {
        id: ID.into(),
        name: "Claude API".into(),
        summary: "The brief through an Anthropic API key. Item titles and context are sent to Anthropic.".into(),
        fields: vec![
            ConfigField::token("API key", "An Anthropic API key from the Claude Console."),
            model("model", "Model for the brief", DEFAULT_MODEL),
            model("digestModel", "Model for bundle summaries", DEFAULT_DIGEST_MODEL),
        ],
        setup_steps: vec![
            "Click “Create an API key” and sign in to the Claude Console.".into(),
            "Create a key and paste it below.".into(),
        ],
        setup_label: "Create an API key".into(),
        setup_url: Some("https://platform.claude.com/settings/keys".into()),
        egress: Egress {
            hosts: vec!["api.anthropic.com".into()],
            description: "Sends the titles, contexts, authors and statuses of inbox items to Anthropic.".into(),
            external_ai: true,
        },
        logo: Some("anthropic".into()),
    }
}

pub struct ClaudePlugin {
    api_key: String,
    model: String,
    digest_model: String,
    language: String,
    http: Arc<dyn HttpClient>,
}

#[derive(Serialize)]
pub struct ApiRequest {
    pub model: String,
    pub max_tokens: u32,
    pub system: String,
    pub messages: Vec<Message>,
    pub output_config: OutputConfig,
    /// Server-side fallback on refusal; not available for Haiku.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallbacks: Option<&'static str>,
}

#[derive(Serialize)]
pub struct Message {
    pub role: &'static str,
    pub content: String,
}

#[derive(Serialize)]
pub struct OutputConfig {
    pub effort: &'static str,
    pub format: Format,
}

#[derive(Serialize)]
pub struct Format {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub schema: Schema,
}

impl ApiRequest {
    fn new(model: &str, max_tokens: u32, system: String, message: String, schema: Schema, fallbacks: bool) -> Self {
        ApiRequest {
            model: model.into(),
            max_tokens,
            system,
            messages: vec![Message { role: "user", content: message }],
            output_config: OutputConfig { effort: "low", format: Format { kind: "json_schema", schema } },
            fallbacks: fallbacks.then_some("default"),
        }
    }
}

#[derive(Deserialize)]
pub struct ApiResponse {
    pub content: Vec<Block>,
    pub stop_reason: Option<String>,
}

#[derive(Deserialize)]
pub struct Block {
    #[serde(rename = "type")]
    pub kind: String,
    pub text: Option<String>,
}

impl ClaudePlugin {
    pub fn new(config: &PluginConfig, http: Arc<dyn HttpClient>, language: &str) -> Result<Self, PluginError> {
        let or = |key: &str, default: &str| {
            Some(config.get(key)).filter(|v| !v.is_empty()).unwrap_or_else(|| default.into())
        };
        Ok(ClaudePlugin {
            api_key: config.required("token")?,
            model: or("model", DEFAULT_MODEL),
            digest_model: or("digestModel", DEFAULT_DIGEST_MODEL),
            language: language.into(),
            http,
        })
    }

    /// The brief's request.
    pub fn body(items: &[InboxItem], model: &str, now: DateTime<Utc>, language: &str) -> ApiRequest {
        ApiRequest::new(model, 16_000, prompt::system(language), prompt::message(items, now), Schema::brief(), true)
    }

    async fn send(&self, body: &ApiRequest) -> Result<ApiResponse, PluginError> {
        let mut request = Request::post_json(ENDPOINT, body)?
            .header("x-api-key", self.api_key.clone())
            .header("anthropic-version", "2023-06-01")
            .timeout(TIMEOUT);
        if body.fallbacks.is_some() {
            request = request.header("anthropic-beta", "server-side-fallback-2026-07-01");
        }
        let mut attempt = 0;
        loop {
            match decode(self.http.as_ref(), request.clone()).await {
                Err(PluginError::TimedOut) => return Err(PluginError::Api(TOO_SLOW.into())),
                Err(error) => match retry_delay(&error, attempt, Utc::now()) {
                    Some(delay) => {
                        attempt += 1;
                        tokio::time::sleep(delay).await;
                    }
                    None => return Err(error),
                },
                ok => return ok,
            }
        }
    }

    /// A brief from the API's answer, without the items Claude may have made up.
    pub fn parse(response: ApiResponse, known_ids: &HashSet<String>) -> Result<Brief, PluginError> {
        output::<prompt::Output>(response).map(|o| o.brief(known_ids))
    }
}

/// Retries an overloaded or rate-limited answer up to twice (AI-04): 429, 408, 409 and 5xx (529 is "overloaded").
/// A rate limit waits as long as `retry-after` says, up to a minute; beyond that, or without it, 2 s then 4 s.
/// `None`: give up and show the error. Same rule on macOS (`ClaudePlugin.retryDelay`).
pub fn retry_delay(error: &PluginError, attempt: u32, now: DateTime<Utc>) -> Option<Duration> {
    if attempt >= 2 {
        return None;
    }
    let backoff = Duration::from_secs(2 << attempt);
    match error {
        PluginError::RateLimited(Some(until)) => {
            let wait = until - now.timestamp();
            (wait <= 60).then(|| Duration::from_secs(wait.max(0) as u64))
        }
        PluginError::RateLimited(None) => Some(backoff),
        PluginError::Status(408 | 409 | 500..) => Some(backoff),
        _ => None,
    }
}

fn output<T: DeserializeOwned>(response: ApiResponse) -> Result<T, PluginError> {
    match response.stop_reason.as_deref() {
        Some("refusal") => return Err(PluginError::Api(DECLINED.into())),
        // Thinking counts toward max_tokens: a long one can leave the answer cut off, and half a JSON is unusable.
        Some("max_tokens") => return Err(PluginError::Api(CUT_OFF.into())),
        _ => {}
    }
    let text = response.content.into_iter().find(|b| b.kind == "text").and_then(|b| b.text);
    text.and_then(|t| serde_json::from_str(&t).ok()).ok_or_else(|| PluginError::Api(UNEXPECTED.into()))
}

#[async_trait]
impl AssistantPlugin for ClaudePlugin {
    async fn brief(&self, items: &[InboxItem], now: DateTime<Utc>) -> Result<Brief, PluginError> {
        let response = self.send(&Self::body(items, &self.model, now, &self.language)).await?;
        Self::parse(response, &prompt::ids(items))
    }

    async fn digest(&self, items: &[InboxItem], topic: &str, now: DateTime<Utc>) -> Result<String, PluginError> {
        let body = ApiRequest::new(
            &self.digest_model,
            2_000,
            prompt::digest_system(&self.language),
            prompt::digest_message(items, topic, now),
            Schema::digest(),
            false,
        );
        output::<prompt::DigestOutput>(self.send(&body).await?).map(|o| o.summary)
    }

    async fn triage(&self, items: &[SnoozedItem], now: DateTime<Utc>) -> Result<Vec<TriageSuggestion>, PluginError> {
        let body = ApiRequest::new(
            &self.model,
            8_000,
            prompt::triage_system(&self.language),
            prompt::triage_message(items, now),
            Schema::triage(),
            true,
        );
        let output: prompt::TriageOutput = output(self.send(&body).await?)?;
        Ok(output.suggestions(&prompt::snoozed_ids(items), now))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::claude::testing::{item, now, snoozed};
    use crate::stub::{config, StubHttp};
    use crate::{GuardedHttpClient, Response};
    use std::sync::Mutex;

    fn plugin(http: impl HttpClient + 'static) -> ClaudePlugin {
        ClaudePlugin::new(&config(&[("token", "k")]), Arc::new(http), "en").unwrap()
    }

    /// Answers every request with `body` and `status`, and keeps the requests.
    struct Recorder {
        status: u16,
        body: Result<String, PluginError>,
        requests: Mutex<Vec<Request>>,
    }

    impl Recorder {
        fn new(status: u16, body: Result<&str, PluginError>) -> Arc<Self> {
            Arc::new(Recorder { status, body: body.map(str::to_string), requests: Mutex::new(vec![]) })
        }

        fn request(&self) -> Request {
            self.requests.lock().unwrap()[0].clone()
        }
    }

    #[async_trait]
    impl HttpClient for Recorder {
        async fn send(&self, request: Request) -> Result<Response, PluginError> {
            self.requests.lock().unwrap().push(request);
            self.body.clone().map(|body| Response { status: self.status, body })
        }
    }

    fn answer(text: &str, stop: &str) -> String {
        serde_json::json!({ "content": [{ "type": "thinking" }, { "type": "text", "text": text }], "stop_reason": stop }).to_string()
    }

    #[test]
    fn request_uses_structured_output_and_fallbacks() {
        let json = serde_json::to_string(&ClaudePlugin::body(&[item("a")], "claude-opus-5-5", now(), "en")).unwrap();
        assert!(json.contains(r#""fallbacks":"default""#));
        assert!(json.contains(r#""type":"json_schema""#));
        assert!(json.contains(r#""additionalProperties":false"#));
        assert!(json.contains(r#""max_tokens":16000"#));
        assert!(json.contains(r#""output_config":{"effort":"low""#));
        assert!(json.contains("Fix login"));
    }

    #[tokio::test]
    async fn parses_the_brief_and_drops_unknown_items() {
        let text = r#"{"summary": "One review waits.", "focus": [{"id": "a", "reason": "Blocks Frank"}, {"id": "ghost", "reason": "?"}]}"#;
        let http = StubHttp::paths(&[("/v1/messages", &answer(text, "end_turn"))]);
        let brief = plugin(http).brief(&[item("a")], now()).await.unwrap();
        assert_eq!(brief.summary, "One review waits.");
        assert_eq!(brief.focus, [remora_core::Focus { id: "a".into(), reason: "Blocks Frank".into() }]);
    }

    #[tokio::test]
    async fn sends_the_key_and_version_to_anthropic_only() {
        let recorder = Recorder::new(200, Ok(&answer(r#"{"summary": "s", "focus": []}"#, "end_turn")));
        let guarded = GuardedHttpClient::new(recorder.clone(), manifest().egress.hosts);
        plugin(guarded).brief(&[item("a")], now()).await.unwrap();
        let request = recorder.request();
        assert_eq!(request.url, "https://api.anthropic.com/v1/messages");
        let header = |name: &str| request.headers.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone());
        assert_eq!(header("x-api-key").as_deref(), Some("k"));
        assert_eq!(header("anthropic-version").as_deref(), Some("2023-06-01"));
        assert_eq!(header("anthropic-beta").as_deref(), Some("server-side-fallback-2026-07-01"));
    }

    #[tokio::test]
    async fn digest_uses_the_lighter_model_without_fallbacks() {
        let recorder = Recorder::new(200, Ok(&answer(r#"{"summary": "Two reviews wait."}"#, "end_turn")));
        let summary = ClaudePlugin::new(&config(&[("token", "k")]), recorder.clone(), "fr")
            .unwrap()
            .digest(&[item("a")], "To review", now())
            .await
            .unwrap();
        assert_eq!(summary, "Two reviews wait.");
        let request = recorder.request();
        let body: serde_json::Value = serde_json::from_str(request.body.as_deref().unwrap()).unwrap();
        assert_eq!(body["model"], "claude-haiku-5-5");
        assert_eq!(body["max_tokens"], 2000);
        assert!(body.get("fallbacks").is_none(), "no fallback for Haiku");
        assert!(body["system"].as_str().unwrap().ends_with(" Write in French."));
        assert!(body["messages"][0]["content"].as_str().unwrap().starts_with("Group: To review."));
        assert!(!request.headers.iter().any(|(k, _)| k == "anthropic-beta"));
    }

    #[tokio::test]
    async fn triage_keeps_known_items_only() {
        let later = prompt::iso8601(now() + chrono::Duration::days(1));
        let text = format!(
            r#"{{"suggestions": [{{"id": "a", "action": "reschedule", "until": "{later}", "reason": "r"}}, {{"id": "x", "action": "done", "until": "", "reason": "?"}}]}}"#
        );
        let recorder = Recorder::new(200, Ok(&answer(&text, "end_turn")));
        let suggestions = plugin(ArcClient(recorder.clone())).triage(&[snoozed(item("a"), 1)], now()).await.unwrap();
        assert_eq!(suggestions.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(), ["a"]);
        let body: serde_json::Value = serde_json::from_str(recorder.request().body.as_deref().unwrap()).unwrap();
        assert_eq!((body["model"].as_str(), body["max_tokens"].as_u64()), (Some("claude-opus-5-5"), Some(8000)));
        assert!(body["messages"][0]["content"].as_str().unwrap().contains("<snoozed_items>"));
    }

    /// A non-streamed answer is silent until complete, so Claude calls get a long timeout; the tools keep 30 s.
    #[tokio::test]
    async fn claude_calls_wait_longer_than_tool_calls() {
        let recorder = Recorder::new(200, Err(PluginError::TimedOut));
        let result = plugin(ArcClient(recorder.clone())).brief(&[item("a")], now()).await;
        assert_eq!(result.err(), Some(PluginError::Api(TOO_SLOW.into())));
        assert_eq!(recorder.request().timeout, Some(TIMEOUT));
        assert!(TIMEOUT >= Duration::from_secs(300));
        assert_eq!(Request::get("https://api.github.com").timeout, None, "the client's 30 s");
    }

    #[tokio::test]
    async fn a_cut_off_answer_says_so() {
        let http = StubHttp::paths(&[("/v1/messages", &answer(r#"{"summary": "One rev"#, "max_tokens"))]);
        assert_eq!(plugin(http).brief(&[item("a")], now()).await.err(), Some(PluginError::Api(CUT_OFF.into())));
    }

    #[tokio::test]
    async fn a_refusal_becomes_an_error() {
        let http = StubHttp::paths(&[("/v1/messages", r#"{"content": [], "stop_reason": "refusal"}"#)]);
        assert_eq!(plugin(http).brief(&[item("a")], now()).await.err(), Some(PluginError::Api(DECLINED.into())));
    }

    #[tokio::test]
    async fn a_strange_answer_or_a_rejected_key_says_so() {
        let http = StubHttp::paths(&[("/v1/messages", &answer("not JSON", "end_turn"))]);
        assert_eq!(plugin(http).brief(&[item("a")], now()).await.err(), Some(PluginError::Api(UNEXPECTED.into())));
        let rejected = Recorder::new(401, Ok(r#"{"type": "error"}"#));
        assert_eq!(plugin(ArcClient(rejected)).brief(&[item("a")], now()).await.err(), Some(PluginError::Unauthorized));
    }

    #[test]
    fn a_key_is_required_and_models_have_defaults() {
        assert_eq!(
            ClaudePlugin::new(&config(&[]), Arc::new(StubHttp::paths(&[])), "en").err(),
            Some(PluginError::MissingField("token".into()))
        );
        let manifest = manifest();
        assert_eq!(manifest.id, "claude");
        assert_eq!(manifest.egress.hosts, ["api.anthropic.com"]);
        assert!(manifest.egress.external_ai);
        assert_eq!(
            manifest.fields.iter().map(|f| (f.key.as_str(), f.default_value.as_str())).collect::<Vec<_>>(),
            [("token", ""), ("model", "claude-opus-5-5"), ("digestModel", "claude-haiku-5-5"),]
        );
    }

    /// An `Arc<Recorder>` the plugin can own while the test keeps reading it.
    struct ArcClient(Arc<Recorder>);

    #[async_trait]
    impl HttpClient for ArcClient {
        async fn send(&self, request: Request) -> Result<Response, PluginError> {
            self.0.send(request).await
        }
    }

    /// Answers with each status in turn, then 200 with a brief.
    struct Flaky {
        statuses: Mutex<Vec<u16>>,
        calls: Mutex<usize>,
    }

    #[async_trait]
    impl HttpClient for Flaky {
        async fn send(&self, _request: Request) -> Result<Response, PluginError> {
            *self.calls.lock().unwrap() += 1;
            let mut statuses = self.statuses.lock().unwrap();
            let status = if statuses.is_empty() { 200 } else { statuses.remove(0) };
            Ok(Response { status, body: answer(r#"{"summary":"Fine.","focus":[]}"#, "end_turn") })
        }
    }

    #[tokio::test(start_paused = true)]
    async fn overloaded_answers_are_retried_twice() {
        let flaky = Arc::new(Flaky { statuses: Mutex::new(vec![529, 503]), calls: Mutex::new(0) });
        let started = tokio::time::Instant::now();
        let brief = ClaudePlugin::new(&config(&[("token", "k")]), flaky.clone(), "en")
            .unwrap()
            .brief(&[item("a")], now())
            .await;
        assert_eq!(brief.unwrap().summary, "Fine.");
        assert_eq!(*flaky.calls.lock().unwrap(), 3);
        assert_eq!(started.elapsed(), Duration::from_secs(6), "2 s, then 4 s");

        let down = Arc::new(Flaky { statuses: Mutex::new(vec![529; 4]), calls: Mutex::new(0) });
        let error =
            ClaudePlugin::new(&config(&[("token", "k")]), down.clone(), "en").unwrap().brief(&[item("a")], now()).await;
        assert_eq!(error.unwrap_err(), PluginError::Status(529));
        assert_eq!(*down.calls.lock().unwrap(), 3);
    }

    #[test]
    fn retries_wait_what_the_server_says_up_to_a_minute() {
        let at = now();
        let in_secs = |s: i64| PluginError::RateLimited(Some(at.timestamp() + s));
        assert_eq!(retry_delay(&in_secs(20), 0, at), Some(Duration::from_secs(20)));
        assert_eq!(retry_delay(&in_secs(600), 0, at), None);
        assert_eq!(retry_delay(&PluginError::RateLimited(None), 1, at), Some(Duration::from_secs(4)));
        assert_eq!(retry_delay(&PluginError::Status(500), 2, at), None);
        assert_eq!(retry_delay(&PluginError::Status(400), 0, at), None);
        assert_eq!(retry_delay(&PluginError::Unauthorized, 0, at), None);
    }
}

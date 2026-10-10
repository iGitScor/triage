//! The brief through any server that speaks OpenAI's Chat Completions: OpenAI, Azure OpenAI, Mistral, or one on this
//! computer (Ollama, LM Studio, vLLM). Same prompts, answers, retries and timeout as the Claude API (AI-23). A port
//! of the macOS `OpenAIPlugin.swift`.

use crate::claude::api::{retry_delay, TIMEOUT};
use crate::claude::prompt::{self, Schema};
use crate::{decode, AssistantPlugin, HttpClient, PluginConfig, PluginError, Request};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use remora_core::{
    is_loopback, Brief, ConfigField, Egress, InboxItem, PluginManifest, ServerUrl, SnoozedItem, TriageSuggestion,
    AI_SERVER_PLUGIN, DEFAULT_AI_SERVER,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;

pub const ID: &str = AI_SERVER_PLUGIN;

const TOO_SLOW: &str = "The assistant took too long to answer. Try again later.";
const DECLINED: &str = "The assistant declined this request.";
const CUT_OFF: &str = "The assistant’s answer was cut off. Try again, or with fewer items.";
const UNEXPECTED: &str = "The assistant returned an unexpected answer.";
const NEEDS_KEY: &str = "An API key is needed for this server.";
const INSECURE: &str = "Only a server on this computer can use http: use https.";

const SUMMARY: &str =
    "The brief through OpenAI, Azure OpenAI, Mistral or a server on this computer. Item titles and context are sent to that server.";
const SERVER_HELP: &str =
    "The API's address, up to /v1. A server on this computer can use http, e.g. http://localhost:11434/v1.";
const KEY_HELP: &str = "From your provider. Not needed for a server on this computer.";
const EGRESS: &str = "Sends the titles, contexts, authors and statuses of inbox items to the server you set.";

/// The same words as the macOS app, which the shared French dictionary translates.
pub fn manifest() -> PluginManifest {
    PluginManifest {
        id: ID.into(),
        name: "OpenAI-compatible".into(),
        summary: SUMMARY.into(),
        fields: vec![
            ConfigField {
                key: "host".into(),
                label: "Server".into(),
                placeholder: DEFAULT_AI_SERVER.into(),
                default_value: DEFAULT_AI_SERVER.into(),
                is_secret: false,
                is_optional: false,
                help: Some(SERVER_HELP.into()),
            },
            ConfigField { is_optional: true, ..ConfigField::token("API key", KEY_HELP) },
            ConfigField {
                key: "model".into(),
                label: "Model for the brief".into(),
                placeholder: "Model id from your provider".into(),
                default_value: String::new(),
                is_secret: false,
                is_optional: false,
                help: None,
            },
            ConfigField {
                key: "digestModel".into(),
                label: "Model for bundle summaries".into(),
                placeholder: "Same as the brief".into(),
                default_value: String::new(),
                is_secret: false,
                is_optional: true,
                help: None,
            },
        ],
        setup_steps: vec![
            "Create an API key with your provider, or start the server on this computer.".into(),
            "Paste the server address, the key and the model id below.".into(),
        ],
        setup_label: String::new(),
        setup_url: None,
        egress: Egress {
            // The account's server is added to these (`registry::allowed_hosts`).
            hosts: vec![],
            description: EGRESS.into(),
            external_ai: true,
        },
        // No OpenAI logo is bundled: the name says it.
        logo: None,
    }
}

pub struct OpenAIPlugin {
    /// `{server}/chat/completions`.
    endpoint: String,
    /// Azure's `api-key`, or a bearer token; none for a local server without one.
    auth: Option<(&'static str, String)>,
    model: String,
    digest_model: String,
    language: String,
    http: Arc<dyn HttpClient>,
}

#[derive(Serialize)]
pub struct ApiRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub response_format: ResponseFormat,
}

#[derive(Serialize)]
pub struct Message {
    pub role: &'static str,
    pub content: String,
}

#[derive(Serialize)]
pub struct ResponseFormat {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub json_schema: JsonSchema,
}

#[derive(Serialize)]
pub struct JsonSchema {
    pub name: &'static str,
    pub strict: bool,
    pub schema: Schema,
}

impl ApiRequest {
    fn new(model: &str, name: &'static str, system: String, message: String, schema: Schema) -> Self {
        ApiRequest {
            model: model.into(),
            messages: vec![Message { role: "system", content: system }, Message { role: "user", content: message }],
            response_format: ResponseFormat {
                kind: "json_schema",
                json_schema: JsonSchema { name, strict: true, schema },
            },
        }
    }
}

#[derive(Deserialize)]
pub struct ApiResponse {
    #[serde(default)]
    pub choices: Vec<Choice>,
}

#[derive(Deserialize)]
pub struct Choice {
    pub message: Option<ChoiceMessage>,
    pub finish_reason: Option<String>,
}

#[derive(Deserialize)]
pub struct ChoiceMessage {
    pub content: Option<String>,
    pub refusal: Option<String>,
}

impl OpenAIPlugin {
    pub fn new(config: &PluginConfig, http: Arc<dyn HttpClient>, language: &str) -> Result<Self, PluginError> {
        let mut server = Some(config.get("host")).filter(|h| !h.is_empty()).unwrap_or_else(|| DEFAULT_AI_SERVER.into());
        while server.ends_with('/') {
            server.pop();
        }
        let url = ServerUrl::parse(&server).ok_or_else(|| PluginError::InvalidField("host".into()))?;
        let local = is_loopback(&url.host);
        if url.scheme == "http" && !local {
            return Err(PluginError::Api(INSECURE.into()));
        }
        let key = config.get("token");
        if key.is_empty() && !local {
            return Err(PluginError::Api(NEEDS_KEY.into()));
        }
        let azure = url.host.ends_with(".openai.azure.com") || url.host.ends_with(".services.ai.azure.com");
        let auth =
            (!key.is_empty())
                .then(|| if azure { ("api-key", key) } else { ("Authorization", format!("Bearer {key}")) });
        let model = config.required("model")?;
        let digest_model = Some(config.get("digestModel")).filter(|m| !m.is_empty()).unwrap_or_else(|| model.clone());
        Ok(OpenAIPlugin {
            endpoint: format!("{server}/chat/completions"),
            auth,
            model,
            digest_model,
            language: language.into(),
            http,
        })
    }

    async fn send(&self, body: &ApiRequest) -> Result<ApiResponse, PluginError> {
        let mut request = Request::post_json(&self.endpoint, body)?.header("User-Agent", "Remora").timeout(TIMEOUT);
        if let Some((name, value)) = &self.auth {
            request = request.header(name, value.clone());
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

    /// The brief's request.
    pub fn body(items: &[InboxItem], model: &str, now: DateTime<Utc>, language: &str) -> ApiRequest {
        ApiRequest::new(model, "brief", prompt::system(language), prompt::message(items, now), Schema::brief())
    }
}

fn output<T: DeserializeOwned>(response: ApiResponse) -> Result<T, PluginError> {
    let Some(choice) = response.choices.into_iter().next() else {
        return Err(PluginError::Api(UNEXPECTED.into()));
    };
    if choice.message.as_ref().and_then(|m| m.refusal.as_deref()).is_some_and(|r| !r.trim().is_empty()) {
        return Err(PluginError::Api(DECLINED.into()));
    }
    if choice.finish_reason.as_deref() == Some("length") {
        return Err(PluginError::Api(CUT_OFF.into()));
    }
    let text = choice.message.and_then(|m| m.content);
    text.and_then(|t| serde_json::from_str(&t).ok()).ok_or_else(|| PluginError::Api(UNEXPECTED.into()))
}

#[async_trait]
impl AssistantPlugin for OpenAIPlugin {
    async fn brief(&self, items: &[InboxItem], now: DateTime<Utc>) -> Result<Brief, PluginError> {
        let response = self.send(&Self::body(items, &self.model, now, &self.language)).await?;
        let known: HashSet<String> = prompt::ids(items);
        output::<prompt::Output>(response).map(|o| o.brief(&known))
    }

    async fn digest(&self, items: &[InboxItem], topic: &str, now: DateTime<Utc>) -> Result<String, PluginError> {
        let body = ApiRequest::new(
            &self.digest_model,
            "digest",
            prompt::digest_system(&self.language),
            prompt::digest_message(items, topic, now),
            Schema::digest(),
        );
        output::<prompt::DigestOutput>(self.send(&body).await?).map(|o| o.summary)
    }

    async fn triage(&self, items: &[SnoozedItem], now: DateTime<Utc>) -> Result<Vec<TriageSuggestion>, PluginError> {
        let body = ApiRequest::new(
            &self.model,
            "triage",
            prompt::triage_system(&self.language),
            prompt::triage_message(items, now),
            Schema::triage(),
        );
        let output: prompt::TriageOutput = output(self.send(&body).await?)?;
        Ok(output.suggestions(&prompt::snoozed_ids(items), now))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::claude::testing::{item, now, snoozed};
    use crate::stub::config;
    use crate::Response;
    use std::sync::Mutex;
    use std::time::Duration;

    /// Answers with each status in turn, then 200 with `body`, and keeps the requests.
    struct Recorder {
        statuses: Mutex<Vec<u16>>,
        body: String,
        requests: Mutex<Vec<Request>>,
    }

    impl Recorder {
        fn new(statuses: &[u16], body: &str) -> Arc<Self> {
            Arc::new(Recorder {
                statuses: Mutex::new(statuses.to_vec()),
                body: body.into(),
                requests: Mutex::new(vec![]),
            })
        }

        fn requests(&self) -> Vec<Request> {
            self.requests.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl HttpClient for Recorder {
        async fn send(&self, request: Request) -> Result<Response, PluginError> {
            self.requests.lock().unwrap().push(request);
            let mut statuses = self.statuses.lock().unwrap();
            let status = if statuses.is_empty() { 200 } else { statuses.remove(0) };
            Ok(Response { status, body: self.body.clone() })
        }
    }

    fn answer(content: &str, finish: &str) -> String {
        serde_json::json!({ "choices": [{ "message": { "role": "assistant", "content": content, "refusal": null }, "finish_reason": finish }] })
            .to_string()
    }

    const BRIEF: &str =
        r#"{"summary":"One review waits.","focus":[{"id":"a","reason":"Blocks Frank"},{"id":"ghost","reason":"?"}]}"#;

    fn plugin(values: &[(&str, &str)], http: Arc<Recorder>) -> Result<OpenAIPlugin, PluginError> {
        OpenAIPlugin::new(&config(values), http, "en")
    }

    fn header(request: &Request, name: &str) -> Option<String> {
        request.headers.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, v)| v.clone())
    }

    #[tokio::test]
    async fn asks_chat_completions_with_a_strict_schema() {
        let http = Recorder::new(&[], &answer(BRIEF, "stop"));
        let plugin =
            plugin(&[("token", "sk"), ("model", "gpt-x"), ("host", "https://api.openai.com/v1/")], http.clone());
        let brief = plugin.unwrap().brief(&[item("a")], now()).await.unwrap();
        assert_eq!(brief.summary, "One review waits.");
        assert_eq!(brief.focus.iter().map(|f| f.id.as_str()).collect::<Vec<_>>(), ["a"], "made-up items dropped");

        let request = &http.requests()[0];
        assert_eq!(request.url, "https://api.openai.com/v1/chat/completions");
        assert_eq!(header(request, "Authorization").as_deref(), Some("Bearer sk"));
        assert_eq!(header(request, "api-key"), None);
        assert_eq!(request.timeout, Some(TIMEOUT));
        let body: serde_json::Value = serde_json::from_str(request.body.as_deref().unwrap()).unwrap();
        assert_eq!(body["model"], "gpt-x");
        assert_eq!(body["messages"][0]["role"], "system");
        assert_eq!(body["messages"][1]["role"], "user");
        assert!(body["messages"][1]["content"].as_str().unwrap().contains("Fix login"));
        assert_eq!(body["response_format"]["type"], "json_schema");
        assert_eq!(body["response_format"]["json_schema"]["name"], "brief");
        assert_eq!(body["response_format"]["json_schema"]["strict"], true);
        assert_eq!(body["response_format"]["json_schema"]["schema"]["additionalProperties"], false);
        assert!(body.get("max_tokens").is_none() && body.get("max_completion_tokens").is_none());
    }

    #[tokio::test]
    async fn azure_gets_its_key_header_and_summaries_their_model() {
        let http = Recorder::new(&[], &answer(r#"{"summary":"Two PRs."}"#, "stop"));
        let values = [("token", "az"), ("model", "big"), ("host", "https://acme.openai.azure.com/openai/v1")];
        let summary = plugin(&values, http.clone()).unwrap().digest(&[item("a")], "acme/app", now()).await.unwrap();
        assert_eq!(summary, "Two PRs.");
        let request = &http.requests()[0];
        assert_eq!(header(request, "api-key").as_deref(), Some("az"));
        assert_eq!(header(request, "Authorization"), None);
        let body: serde_json::Value = serde_json::from_str(request.body.as_deref().unwrap()).unwrap();
        assert_eq!(body["model"], "big", "no summary model: the brief's");
        assert_eq!(body["response_format"]["json_schema"]["name"], "digest");

        let http = Recorder::new(&[], &answer(r#"{"summary":"x"}"#, "stop"));
        let values = [("token", "k"), ("model", "big"), ("digestModel", "small")];
        plugin(&values, http.clone()).unwrap().digest(&[item("a")], "t", now()).await.unwrap();
        assert!(http.requests()[0].body.as_deref().unwrap().contains(r#""model":"small""#));
    }

    #[tokio::test]
    async fn triage_keeps_known_items() {
        let suggestions = r#"{"suggestions":[{"id":"s","action":"done","until":"","reason":"Merged"},{"id":"ghost","action":"done","until":"","reason":"?"}]}"#;
        let http = Recorder::new(&[], &answer(suggestions, "stop"));
        let result = plugin(&[("token", "k"), ("model", "m")], http.clone())
            .unwrap()
            .triage(&[snoozed(item("s"), 1)], now())
            .await;
        assert_eq!(result.unwrap().iter().map(|s| s.id.as_str()).collect::<Vec<_>>(), ["s"]);
        assert!(http.requests()[0].body.as_deref().unwrap().contains(r#""name":"triage""#));
    }

    #[test]
    fn a_server_on_this_computer_needs_no_key_and_may_use_http() {
        let http = Recorder::new(&[], "");
        let local = plugin(&[("host", "http://localhost:11434/v1"), ("model", "llama")], http.clone()).unwrap();
        assert_eq!(local.endpoint, "http://localhost:11434/v1/chat/completions");
        assert!(local.auth.is_none());
        assert!(plugin(&[("host", "http://127.0.0.1:1234/v1"), ("model", "m")], http.clone()).is_ok());
        assert_eq!(
            plugin(&[("host", "http://llm.lan/v1"), ("token", "k"), ("model", "m")], http.clone()).err(),
            Some(PluginError::Api(INSECURE.into()))
        );
        assert_eq!(
            plugin(&[("model", "m")], http.clone()).err(),
            Some(PluginError::Api(NEEDS_KEY.into())),
            "OpenAI by default"
        );
        assert_eq!(plugin(&[("token", "k")], http.clone()).err(), Some(PluginError::MissingField("model".into())));
        assert_eq!(
            plugin(&[("host", "api.openai.com/v1"), ("token", "k"), ("model", "m")], http).err(),
            Some(PluginError::InvalidField("host".into()))
        );
    }

    #[tokio::test]
    async fn refusals_cut_offs_and_strange_answers_say_so() {
        let cases = [
            (
                serde_json::json!({ "choices": [{ "message": { "content": null, "refusal": "No." }, "finish_reason": "stop" }] })
                    .to_string(),
                DECLINED,
            ),
            (answer(r#"{"summary":"One rev"#, "length"), CUT_OFF),
            (answer("not json", "stop"), UNEXPECTED),
            (r#"{"choices":[]}"#.to_string(), UNEXPECTED),
        ];
        for (body, expected) in cases {
            let http = Recorder::new(&[], &body);
            let result = plugin(&[("token", "k"), ("model", "m")], http).unwrap().brief(&[item("a")], now()).await;
            assert_eq!(result.unwrap_err(), PluginError::Api(expected.into()));
        }
    }

    #[tokio::test(start_paused = true)]
    async fn overloaded_answers_are_retried_twice() {
        let http = Recorder::new(&[529, 503], &answer(BRIEF, "stop"));
        let started = tokio::time::Instant::now();
        let brief = plugin(&[("token", "k"), ("model", "m")], http.clone()).unwrap().brief(&[item("a")], now()).await;
        assert!(brief.is_ok());
        assert_eq!(http.requests().len(), 3);
        assert_eq!(started.elapsed(), Duration::from_secs(6), "2 s, then 4 s");

        let down = Recorder::new(&[503; 4], "");
        let error = plugin(&[("token", "k"), ("model", "m")], down.clone()).unwrap().brief(&[item("a")], now()).await;
        assert_eq!(error.unwrap_err(), PluginError::Status(503));
        assert_eq!(down.requests().len(), 3);
    }

    /// A non-streamed answer is silent until complete: the Claude API's long timeout, and its own message.
    #[tokio::test]
    async fn a_timeout_says_the_assistant_was_too_slow() {
        struct Slow;
        #[async_trait]
        impl HttpClient for Slow {
            async fn send(&self, _request: Request) -> Result<crate::Response, PluginError> {
                Err(PluginError::TimedOut)
            }
        }
        let plugin = OpenAIPlugin::new(&config(&[("token", "k"), ("model", "m")]), Arc::new(Slow), "en").unwrap();
        assert_eq!(plugin.brief(&[item("a")], now()).await.unwrap_err(), PluginError::Api(TOO_SLOW.into()));
    }
}

use async_trait::async_trait;
use remora_core::host_matches;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
}

#[derive(Clone, Debug)]
pub struct Request {
    pub method: Method,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
}

impl Request {
    pub fn get(url: impl Into<String>) -> Self {
        Request { method: Method::Get, url: url.into(), headers: vec![], body: None }
    }

    pub fn post_json(url: impl Into<String>, body: &impl Serialize) -> Result<Self, PluginError> {
        let body = serde_json::to_string(body).map_err(|e| PluginError::Decode(e.to_string()))?;
        Ok(Request { method: Method::Post, url: url.into(), headers: vec![("Content-Type".into(), "application/json".into())], body: Some(body) })
    }

    pub fn header(mut self, name: &str, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    pub fn host(&self) -> String {
        reqwest::Url::parse(&self.url).ok().and_then(|u| u.host_str().map(str::to_lowercase)).unwrap_or_default()
    }
}

/// `base` + `path` + encoded query parameters.
pub fn url_with_query(base: &str, path: &str, query: &[(&str, &str)]) -> String {
    let mut url = reqwest::Url::parse(&format!("{}/{}", base.trim_end_matches('/'), path.trim_start_matches('/'))).expect("valid base URL");
    if !query.is_empty() {
        url.query_pairs_mut().extend_pairs(query);
    }
    url.to_string()
}

pub struct Response {
    pub status: u16,
    pub body: String,
}

/// Errors are English sentences, translated by the UI.
#[derive(Clone, Debug, PartialEq)]
pub enum PluginError {
    Unauthorized,
    Status(u16, String),
    Api(String),
    BlockedHost(String),
    Network(String),
    Decode(String),
    MissingField(String),
    InvalidField(String),
    UnknownPlugin(String),
}

impl fmt::Display for PluginError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PluginError::Unauthorized => write!(f, "The token was rejected. Check it and its scopes."),
            PluginError::Status(code, body) if body.is_empty() => write!(f, "HTTP {code}"),
            PluginError::Status(code, body) => write!(f, "HTTP {code}: {body}"),
            PluginError::Api(message) => write!(f, "{message}"),
            PluginError::BlockedHost(host) => write!(f, "Blocked: {host} is not an allowed destination."),
            PluginError::Network(message) => write!(f, "Network error: {message}"),
            PluginError::Decode(message) => write!(f, "Unexpected answer from the server: {message}"),
            PluginError::MissingField(key) => write!(f, "“{key}” is required."),
            PluginError::InvalidField(key) => write!(f, "“{key}” is not valid."),
            PluginError::UnknownPlugin(id) => write!(f, "Unknown plugin “{id}”."),
        }
    }
}

impl std::error::Error for PluginError {}

#[async_trait]
pub trait HttpClient: Send + Sync {
    async fn send(&self, request: Request) -> Result<Response, PluginError>;
}

/// The real network, through the operating system's TLS (it trusts the company's root certificates).
pub struct ReqwestClient {
    client: reqwest::Client,
}

impl ReqwestClient {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent("Remora")
            .timeout(Duration::from_secs(30))
            .build()
            .expect("HTTP client");
        ReqwestClient { client }
    }
}

impl Default for ReqwestClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl HttpClient for ReqwestClient {
    async fn send(&self, request: Request) -> Result<Response, PluginError> {
        let mut builder = match request.method {
            Method::Get => self.client.get(&request.url),
            Method::Post => self.client.post(&request.url),
        };
        for (name, value) in &request.headers {
            builder = builder.header(name, value);
        }
        if let Some(body) = request.body {
            builder = builder.body(body);
        }
        let response = builder.send().await.map_err(|e| PluginError::Network(e.to_string()))?;
        let status = response.status().as_u16();
        let body = response.text().await.map_err(|e| PluginError::Network(e.to_string()))?;
        Ok(Response { status, body })
    }
}

/// Lets a plugin reach only the hosts it declared. Every plugin request goes through one of these.
pub struct GuardedHttpClient {
    base: Arc<dyn HttpClient>,
    hosts: Vec<String>,
}

impl GuardedHttpClient {
    pub fn new(base: Arc<dyn HttpClient>, hosts: Vec<String>) -> Self {
        GuardedHttpClient { base, hosts }
    }
}

#[async_trait]
impl HttpClient for GuardedHttpClient {
    async fn send(&self, request: Request) -> Result<Response, PluginError> {
        let host = request.host();
        if !host_matches(&host, &self.hosts) {
            return Err(PluginError::BlockedHost(if host.is_empty() { "?".into() } else { host }));
        }
        self.base.send(request).await
    }
}

/// Sends and decodes JSON, mapping HTTP failures to `PluginError`.
pub async fn decode<T: DeserializeOwned>(http: &dyn HttpClient, request: Request) -> Result<T, PluginError> {
    let response = http.send(request).await?;
    match response.status {
        200..=299 => serde_json::from_str(&response.body).map_err(|e| PluginError::Decode(e.to_string())),
        401 | 403 => Err(PluginError::Unauthorized),
        code => Err(PluginError::Status(code, response.body.chars().take(200).collect())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stub::StubHttp;

    #[tokio::test]
    async fn guarded_client_only_reaches_declared_hosts() {
        let client = GuardedHttpClient::new(Arc::new(StubHttp::paths(&[("/x", "{}")])), vec!["api.linear.app".into(), "gitlab.acme.io".into()]);
        assert!(client.send(Request::get("https://api.linear.app/x")).await.is_ok());
        assert!(client.send(Request::get("https://eu.gitlab.acme.io/x")).await.is_ok());
        assert_eq!(client.send(Request::get("https://evil.example.com/x")).await.err(), Some(PluginError::BlockedHost("evil.example.com".into())));
        assert_eq!(client.send(Request::get("https://api.linear.app.evil.com/x")).await.err(), Some(PluginError::BlockedHost("api.linear.app.evil.com".into())));
    }
}

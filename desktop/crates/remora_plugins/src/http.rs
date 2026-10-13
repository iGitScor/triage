use async_trait::async_trait;
use remora_core::host_matches;
use remora_core::Message;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::{HashMap, VecDeque};
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
    /// Replaces the client's 30 s, for answers that are silent until complete (Claude's).
    pub timeout: Option<Duration>,
}

impl Request {
    pub fn get(url: impl Into<String>) -> Self {
        Request { method: Method::Get, url: url.into(), headers: vec![], body: None, timeout: None }
    }

    pub fn post_json(url: impl Into<String>, body: &impl Serialize) -> Result<Self, PluginError> {
        let body = serde_json::to_string(body).map_err(|e| PluginError::Decode(e.to_string()))?;
        Ok(Request {
            method: Method::Post,
            url: url.into(),
            headers: vec![("Content-Type".into(), "application/json".into())],
            body: Some(body),
            timeout: None,
        })
    }

    /// A form, as OAuth's token endpoints take it.
    pub fn post_form(url: impl Into<String>, fields: &[(&str, &str)]) -> Self {
        let body = reqwest::Url::parse_with_params("http://form", fields)
            .ok()
            .and_then(|url| url.query().map(str::to_string))
            .unwrap_or_default();
        Request {
            method: Method::Post,
            url: url.into(),
            headers: vec![("Content-Type".into(), "application/x-www-form-urlencoded".into())],
            body: Some(body),
            timeout: None,
        }
    }

    pub fn header(mut self, name: &str, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn host(&self) -> String {
        reqwest::Url::parse(&self.url).ok().and_then(|u| u.host_str().map(str::to_lowercase)).unwrap_or_default()
    }
}

/// `base` + `path` + encoded query parameters. A base that isn't a URL is the setting's fault, not a crash (DEV-08).
pub fn url_with_query(base: &str, path: &str, query: &[(&str, &str)]) -> Result<String, PluginError> {
    let mut url = reqwest::Url::parse(&format!("{}/{}", base.trim_end_matches('/'), path.trim_start_matches('/')))
        .map_err(|_| PluginError::InvalidField(base.into()))?;
    if !query.is_empty() {
        url.query_pairs_mut().extend_pairs(query);
    }
    Ok(url.to_string())
}

pub struct Response {
    pub status: u16,
    pub body: String,
    /// The answer's `ETag`, for `ConditionalHttpClient`.
    pub etag: Option<String>,
    /// When the server's rate limit resets, in seconds since 1970: GitHub's GraphQL reports its limit in a 200.
    pub reset: Option<i64>,
}

/// Errors are English sentences, translated by the UI.
#[derive(Clone, Debug, PartialEq)]
pub enum PluginError {
    Unauthorized,
    /// Any other failure. Only the code: the body can echo the request and isn't meant for people.
    Status(u16),
    Api(String),
    BlockedHost(String),
    BlockedRedirect(String),
    /// 429, or a 403 with no requests left. When to try again (seconds since 1970), if the server said.
    RateLimited(Option<i64>),
    Network(String),
    /// No answer in time.
    TimedOut,
    /// The answer was larger than Remora reads: a broken or hostile server.
    TooLarge(usize),
    Decode(String),
    MissingField(String),
    InvalidField(String),
    InsecureField(String),
    UnknownPlugin(String),
    /// The browser sign-in didn't finish: what the server answered (`access_denied`), or why.
    SignIn(String),
}

impl PluginError {
    /// What kind of failure this is for the inbox. A network error is a host that can't be reached; the inbox
    /// turns it into "offline" when every source failed that way.
    pub fn kind(&self) -> remora_core::FailureKind {
        use remora_core::FailureKind;
        match self {
            PluginError::Unauthorized => FailureKind::Auth,
            PluginError::RateLimited(_) => FailureKind::RateLimited,
            PluginError::Network(_) | PluginError::TimedOut => FailureKind::Unreachable,
            _ => FailureKind::Other,
        }
    }
}

impl fmt::Display for PluginError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PluginError::Unauthorized => write!(f, "The token was rejected. Check it and its scopes."),
            PluginError::Status(code) => {
                let text = match code {
                    404 => {
                        "The server couldn’t find what Remora asked for: check the address and what the token can see."
                    }
                    500.. => "The server had a problem: Remora tries again at the next refresh.",
                    _ => "The server refused the request.",
                };
                write!(f, "{text} (HTTP {code})")
            }
            PluginError::Api(message) => write!(f, "{message}"),
            PluginError::BlockedHost(host) => write!(f, "Blocked: {host} is not an allowed destination."),
            PluginError::BlockedRedirect(host) => write!(f, "Blocked: the server redirected to {host}."),
            PluginError::RateLimited(_) => write!(f, "Too many requests: Remora waits a little before trying again."),
            PluginError::Network(message) => write!(f, "Network error: {message}"),
            // Same words as any network failure: the interface already translates them.
            PluginError::TimedOut => PluginError::Network("timed out".into()).fmt(f),
            PluginError::TooLarge(limit) => {
                write!(f, "The server’s answer is too large: Remora reads at most {} MB.", limit / 1_000_000)
            }
            PluginError::Decode(message) => write!(f, "Unexpected answer from the server: {message}"),
            PluginError::MissingField(key) => write!(f, "“{key}” is required."),
            PluginError::InvalidField(key) => write!(f, "“{key}” is not valid."),
            PluginError::InsecureField(key) => {
                write!(f, "“{key}” must start with https://: the token would travel unencrypted.")
            }
            PluginError::UnknownPlugin(id) => write!(f, "Unknown plugin “{id}”."),
            PluginError::SignIn(reason) => write!(f, "The sign-in didn’t finish: {reason}."),
        }
    }
}

impl std::error::Error for PluginError {}

impl PluginError {
    /// For the interface: a stable code and its values (`MESSAGES` in i18n.ts), with this English sentence. What a
    /// server or the system wrote stays as it is.
    pub fn message(&self) -> Message {
        let text = self.to_string();
        let with = |code: &str, value: &str| Message::new(code, vec![value.to_string()], text.clone());
        match self {
            PluginError::Unauthorized => Message::new("unauthorized", vec![], text),
            PluginError::Status(404) => with("http.notFound", "404"),
            PluginError::Status(code @ 500..) => with("http.serverError", &code.to_string()),
            PluginError::Status(code) => with("http.refused", &code.to_string()),
            PluginError::Api(_) => text.into(),
            PluginError::BlockedHost(host) => with("blockedHost", host),
            PluginError::BlockedRedirect(host) => with("blockedRedirect", host),
            PluginError::RateLimited(_) => Message::new("rateLimited", vec![], text),
            PluginError::Network(message) => with("network", message),
            PluginError::TimedOut => with("network", "timed out"),
            PluginError::TooLarge(limit) => with("tooLarge", &(limit / 1_000_000).to_string()),
            PluginError::Decode(message) => with("decode", message),
            PluginError::MissingField(key) => with("missingField", key),
            PluginError::InvalidField(key) => with("invalidField", key),
            PluginError::InsecureField(key) => with("insecureField", key),
            PluginError::UnknownPlugin(id) => with("unknownPlugin", id),
            PluginError::SignIn(reason) => with("signIn", reason),
        }
    }
}

impl From<PluginError> for Message {
    fn from(error: PluginError) -> Self {
        error.message()
    }
}

#[async_trait]
pub trait HttpClient: Send + Sync {
    async fn send(&self, request: Request) -> Result<Response, PluginError>;
}

/// The real network, through the operating system's TLS (it trusts the company's root certificates).
pub struct ReqwestClient {
    /// Why there is none, when the system's TLS couldn't be set up: every request then fails with it, and the app
    /// still opens (DEV-08).
    client: Result<reqwest::Client, String>,
}

impl ReqwestClient {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent("Remora")
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                let from = attempt.previous().last().cloned();
                if attempt.previous().len() < 10 && redirect_allowed(from.as_ref(), attempt.url()) {
                    attempt.follow()
                } else {
                    attempt.stop()
                }
            }))
            .build()
            .map_err(|e| e.to_string());
        ReqwestClient { client }
    }

    fn client(&self) -> Result<&reqwest::Client, PluginError> {
        self.client.as_ref().map_err(|e| PluginError::Network(e.clone()))
    }
}

impl ReqwestClient {
    /// An avatar for the window: an image, at most `MAX_IMAGE`, sent with no header but the user agent.
    /// The caller checks the URL is allowed first.
    pub async fn get_image(&self, url: &str) -> Result<(String, Vec<u8>), PluginError> {
        let response = self.client()?.get(url).send().await.map_err(|e| PluginError::Network(e.to_string()))?;
        if !response.status().is_success() {
            return Err(PluginError::Status(response.status().as_u16()));
        }
        let kind = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_string();
        if !kind.starts_with("image/") {
            return Err(PluginError::Decode(format!("not an image ({kind})")));
        }
        Ok((kind, read_capped(response, MAX_IMAGE).await?))
    }
}

/// Avatars are a few kilobytes: anything near this is not one.
pub const MAX_IMAGE: usize = 1_000_000;

impl Default for ReqwestClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl HttpClient for ReqwestClient {
    async fn send(&self, request: Request) -> Result<Response, PluginError> {
        let client = self.client()?;
        let mut builder = match request.method {
            Method::Get => client.get(&request.url),
            Method::Post => client.post(&request.url),
        };
        for (name, value) in &request.headers {
            builder = builder.header(name, value);
        }
        if let Some(body) = request.body {
            builder = builder.body(body);
        }
        if let Some(timeout) = request.timeout {
            builder = builder.timeout(timeout);
        }
        let response = builder.send().await.map_err(|e| {
            if e.is_timeout() {
                PluginError::TimedOut
            } else {
                PluginError::Network(e.to_string())
            }
        })?;
        let status = response.status().as_u16();
        let header = |name: &str| response.headers().get(name).and_then(|v| v.to_str().ok()).map(str::to_string);
        if let Some(resume) = rate_limit(status, header, chrono::Utc::now()) {
            return Err(PluginError::RateLimited(resume));
        }
        let etag = header("etag");
        let reset = resume_time(header, chrono::Utc::now());
        // 304 is a 3xx too, but the answer to a conditional request (`ConditionalHttpClient`), not a redirect.
        if response.status().is_redirection() && status != 304 {
            // Only refused redirects get here (see `redirect_allowed`).
            let location =
                response.headers().get(reqwest::header::LOCATION).and_then(|v| v.to_str().ok()).unwrap_or_default();
            let target = response.url().join(location).ok().and_then(|u| u.host_str().map(str::to_string));
            return Err(PluginError::BlockedRedirect(target.unwrap_or_else(|| "?".into())));
        }
        let body = read_capped(response, MAX_BODY).await?;
        Ok(Response { status, body: String::from_utf8_lossy(&body).into_owned(), etag, reset })
    }
}

/// The most of an answer Remora reads: far above any real API page, low enough that a broken or hostile
/// server can't exhaust memory.
pub const MAX_BODY: usize = 5_000_000;

/// Reads the body in chunks and stops past `limit`, whatever the server announced.
pub async fn read_capped(mut response: reqwest::Response, limit: usize) -> Result<Vec<u8>, PluginError> {
    if response.content_length().is_some_and(|length| length as usize > limit) {
        return Err(PluginError::TooLarge(limit));
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| PluginError::Network(e.to_string()))? {
        if body.len() + chunk.len() > limit {
            return Err(PluginError::TooLarge(limit));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

/// A rate limit, and when to come back: `Some(resume)` for a 429, or a 403 with nothing left
/// (`x-ratelimit-remaining` on GitHub, `RateLimit-Remaining` on GitLab) or a `Retry-After` (GitHub's secondary limit).
/// `resume` is `resume_time`.
pub fn rate_limit(
    status: u16,
    header: impl Fn(&str) -> Option<String>,
    now: chrono::DateTime<chrono::Utc>,
) -> Option<Option<i64>> {
    let exhausted = ["x-ratelimit-remaining", "ratelimit-remaining"]
        .iter()
        .any(|name| header(name).is_some_and(|v| v.trim() == "0"));
    if status != 429 && !(status == 403 && (exhausted || header("retry-after").is_some())) {
        return None;
    }
    Some(resume_time(header, now))
}

/// When a rate-limited server says to come back: `Retry-After` (seconds or a date), else the reset time GitHub and
/// GitLab give in seconds since 1970.
pub fn resume_time(header: impl Fn(&str) -> Option<String>, now: chrono::DateTime<chrono::Utc>) -> Option<i64> {
    let retry_after = header("retry-after").and_then(|value| {
        let value = value.trim().to_string();
        value
            .parse::<i64>()
            .ok()
            .map(|seconds| now.timestamp() + seconds)
            .or_else(|| chrono::DateTime::parse_from_rfc2822(&value).ok().map(|d| d.timestamp()))
    });
    let reset = || {
        ["x-ratelimit-reset", "ratelimit-reset"]
            .iter()
            .find_map(|name| header(name).and_then(|v| v.trim().parse::<i64>().ok()))
    };
    retry_after.or_else(reset)
}

/// Redirects stay on the same host, over https: the egress guard only sees the first URL, and headers such as
/// GitLab's `PRIVATE-TOKEN` would follow a redirect to another host.
pub fn redirect_allowed(from: Option<&reqwest::Url>, to: &reqwest::Url) -> bool {
    let Some(from) = from else { return false };
    let same_host =
        from.host_str().is_some() && from.host_str().map(str::to_lowercase) == to.host_str().map(str::to_lowercase);
    same_host && (to.scheme() == "https" || (to.scheme() == "http" && from.scheme() == "http"))
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

/// Conditional requests (PERF-09): a GET whose last answer carried an `ETag` is sent with `If-None-Match`, and a 304
/// is answered with the copy kept from that last time, as a 200. Unchanged lists then cost the server no body. Only
/// GETs are conditional: GitLab's REST lists and Notion's reads gain, but GitHub and Linear are read with one GraphQL
/// POST each, which this doesn't cover (PERF-19). Sits behind the egress guard, so only allowed hosts are kept.
pub struct ConditionalHttpClient {
    base: Arc<dyn HttpClient>,
    store: Arc<ETagStore>,
}

impl ConditionalHttpClient {
    pub fn new(base: Arc<dyn HttpClient>, store: Arc<ETagStore>) -> Self {
        ConditionalHttpClient { base, store }
    }
}

#[async_trait]
impl HttpClient for ConditionalHttpClient {
    async fn send(&self, mut request: Request) -> Result<Response, PluginError> {
        if request.method != Method::Get {
            return self.base.send(request).await;
        }
        let url = request.url.clone();
        let kept = self.store.answer(&url);
        if let Some((etag, _)) = &kept {
            request = request.header("If-None-Match", etag.clone());
        }
        let response = self.base.send(request).await?;
        match (response.status, kept) {
            (304, Some((etag, body))) => Ok(Response { status: 200, body, etag: Some(etag), reset: response.reset }),
            (200..=299, _) if response.etag.is_some() => {
                self.store.keep(&url, response.etag.clone().unwrap_or_default(), response.body.clone());
                Ok(response)
            }
            (_, kept) => {
                if kept.is_some() {
                    self.store.forget(&url);
                }
                Ok(response)
            }
        }
    }
}

/// The last answer to each GET of one account that came with an `ETag`. In memory only: it holds message content,
/// which must not outlive "Erase local data". Bounded, the oldest answers going first.
pub struct ETagStore {
    limit: usize,
    answers: std::sync::Mutex<(HashMap<String, Answer>, VecDeque<String>)>,
}

/// An `ETag` and the body it names.
type Answer = (String, String);

impl Default for ETagStore {
    fn default() -> Self {
        ETagStore::with_limit(256)
    }
}

impl ETagStore {
    pub fn with_limit(limit: usize) -> Self {
        ETagStore { limit, answers: Default::default() }
    }

    /// The `ETag` and body kept for `url`.
    fn answer(&self, url: &str) -> Option<Answer> {
        self.answers.lock().unwrap().0.get(url).cloned()
    }

    fn keep(&self, url: &str, etag: String, body: String) {
        let (answers, order) = &mut *self.answers.lock().unwrap();
        if answers.insert(url.to_string(), (etag, body)).is_none() {
            order.push_back(url.to_string());
        }
        while order.len() > self.limit {
            if let Some(oldest) = order.pop_front() {
                answers.remove(&oldest);
            }
        }
    }

    fn forget(&self, url: &str) {
        let (answers, order) = &mut *self.answers.lock().unwrap();
        answers.remove(url);
        order.retain(|kept| kept != url);
    }
}

/// Sends and decodes JSON, mapping HTTP failures to `PluginError`.
pub async fn decode<T: DeserializeOwned>(http: &dyn HttpClient, request: Request) -> Result<T, PluginError> {
    decode_with_reset(http, request).await.map(|(value, _)| value)
}

/// `decode`, with when the rate limit resets, for an API that reports failures in a 200 (GitHub's GraphQL limit).
pub async fn decode_with_reset<T: DeserializeOwned>(
    http: &dyn HttpClient,
    request: Request,
) -> Result<(T, Option<i64>), PluginError> {
    let response = http.send(request).await?;
    match response.status {
        200..=299 => serde_json::from_str(&response.body)
            .map(|value| (value, response.reset))
            .map_err(|e| PluginError::Decode(e.to_string())),
        401 | 403 => Err(PluginError::Unauthorized),
        429 => Err(PluginError::RateLimited(None)),
        code => Err(PluginError::Status(code)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stub::StubHttp;

    #[tokio::test]
    async fn a_bad_address_or_a_missing_tls_is_an_error_not_a_crash() {
        assert_eq!(url_with_query("not a url", "x", &[]), Err(PluginError::InvalidField("not a url".into())));
        assert_eq!(url_with_query("https://a.io/", "/v1", &[("q", "a b")]).as_deref(), Ok("https://a.io/v1?q=a+b"));
        let broken = ReqwestClient { client: Err("no TLS".into()) };
        assert_eq!(broken.send(Request::get("https://a.io")).await.err(), Some(PluginError::Network("no TLS".into())));
        assert_eq!(broken.get_image("https://a.io/a.png").await.err(), Some(PluginError::Network("no TLS".into())));
    }

    #[tokio::test]
    async fn guarded_client_only_reaches_declared_hosts() {
        let client = GuardedHttpClient::new(
            Arc::new(StubHttp::paths(&[("/x", "{}")])),
            vec!["api.linear.app".into(), "gitlab.acme.io".into()],
        );
        assert!(client.send(Request::get("https://api.linear.app/x")).await.is_ok());
        assert!(client.send(Request::get("https://eu.gitlab.acme.io/x")).await.is_ok());
        assert_eq!(
            client.send(Request::get("https://evil.example.com/x")).await.err(),
            Some(PluginError::BlockedHost("evil.example.com".into()))
        );
        assert_eq!(
            client.send(Request::get("https://api.linear.app.evil.com/x")).await.err(),
            Some(PluginError::BlockedHost("api.linear.app.evil.com".into()))
        );
    }

    /// A server whose answer carries `ETag: "v1"`, and that answers 304 when asked with it. Records the
    /// `If-None-Match` of each request.
    struct Versioned {
        etag: Option<&'static str>,
        asked: std::sync::Mutex<Vec<Option<String>>>,
    }

    #[async_trait]
    impl HttpClient for Versioned {
        async fn send(&self, request: Request) -> Result<Response, PluginError> {
            let condition =
                request.headers.iter().find(|(name, _)| name == "If-None-Match").map(|(_, value)| value.clone());
            self.asked.lock().unwrap().push(condition.clone());
            if condition.is_some() && condition.as_deref() == self.etag {
                return Ok(Response { status: 304, body: String::new(), etag: None, reset: None });
            }
            Ok(Response {
                status: 200,
                body: format!("body of {}", request.url),
                etag: self.etag.map(str::to_string),
                reset: None,
            })
        }
    }

    fn versioned(etag: Option<&'static str>) -> Arc<Versioned> {
        Arc::new(Versioned { etag, asked: Default::default() })
    }

    const LIST: &str = "https://gitlab.acme.io/api/v4/merge_requests";

    #[tokio::test]
    async fn an_unchanged_answer_is_served_from_memory() {
        let server = versioned(Some("\"v1\""));
        let client = ConditionalHttpClient::new(server.clone(), Arc::new(ETagStore::default()));
        let first = client.send(Request::get(LIST)).await.unwrap();
        let second = client.send(Request::get(LIST)).await.unwrap();
        assert_eq!(*server.asked.lock().unwrap(), vec![None, Some("\"v1\"".to_string())]);
        assert_eq!((second.status, second.body), (200, first.body));
    }

    #[tokio::test]
    async fn only_gets_with_an_etag_are_conditional() {
        let server = versioned(Some("\"v1\""));
        let client = ConditionalHttpClient::new(server.clone(), Arc::new(ETagStore::default()));
        let post = || Request::post_json(LIST, &serde_json::json!({"query": "x"})).unwrap();
        client.send(post()).await.unwrap();
        client.send(post()).await.unwrap();
        let plain = versioned(None);
        let without = ConditionalHttpClient::new(plain.clone(), Arc::new(ETagStore::default()));
        without.send(Request::get(LIST)).await.unwrap();
        without.send(Request::get(LIST)).await.unwrap();
        assert!(server.asked.lock().unwrap().iter().chain(plain.asked.lock().unwrap().iter()).all(Option::is_none));
    }

    #[test]
    fn the_store_keeps_the_latest_answers_only() {
        let store = ETagStore::with_limit(2);
        for url in ["a", "b", "c"] {
            store.keep(url, "e".into(), String::new());
        }
        assert!(store.answer("a").is_none());
        assert!(store.answer("c").is_some());
    }

    /// A 304 reaches the conditional client instead of failing as a refused redirect.
    #[tokio::test]
    async fn not_modified_is_not_a_redirect() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0u8; 1024];
            let _ = socket.read(&mut request).await;
            let _ = socket.write_all(b"HTTP/1.1 304 Not Modified\r\nETag: \"v1\"\r\nConnection: close\r\n\r\n").await;
        });
        let response = ReqwestClient::new().send(Request::get(format!("http://{address}/"))).await.unwrap();
        assert_eq!((response.status, response.etag.as_deref()), (304, Some("\"v1\"")));
    }

    /// A local server answering one request with `body`, without announcing its length (read until the connection
    /// closes), as a broken or hostile server would.
    async fn serve_once(body: Vec<u8>) -> String {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0u8; 1024];
            let _ = socket.read(&mut request).await;
            let _ = socket.write_all(b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n").await;
            let _ = socket.write_all(&body).await;
        });
        format!("http://{address}/")
    }

    /// An answer past the limit fails with a clear error instead of filling memory.
    #[tokio::test]
    async fn answers_are_read_up_to_a_limit() {
        let client = ReqwestClient::new();
        let small = serve_once(vec![b'x'; MAX_BODY - 1]).await;
        assert_eq!(client.send(Request::get(&small)).await.unwrap().body.len(), MAX_BODY - 1);
        let huge = serve_once(vec![b'x'; MAX_BODY + 1]).await;
        assert_eq!(client.send(Request::get(&huge)).await.err(), Some(PluginError::TooLarge(MAX_BODY)));
        assert_eq!(
            PluginError::TooLarge(MAX_BODY).to_string(),
            "The server’s answer is too large: Remora reads at most 5 MB."
        );
    }

    /// Every code an error or remark can carry has its English in the interface (`MESSAGES` in i18n.ts), which is
    /// where it is translated: a code with no wording there would show only the English text.
    #[test]
    fn every_message_code_has_its_wording_in_the_interface() {
        let interface = include_str!("../../../app/src/lib/i18n.ts");
        let errors = [
            PluginError::Unauthorized,
            PluginError::Status(404),
            PluginError::Status(502),
            PluginError::Status(400),
            PluginError::BlockedHost("evil.io".into()),
            PluginError::BlockedRedirect("evil.io".into()),
            PluginError::RateLimited(None),
            PluginError::Network("reset".into()),
            PluginError::TimedOut,
            PluginError::TooLarge(MAX_BODY),
            PluginError::Decode("eof".into()),
            PluginError::MissingField("token".into()),
            PluginError::InvalidField("host".into()),
            PluginError::InsecureField("host".into()),
            PluginError::UnknownPlugin("x".into()),
        ];
        let codes = errors
            .iter()
            .map(PluginError::message)
            .chain([
                crate::truncated("Jira"),
                crate::partial("Replies in your threads", &PluginError::TimedOut),
                crate::notion::database_failed("abc", &PluginError::Status(404)),
            ])
            .map(|m| m.code);
        for code in codes {
            let key = if code.contains('.') { format!("'{code}':") } else { format!("  {code}:") };
            assert!(interface.contains(&key), "{code} has no wording in i18n.ts");
        }
        // A server's own words stay as they are.
        assert!(PluginError::Api("Slack: invalid_auth".into()).message().is_text());
        assert_eq!(PluginError::Status(404).message().args, ["404"]);
    }

    /// GitHub's 403 with nothing left or a Retry-After, GitLab's reset header, Retry-After, and a refused token.
    #[test]
    fn rate_limits_are_recognised_with_their_reset_time() {
        let now: chrono::DateTime<chrono::Utc> = "2027-01-15T08:00:00Z".parse().unwrap();
        let headers = |pairs: &'static [(&'static str, &'static str)]| {
            move |name: &str| pairs.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.to_string())
        };
        assert_eq!(
            rate_limit(403, headers(&[("X-RateLimit-Remaining", "0"), ("X-RateLimit-Reset", "1800000600")]), now),
            Some(Some(1_800_000_600))
        );
        assert_eq!(rate_limit(429, headers(&[("RateLimit-Reset", "1800000900")]), now), Some(Some(1_800_000_900)));
        assert_eq!(rate_limit(429, headers(&[("Retry-After", "120")]), now), Some(Some(now.timestamp() + 120)));
        assert_eq!(
            rate_limit(429, headers(&[("Retry-After", "Fri, 15 Jan 2027 08:05:00 GMT")]), now),
            Some(Some(now.timestamp() + 300))
        );
        assert_eq!(rate_limit(429, headers(&[]), now), Some(None));
        assert_eq!(
            rate_limit(403, headers(&[("X-RateLimit-Remaining", "12")]), now),
            None,
            "a refused token, not a rate limit"
        );
        assert_eq!(
            rate_limit(403, headers(&[("X-RateLimit-Remaining", "4000"), ("Retry-After", "60")]), now),
            Some(Some(now.timestamp() + 60)),
            "GitHub's secondary limit"
        );
        assert_eq!(rate_limit(200, headers(&[]), now), None);
    }

    #[test]
    fn redirects_stay_on_the_host_over_https() {
        let url = |s: &str| reqwest::Url::parse(s).unwrap();
        assert!(redirect_allowed(Some(&url("https://gitlab.acme.io/a")), &url("https://GitLab.acme.io/b")));
        assert!(redirect_allowed(Some(&url("http://gitlab.lan/a")), &url("https://gitlab.lan/a")));
        assert!(!redirect_allowed(Some(&url("https://gitlab.acme.io/a")), &url("https://collector.evil/a")));
        assert!(!redirect_allowed(Some(&url("https://gitlab.acme.io/a")), &url("https://evil.gitlab.acme.io/a")));
        assert!(!redirect_allowed(Some(&url("https://gitlab.acme.io/a")), &url("http://gitlab.acme.io/a")));
        assert!(!redirect_allowed(None, &url("https://gitlab.acme.io/a")));
    }
}

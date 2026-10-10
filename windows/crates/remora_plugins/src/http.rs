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

/// `base` + `path` + encoded query parameters.
pub fn url_with_query(base: &str, path: &str, query: &[(&str, &str)]) -> String {
    let mut url = reqwest::Url::parse(&format!("{}/{}", base.trim_end_matches('/'), path.trim_start_matches('/')))
        .expect("valid base URL");
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
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                let from = attempt.previous().last().cloned();
                if attempt.previous().len() < 10 && redirect_allowed(from.as_ref(), attempt.url()) {
                    attempt.follow()
                } else {
                    attempt.stop()
                }
            }))
            .build()
            .expect("HTTP client");
        ReqwestClient { client }
    }
}

impl ReqwestClient {
    /// An avatar for the window: an image, at most `MAX_IMAGE`, sent with no header but the user agent.
    /// The caller checks the URL is allowed first.
    pub async fn get_image(&self, url: &str) -> Result<(String, Vec<u8>), PluginError> {
        let response = self.client.get(url).send().await.map_err(|e| PluginError::Network(e.to_string()))?;
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
        if response.status().is_redirection() {
            // Only refused redirects get here (see `redirect_allowed`).
            let location =
                response.headers().get(reqwest::header::LOCATION).and_then(|v| v.to_str().ok()).unwrap_or_default();
            let target = response.url().join(location).ok().and_then(|u| u.host_str().map(str::to_string));
            return Err(PluginError::BlockedRedirect(target.unwrap_or_else(|| "?".into())));
        }
        let body = read_capped(response, MAX_BODY).await?;
        Ok(Response { status, body: String::from_utf8_lossy(&body).into_owned() })
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
/// (`x-ratelimit-remaining` on GitHub, `RateLimit-Remaining` on GitLab). `resume` comes from `Retry-After` (seconds or
/// a date), else from the reset time those two give in seconds since 1970.
pub fn rate_limit(
    status: u16,
    header: impl Fn(&str) -> Option<String>,
    now: chrono::DateTime<chrono::Utc>,
) -> Option<Option<i64>> {
    let exhausted = ["x-ratelimit-remaining", "ratelimit-remaining"]
        .iter()
        .any(|name| header(name).is_some_and(|v| v.trim() == "0"));
    if status != 429 && !(status == 403 && exhausted) {
        return None;
    }
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
    Some(retry_after.or_else(reset))
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

/// Sends and decodes JSON, mapping HTTP failures to `PluginError`.
pub async fn decode<T: DeserializeOwned>(http: &dyn HttpClient, request: Request) -> Result<T, PluginError> {
    let response = http.send(request).await?;
    match response.status {
        200..=299 => serde_json::from_str(&response.body).map_err(|e| PluginError::Decode(e.to_string())),
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

    /// GitHub's 403 with nothing left, GitLab's reset header, Retry-After, and a refused token.
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

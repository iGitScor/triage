//! Signing in through the browser (`SignIn`): OAuth 2's authorization code with PKCE (RFC 7636), for an app that
//! keeps no secret. The browser comes back to a port on this computer (RFC 8252), open only while it waits.

use crate::{HttpClient, PluginConfig, PluginError, Request};
use remora_core::SignIn;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// The secret only this sign-in knows, and what the authorization server is shown of it.
pub struct Pkce {
    pub verifier: String,
    pub challenge: String,
}

impl Pkce {
    pub fn new() -> Self {
        let verifier = random(32);
        Pkce { challenge: challenge(&verifier), verifier }
    }
}

impl Default for Pkce {
    fn default() -> Self {
        Self::new()
    }
}

pub fn challenge(verifier: &str) -> String {
    base64url(&Sha256::digest(verifier.as_bytes()))
}

/// `bytes` random bytes, URL-safe.
pub fn random(bytes: usize) -> String {
    let mut buffer = vec![0; bytes];
    getrandom::fill(&mut buffer).expect("the system's random numbers");
    base64url(&buffer)
}

/// Base64 with `-` and `_`, unpadded.
pub fn base64url(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |n, (i, b)| n | (*b as u32) << (16 - 8 * i));
        for i in 0..=chunk.len() {
            out.push(ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char);
        }
    }
    out
}

/// The URL with each `{key}` replaced by that setting. Letters, digits, dots and dashes only, so a setting can't
/// change where the URL goes.
pub fn fill(template: &str, values: &HashMap<String, String>) -> Result<String, PluginError> {
    let mut url = template.to_string();
    while let Some(start) = url.find('{') {
        let end =
            url[start..].find('}').map(|end| start + end).ok_or_else(|| PluginError::InvalidField(url.clone()))?;
        let key = url[start + 1..end].to_string();
        let value = setting(values, &key)?;
        if !value.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-') {
            return Err(PluginError::InvalidField(key));
        }
        url.replace_range(start..=end, &value);
    }
    Ok(url)
}

fn setting(values: &HashMap<String, String>, key: &str) -> Result<String, PluginError> {
    let value = values.get(key).map(|v| v.trim().to_string()).unwrap_or_default();
    if value.is_empty() {
        Err(PluginError::MissingField(key.into()))
    } else {
        Ok(value)
    }
}

/// Where the browser goes to sign in.
pub fn authorize_url(
    sign_in: &SignIn,
    values: &HashMap<String, String>,
    redirect: &str,
    pkce: &Pkce,
    state: &str,
) -> Result<String, PluginError> {
    let client_id = setting(values, &sign_in.client_id_key)?;
    let scope = sign_in.scopes.join(" ");
    let url = reqwest::Url::parse_with_params(
        &fill(&sign_in.authorize_url, values)?,
        [
            ("client_id", client_id.as_str()),
            ("response_type", "code"),
            ("redirect_uri", redirect),
            ("response_mode", "query"),
            ("scope", &scope),
            ("state", state),
            ("code_challenge", &pkce.challenge),
            ("code_challenge_method", "S256"),
            ("prompt", "select_account"),
        ],
    )
    .map_err(|_| PluginError::InvalidField(sign_in.authorize_url.clone()))?;
    Ok(url.to_string())
}

/// What the token endpoint gives.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Tokens {
    pub access_token: String,
    /// Absent when the server keeps the previous one.
    #[serde(default)]
    pub refresh_token: Option<String>,
    /// Seconds.
    #[serde(default)]
    pub expires_in: Option<u64>,
}

/// Trades the code the browser brought back for tokens.
pub async fn redeem(
    http: &dyn HttpClient,
    sign_in: &SignIn,
    values: &HashMap<String, String>,
    code: &str,
    redirect: &str,
    verifier: &str,
) -> Result<Tokens, PluginError> {
    let client_id = setting(values, &sign_in.client_id_key)?;
    let scope = sign_in.scopes.join(" ");
    let fields = [
        ("grant_type", "authorization_code"),
        ("client_id", client_id.as_str()),
        ("code", code),
        ("redirect_uri", redirect),
        ("code_verifier", verifier),
        ("scope", &scope),
    ];
    tokens(http, &fill(&sign_in.token_url, values)?, &fields).await
}

/// A new access token from the refresh token, and maybe a new refresh token.
pub async fn refresh(
    http: &dyn HttpClient,
    sign_in: &SignIn,
    values: &HashMap<String, String>,
    refresh_token: &str,
) -> Result<Tokens, PluginError> {
    let client_id = setting(values, &sign_in.client_id_key)?;
    let scope = sign_in.scopes.join(" ");
    let fields = [
        ("grant_type", "refresh_token"),
        ("client_id", client_id.as_str()),
        ("refresh_token", refresh_token),
        ("scope", &scope),
    ];
    tokens(http, &fill(&sign_in.token_url, values)?, &fields).await
}

async fn tokens(http: &dyn HttpClient, url: &str, fields: &[(&str, &str)]) -> Result<Tokens, PluginError> {
    let response = http.send(Request::post_form(url, fields)).await?;
    if (200..300).contains(&response.status) {
        return serde_json::from_str(&response.body).map_err(|e| PluginError::Decode(e.to_string()));
    }
    #[derive(Deserialize)]
    struct Refusal {
        error: String,
    }
    // Only the error's code: its description can echo the request.
    match serde_json::from_str::<Refusal>(&response.body).map(|r| r.error) {
        // The sign-in expired, or was revoked: only signing in again helps.
        Ok(error) if error == "invalid_grant" => Err(PluginError::Unauthorized),
        Ok(error) => Err(PluginError::SignIn(error)),
        Err(_) if response.status == 401 => Err(PluginError::Unauthorized),
        Err(_) => Err(PluginError::Status(response.status)),
    }
}

/// An access token for the account: the one kept until a few minutes before it expires, else a new one from the
/// refresh token. A refresh token the server rotated is used from then on, and handed to the app to save
/// (`AccountCache::renew`). Call it once per fetch, before requests run in parallel.
pub async fn access_token(
    config: &PluginConfig,
    http: &dyn HttpClient,
    sign_in: &SignIn,
) -> Result<String, PluginError> {
    if let Some((token, until)) = config.cache.kept::<(String, Instant)>("oauth.access") {
        if Instant::now() < until {
            return Ok(token);
        }
    }
    let current = match config.cache.kept::<String>("oauth.refresh") {
        Some(renewed) => renewed,
        None => config.required("refreshToken")?,
    };
    let tokens = refresh(http, sign_in, &config.values, &current).await?;
    if let Some(renewed) = tokens.refresh_token.filter(|t| *t != current) {
        config.cache.keep("oauth.refresh", renewed.clone());
        config.cache.renew("refreshToken", renewed);
    }
    let lifetime = tokens.expires_in.unwrap_or(3600).saturating_sub(300).max(60);
    config.cache.keep("oauth.access", (tokens.access_token.clone(), Instant::now() + Duration::from_secs(lifetime)));
    Ok(tokens.access_token)
}

/// A port on this computer that waits for the browser to come back.
pub struct Loopback {
    listener: TcpListener,
    /// `http://localhost:<port>`: the organization registers `http://localhost`, any port.
    pub redirect: String,
}

/// What a request to the loopback port says.
#[derive(Debug, PartialEq)]
pub enum Callback {
    Code(String),
    Refused(String),
    /// Not the sign-in coming back (a favicon, another tab).
    Other,
}

impl Loopback {
    pub async fn bind() -> Result<Self, PluginError> {
        let listener =
            TcpListener::bind("127.0.0.1:0").await.map_err(|e| PluginError::SignIn(format!("no local port: {e}")))?;
        let port = listener.local_addr().map_err(|e| PluginError::SignIn(e.to_string()))?.port();
        Ok(Loopback { listener, redirect: format!("http://localhost:{port}") })
    }

    /// The code the browser brings back with `state`. Waits until then: the caller limits how long.
    pub async fn code(self, state: &str) -> Result<String, PluginError> {
        loop {
            let Ok((mut stream, _)) = self.listener.accept().await else { continue };
            let mut request = Vec::new();
            let mut buffer = [0; 1024];
            while request.len() < 8192 && !request.windows(4).any(|w| w == b"\r\n\r\n") {
                match stream.read(&mut buffer).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => request.extend_from_slice(&buffer[..n]),
                }
            }
            let line = String::from_utf8_lossy(&request).lines().next().unwrap_or_default().to_string();
            let outcome = callback(&line, state);
            let (status, page) = match outcome {
                Callback::Other => ("404 Not Found", ""),
                Callback::Code(_) => ("200 OK", SIGNED_IN),
                Callback::Refused(_) => ("200 OK", NOT_SIGNED_IN),
            };
            let answer = format!(
                "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\n\
                 Connection: close\r\n\r\n{page}",
                page.len()
            );
            let _ = stream.write_all(answer.as_bytes()).await;
            let _ = stream.shutdown().await;
            match outcome {
                Callback::Code(code) => return Ok(code),
                Callback::Refused(error) => return Err(PluginError::SignIn(error)),
                Callback::Other => {}
            }
        }
    }
}

/// Reads `GET /?code=…&state=… HTTP/1.1`. A different `state` isn't this sign-in's answer.
pub fn callback(request_line: &str, state: &str) -> Callback {
    let mut parts = request_line.split(' ');
    let (Some("GET"), Some(target)) = (parts.next(), parts.next()) else { return Callback::Other };
    let Ok(url) = reqwest::Url::parse(&format!("http://localhost{target}")) else { return Callback::Other };
    let query: HashMap<_, _> = url.query_pairs().into_owned().collect();
    if url.path() != "/" || query.get("state").map(String::as_str) != Some(state) {
        return Callback::Other;
    }
    match (query.get("code"), query.get("error")) {
        (Some(code), None) if !code.is_empty() => Callback::Code(code.clone()),
        (_, Some(error)) => Callback::Refused(error.clone()),
        _ => Callback::Refused("no code".into()),
    }
}

const SIGNED_IN: &str = "<!doctype html><meta charset=utf-8><title>Remora</title>\
<body style=\"font:16px system-ui;margin:3em;text-align:center\"><h1>✓</h1>\
<p>You’re signed in: you can close this tab and go back to Remora.</p>\
<p lang=fr>Vous êtes connecté : vous pouvez fermer cet onglet et revenir à Remora.</p>";

const NOT_SIGNED_IN: &str = "<!doctype html><meta charset=utf-8><title>Remora</title>\
<body style=\"font:16px system-ui;margin:3em;text-align:center\">\
<p>The sign-in didn’t finish: Remora says why.</p>\
<p lang=fr>La connexion n’a pas abouti : Remora vous dit pourquoi.</p>";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stub::StubHttp;
    use crate::Response;
    use async_trait::async_trait;
    use std::sync::Mutex;

    fn microsoft() -> SignIn {
        SignIn {
            authorize_url: "https://login.microsoftonline.com/{tenant}/oauth2/v2.0/authorize".into(),
            token_url: "https://login.microsoftonline.com/{tenant}/oauth2/v2.0/token".into(),
            client_id_key: "clientId".into(),
            scopes: vec!["offline_access".into(), "Mail.Read".into()],
        }
    }

    fn values(tenant: &str) -> HashMap<String, String> {
        HashMap::from([("tenant".into(), tenant.into()), ("clientId".into(), "app-1".into())])
    }

    #[test]
    fn pkce_follows_the_rfc() {
        // RFC 7636, appendix B.
        assert_eq!(
            challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
        assert_eq!((base64url(b"a"), base64url(b"ab"), base64url(b"abc")), ("YQ".into(), "YWI".into(), "YWJj".into()));
        assert_eq!(base64url(&[0xfb, 0xff]), "-_8");
        let (one, two) = (Pkce::new(), Pkce::new());
        assert_eq!(one.verifier.len(), 43);
        assert_ne!(one.verifier, two.verifier);
    }

    #[test]
    fn the_authorize_url_carries_the_challenge_and_the_tenant() {
        let pkce = Pkce::new();
        let url = authorize_url(&microsoft(), &values("acme.com"), "http://localhost:5050", &pkce, "s1").unwrap();
        let url = reqwest::Url::parse(&url).unwrap();
        assert_eq!(url.path(), "/acme.com/oauth2/v2.0/authorize");
        let query: HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(query["client_id"], "app-1");
        assert_eq!(query["redirect_uri"], "http://localhost:5050");
        assert_eq!(query["scope"], "offline_access Mail.Read");
        assert_eq!(query["code_challenge"], pkce.challenge);
        assert_eq!(query["code_challenge_method"], "S256");
        assert_eq!(query["state"], "s1");
    }

    #[test]
    fn a_setting_cant_change_where_the_url_goes() {
        let pkce = Pkce::new();
        for tenant in ["evil.com/x?", "a b", "x#"] {
            assert_eq!(
                authorize_url(&microsoft(), &values(tenant), "http://localhost:1", &pkce, "s").err(),
                Some(PluginError::InvalidField("tenant".into()))
            );
        }
        assert_eq!(
            authorize_url(&microsoft(), &values(" "), "http://localhost:1", &pkce, "s").err(),
            Some(PluginError::MissingField("tenant".into()))
        );
    }

    #[test]
    fn reads_the_browser_coming_back() {
        assert_eq!(callback("GET /?code=c%2B1&state=s1 HTTP/1.1", "s1"), Callback::Code("c+1".into()));
        assert_eq!(
            callback("GET /?error=access_denied&state=s1 HTTP/1.1", "s1"),
            Callback::Refused("access_denied".into())
        );
        assert_eq!(callback("GET /?code=c&state=other HTTP/1.1", "s1"), Callback::Other);
        assert_eq!(callback("GET /favicon.ico HTTP/1.1", "s1"), Callback::Other);
        assert_eq!(callback("POST /?code=c&state=s1 HTTP/1.1", "s1"), Callback::Other);
    }

    #[tokio::test]
    async fn the_loopback_waits_for_this_sign_in() {
        let loopback = Loopback::bind().await.unwrap();
        let port = loopback.redirect.rsplit(':').next().unwrap().to_string();
        let waiting = tokio::spawn(async move { loopback.code("s1").await });
        let get = |target: &'static str| {
            let port = port.clone();
            async move {
                let mut stream = tokio::net::TcpStream::connect(format!("127.0.0.1:{port}")).await.unwrap();
                stream.write_all(format!("GET {target} HTTP/1.1\r\nHost: localhost\r\n\r\n").as_bytes()).await.unwrap();
                let mut answer = String::new();
                stream.read_to_string(&mut answer).await.unwrap();
                answer
            }
        };
        assert!(get("/favicon.ico").await.starts_with("HTTP/1.1 404"));
        assert!(get("/?code=c1&state=s1").await.contains("You’re signed in"));
        assert_eq!(waiting.await.unwrap(), Ok("c1".into()));
    }

    /// Answers the token endpoint with `status` and `body`, keeping the form it was sent.
    struct TokenServer {
        status: u16,
        body: &'static str,
        sent: Mutex<Option<Request>>,
    }

    #[async_trait]
    impl HttpClient for TokenServer {
        async fn send(&self, request: Request) -> Result<Response, PluginError> {
            *self.sent.lock().unwrap() = Some(request);
            Ok(Response { status: self.status, body: self.body.into(), etag: None, reset: None })
        }
    }

    fn server(status: u16, body: &'static str) -> TokenServer {
        TokenServer { status, body, sent: Mutex::new(None) }
    }

    #[tokio::test]
    async fn redeems_the_code_with_the_verifier() {
        let http = server(200, r#"{"access_token": "a1", "refresh_token": "r1", "expires_in": 3600}"#);
        let tokens = redeem(&http, &microsoft(), &values("acme.com"), "c1", "http://localhost:5050", "v1").await;
        assert_eq!(
            tokens,
            Ok(Tokens { access_token: "a1".into(), refresh_token: Some("r1".into()), expires_in: Some(3600) })
        );
        let sent = http.sent.lock().unwrap().clone().unwrap();
        assert_eq!(sent.url, "https://login.microsoftonline.com/acme.com/oauth2/v2.0/token");
        assert!(sent.headers.contains(&("Content-Type".into(), "application/x-www-form-urlencoded".into())));
        let form: HashMap<_, _> = reqwest::Url::parse(&format!("http://x/?{}", sent.body.unwrap()))
            .unwrap()
            .query_pairs()
            .into_owned()
            .collect();
        assert_eq!(form["grant_type"], "authorization_code");
        assert_eq!(form["code_verifier"], "v1");
        assert_eq!(form["redirect_uri"], "http://localhost:5050");
        assert_eq!(form["scope"], "offline_access Mail.Read");
    }

    #[tokio::test]
    async fn the_access_token_is_kept_and_a_rotated_refresh_token_saved() {
        let http = server(200, r#"{"access_token": "a1", "refresh_token": "r2", "expires_in": 3600}"#);
        let mut values = values("acme.com");
        values.insert("refreshToken".into(), "r1".into());
        let config = PluginConfig { account_id: "acc".into(), values, cache: Default::default() };
        assert_eq!(access_token(&config, &http, &microsoft()).await, Ok("a1".into()));
        let sent = http.sent.lock().unwrap().take().unwrap();
        assert!(sent.body.unwrap().contains("refresh_token=r1"));
        assert_eq!(config.cache.take_renewed(), HashMap::from([("refreshToken".into(), "r2".into())]));
        assert_eq!(access_token(&config, &http, &microsoft()).await, Ok("a1".into()));
        assert!(http.sent.lock().unwrap().is_none(), "kept until it expires");
        config.cache.keep("oauth.access", ("a0".to_string(), Instant::now()));
        access_token(&config, &http, &microsoft()).await.unwrap();
        let sent = http.sent.lock().unwrap().take().unwrap();
        assert!(sent.body.unwrap().contains("refresh_token=r2"), "the rotated one from then on");
    }

    #[tokio::test]
    async fn an_expired_sign_in_asks_to_sign_in_again() {
        let expired = server(400, r#"{"error": "invalid_grant", "error_description": "AADSTS70008: expired"}"#);
        assert_eq!(refresh(&expired, &microsoft(), &values("acme.com"), "r0").await, Err(PluginError::Unauthorized));
        let unknown_app = server(400, r#"{"error": "unauthorized_client", "error_description": "AADSTS700016"}"#);
        assert_eq!(
            refresh(&unknown_app, &microsoft(), &values("acme.com"), "r0").await,
            Err(PluginError::SignIn("unauthorized_client".into()))
        );
        assert_eq!(
            refresh(&StubHttp::paths(&[]), &microsoft(), &values("acme.com"), "r0").await,
            Err(PluginError::Status(404))
        );
    }
}

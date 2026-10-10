//! Canned HTTP answers for plugin tests.
use crate::{HttpClient, PluginConfig, PluginError, Request, Response};
use async_trait::async_trait;
use std::collections::HashMap;

/// Answers by the end of the URL path, or by a word in the request body.
pub struct StubHttp {
    pub by_path: Vec<(&'static str, String)>,
    pub by_body: Vec<(&'static str, String)>,
}

impl StubHttp {
    pub fn paths(routes: &[(&'static str, &str)]) -> Self {
        StubHttp { by_path: routes.iter().map(|(k, v)| (*k, v.to_string())).collect(), by_body: vec![] }
    }

    pub fn bodies(routes: &[(&'static str, &str)]) -> Self {
        StubHttp { by_path: vec![], by_body: routes.iter().map(|(k, v)| (*k, v.to_string())).collect() }
    }
}

#[async_trait]
impl HttpClient for StubHttp {
    async fn send(&self, request: Request) -> Result<Response, PluginError> {
        let path = reqwest::Url::parse(&request.url).map(|u| u.path().to_string()).unwrap_or_default();
        let body = request.body.clone().unwrap_or_default();
        let answer = self
            .by_body
            .iter()
            .find(|(word, _)| body.contains(word))
            .or_else(|| self.by_path.iter().filter(|(suffix, _)| path.ends_with(suffix)).max_by_key(|(suffix, _)| suffix.len()));
        Ok(match answer {
            Some((_, json)) => Response { status: 200, body: json.clone() },
            None => Response { status: 404, body: String::new() },
        })
    }
}

pub fn config(values: &[(&str, &str)]) -> PluginConfig {
    PluginConfig { account_id: "acc".into(), values: values.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect::<HashMap<_, _>>() }
}

/// Records the paths a plugin requests, then answers with `base`.
pub struct Recording {
    base: StubHttp,
    paths: std::sync::Mutex<Vec<String>>,
}

impl Recording {
    pub fn new(base: StubHttp) -> Self {
        Recording { base, paths: std::sync::Mutex::new(vec![]) }
    }

    pub fn paths(&self) -> Vec<String> {
        self.paths.lock().unwrap().clone()
    }
}

#[async_trait]
impl HttpClient for Recording {
    async fn send(&self, request: Request) -> Result<Response, PluginError> {
        let path = reqwest::Url::parse(&request.url).map(|u| u.path().to_string()).unwrap_or_default();
        self.paths.lock().unwrap().push(path);
        self.base.send(request).await
    }
}

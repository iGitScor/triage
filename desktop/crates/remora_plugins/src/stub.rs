//! Canned HTTP answers for plugin tests.
use crate::{HttpClient, PluginConfig, PluginError, Request, Response};
use async_trait::async_trait;
use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

/// Answers by the end of the URL path, or by a word in the request body. A path route with a `?` also needs that
/// text in the query (`"/merge_requests?scope=created_by_me"`); the longest matching route wins.
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
        let url = reqwest::Url::parse(&request.url).ok();
        let path = url.as_ref().map(|u| u.path().to_string()).unwrap_or_default();
        let query = url.as_ref().and_then(|u| u.query().map(str::to_string)).unwrap_or_default();
        let matches = |route: &str| match route.split_once('?') {
            Some((suffix, part)) => path.ends_with(suffix) && query.contains(part),
            None => path.ends_with(route),
        };
        let body = request.body.clone().unwrap_or_default();
        let answer =
            self.by_body.iter().find(|(word, _)| body.contains(word)).or_else(|| {
                self.by_path.iter().filter(|(route, _)| matches(route)).max_by_key(|(route, _)| route.len())
            });
        Ok(match answer {
            Some((_, json)) => Response { status: 200, body: json.clone(), etag: None, reset: None },
            None => Response { status: 404, body: String::new(), etag: None, reset: None },
        })
    }
}

pub fn config(values: &[(&str, &str)]) -> PluginConfig {
    PluginConfig {
        account_id: "acc".into(),
        values: values.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect::<HashMap<_, _>>(),
        cache: Default::default(),
    }
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

/// What `registry`'s egress test runs for one plugin (TEST-03): an account's settings, and a stub answering its
/// fetch with the plugin's own fixtures.
pub struct EgressFixture {
    pub settings: Vec<(&'static str, &'static str)>,
    pub http: Arc<dyn HttpClient>,
}

/// Keeps the host of every request before the guard sees it, so a blocked one the plugin passes over still counts.
pub struct HostRecording {
    base: Arc<dyn HttpClient>,
    hosts: std::sync::Mutex<BTreeSet<String>>,
}

impl HostRecording {
    pub fn new(base: Arc<dyn HttpClient>) -> Self {
        HostRecording { base, hosts: Default::default() }
    }

    pub fn hosts(&self) -> Vec<String> {
        self.hosts.lock().unwrap().iter().cloned().collect()
    }
}

#[async_trait]
impl HttpClient for HostRecording {
    async fn send(&self, request: Request) -> Result<Response, PluginError> {
        self.hosts.lock().unwrap().insert(request.host());
        self.base.send(request).await
    }
}

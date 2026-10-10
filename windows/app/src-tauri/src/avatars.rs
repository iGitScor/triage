//! Avatars for the window. The page asks `avatar:` for an image (convertFileSrc), never the network: Rust
//! fetches it only if the inbox allows that URL now (an account's host, images allowed by the policy), with the same
//! TLS and redirect rules as the tools, at most 1 MB, and keeps it in memory. So the window's CSP allows no remote host.

use crate::AppState;
use remora_plugins::ReqwestClient;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use tauri::http::{Request, Response, StatusCode};
use tauri::{AppHandle, Manager};

const CACHE_SIZE: usize = 300;

/// URL → (content type, bytes).
type Cache = Mutex<HashMap<String, (String, Vec<u8>)>>;

fn client() -> &'static ReqwestClient {
    static CLIENT: OnceLock<ReqwestClient> = OnceLock::new();
    CLIENT.get_or_init(ReqwestClient::new)
}

fn cache() -> &'static Cache {
    static CACHE: OnceLock<Cache> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}

/// The original URL from `avatar://localhost/<encoded>` (or `http://avatar.localhost/<encoded>` on Windows).
pub fn original_url(path: &str) -> Option<String> {
    let encoded = path.trim_start_matches('/');
    let url = percent_encoding::percent_decode_str(encoded).decode_utf8().ok()?.into_owned();
    url.starts_with("https://").then_some(url)
}

fn not_found() -> Response<Vec<u8>> {
    Response::builder().status(StatusCode::NOT_FOUND).body(Vec::new()).expect("a response")
}

pub async fn serve(app: AppHandle, request: Request<Vec<u8>>) -> Response<Vec<u8>> {
    let Some(url) = original_url(request.uri().path()) else { return not_found() };
    // Checked at each request: a policy that turns images off, or a disconnected account, takes effect at once.
    let allowed = remora_app::view::image_allowed(&*app.state::<AppState>().inbox.lock().await, &url);
    if !allowed {
        return not_found();
    }
    let cached = cache().lock().ok().and_then(|c| c.get(&url).cloned());
    let (kind, bytes) = match cached {
        Some(hit) => hit,
        None => match client().get_image(&url).await {
            Ok(image) => {
                if let Ok(mut cache) = cache().lock() {
                    if cache.len() >= CACHE_SIZE {
                        cache.clear();
                    }
                    cache.insert(url, image.clone());
                }
                image
            }
            Err(_) => return not_found(),
        },
    };
    Response::builder()
        .header("Content-Type", kind)
        .header("Cache-Control", "max-age=3600")
        .body(bytes)
        .unwrap_or_else(|_| not_found())
}

/// Erase: no image kept from before.
pub fn forget() {
    if let Ok(mut cache) = cache().lock() {
        cache.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_original_url_is_decoded_and_https_only() {
        assert_eq!(
            original_url("/https%3A%2F%2Favatars.githubusercontent.com%2Fu%2F1%3Fv%3D4").as_deref(),
            Some("https://avatars.githubusercontent.com/u/1?v=4")
        );
        assert_eq!(original_url("/http%3A%2F%2Fexample.com%2Fa.png"), None, "never plain http");
        assert_eq!(original_url("/file%3A%2F%2F%2Fetc%2Fpasswd"), None);
    }
}

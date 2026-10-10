//! Remora's sources for the Windows app: GitHub, GitLab, Slack, Linear, Notion. A port of the macOS
//! `RemoraPlugins`: same queries, same mappings, same tests. Every plugin declares where its data goes
//! (`PluginManifest::egress`) and reaches the network only through a `GuardedHttpClient`.

pub mod claude;
mod code_review;
pub mod github;
pub mod gitlab;
pub mod http;
pub mod linear;
pub mod links;
pub mod notion;
pub mod openai;
pub mod plugin;
pub mod registry;
pub mod slack;

pub use http::*;
pub use plugin::*;
pub use registry::*;

#[cfg(test)]
pub(crate) mod stub;

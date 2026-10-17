//! The persistence port: where the inbox's files go. Each service saves its own files through it, right after each
//! change, so no caller has to remember to.

use crate::JsonStore;
use serde::de::DeserializeOwned;
use serde::Serialize;

pub(super) const ACCOUNTS: &str = "accounts";
pub(super) const CACHE: &str = "cache";
pub(super) const STATES: &str = "states";
pub(super) const REMINDERS: &str = "reminders";
pub(super) const PREFERENCES: &str = "preferences";
/// Every snooze, kept on this computer to learn habits and spot patterns.
pub(super) const HISTORY: &str = "snooze-history";
/// What you handle quickly or put off, to rank by your habits.
pub(super) const LEARNING: &str = "learning";
/// Timed reviews against their estimate, for review prep's pace.
pub(super) const REVIEW_TIMES: &str = "review-times";
/// What the assistant wrote: fetched again when stale, deleted when AI is no longer allowed.
pub(super) const BRIEF: &str = "brief";
pub(super) const SUMMARIES: &str = "bundle-summaries";
/// What the assistant used, per day (AI-03).
pub(super) const USAGE: &str = "assistant-usage";
/// What agents asked through the MCP server (V15-01), the most recent `AGENT_READS_KEPT`.
pub(super) const AGENT_READS: &str = "agent-reads";

/// A folder of JSON files, or nowhere: the demo and tests save nothing.
#[derive(Clone, Default)]
pub struct Persistence(Option<JsonStore>);

impl Persistence {
    pub fn new(store: Option<JsonStore>) -> Self {
        Persistence(store)
    }

    /// A saved file, or nothing (no file, no folder, or a file that can't be read).
    pub fn load<T: DeserializeOwned>(&self, name: &str) -> Option<T> {
        self.0.as_ref()?.load(name)
    }

    pub fn save<T: Serialize>(&self, name: &str, value: &T) {
        if let Some(store) = &self.0 {
            if let Err(error) = store.save(name, value) {
                eprintln!("Remora: could not save {name}: {error}");
            }
        }
    }

    /// Deletes every file.
    pub fn erase(&self) -> Result<(), String> {
        match &self.0 {
            Some(store) => store.erase().map_err(|e| e.to_string()),
            None => Ok(()),
        }
    }

    /// The files, for the app to flush when it quits.
    pub fn store(&self) -> Option<JsonStore> {
        self.0.clone()
    }
}

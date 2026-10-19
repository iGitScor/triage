//! Remora's domain and rules, shared by the Windows app. A port of the macOS app's `RemoraCore`
//! (see docs/develop/index.md): same concepts, same IDs, same tests.

pub mod assembler;
pub mod assistant;
pub mod backoff;
pub mod batch;
pub mod classifier;
pub mod clock;
pub mod command;
pub mod detector;
pub mod domain;
pub mod emoji;
pub mod failure;
pub mod handoff;
pub mod in_review;
pub mod link;
pub mod link_finder;
pub mod mcp;
pub mod message;
pub mod message_reach;
pub mod policy;
pub mod prioritizer;
pub mod ranker;
pub mod reaction;
pub mod recurrence;
pub mod redaction;
pub mod review_prep;
pub mod snooze_advisor;
pub mod task_stage;
pub mod text;
pub mod ticket_keys;
pub mod usage;
pub mod waiting;

pub use assembler::*;
pub use assistant::*;
pub use backoff::*;
pub use batch::*;
pub use classifier::*;
pub use clock::*;
pub use command::*;
pub use detector::*;
pub use domain::*;
pub use failure::*;
pub use handoff::*;
pub use link::*;
pub use link_finder::*;
pub use mcp::*;
pub use message::*;
pub use message_reach::*;
pub use policy::*;
pub use prioritizer::*;
pub use ranker::*;
pub use reaction::*;
pub use recurrence::*;
pub use redaction::redact;
pub use review_prep::*;
pub use snooze_advisor::*;
pub use task_stage::*;
pub use ticket_keys::*;
pub use usage::*;
pub use waiting::*;

#[cfg(test)]
pub(crate) mod fixtures;

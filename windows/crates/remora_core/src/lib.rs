//! Remora's domain and rules, shared by the Windows app. A port of the macOS app's `RemoraCore`
//! (see docs/develop/index.md): same concepts, same IDs, same tests.

pub mod assembler;
pub mod assistant;
pub mod backoff;
pub mod classifier;
pub mod clock;
pub mod detector;
pub mod domain;
pub mod emoji;
pub mod failure;
pub mod link_finder;
pub mod policy;
pub mod prioritizer;
pub mod ranker;
pub mod review_prep;
pub mod snooze_advisor;
pub mod text;
pub mod waiting;

pub use assembler::*;
pub use assistant::*;
pub use backoff::*;
pub use classifier::*;
pub use clock::*;
pub use detector::*;
pub use domain::*;
pub use failure::*;
pub use link_finder::*;
pub use policy::*;
pub use prioritizer::*;
pub use ranker::*;
pub use review_prep::*;
pub use snooze_advisor::*;
pub use waiting::*;

#[cfg(test)]
pub(crate) mod fixtures;

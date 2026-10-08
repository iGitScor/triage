//! Remora's domain and rules, shared by the Windows app. A port of the macOS app's `RemoraCore`
//! (see docs/develop/index.md): same concepts, same IDs, same tests.

pub mod assembler;
pub mod classifier;
pub mod clock;
pub mod detector;
pub mod domain;
pub mod policy;
pub mod prioritizer;

pub use assembler::*;
pub use classifier::*;
pub use clock::*;
pub use detector::*;
pub use domain::*;
pub use policy::*;
pub use prioritizer::*;

#[cfg(test)]
pub(crate) mod fixtures;

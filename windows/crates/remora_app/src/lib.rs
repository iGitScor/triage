//! Remora's application layer for Windows: where data lives, credentials, the managed policy and the
//! inbox service the tray app drives. No UI and no Tauri, so all of it is tested with plain `cargo test`.

pub mod demo;
pub mod inbox;
pub mod managed;
pub mod paths;
pub mod preferences;
pub mod store;
pub mod tray;
pub mod vault;
pub mod view;

pub use inbox::*;
pub use managed::Managed;
pub use preferences::*;
pub use store::JsonStore;
pub use vault::*;

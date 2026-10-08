//! Prints the demo inbox as the interface receives it, for the interface's preview and tests:
//! `cargo run -p remora_app --example demo_view > app/src/fixtures/demo.json`.

use remora_app::{demo, view, Inbox, Managed, MemoryVault};
use std::sync::Arc;

fn main() {
    let mut inbox = Inbox::in_memory(Arc::new(MemoryVault::default()), Managed::default());
    demo::load(&mut inbox, chrono::Utc::now());
    let output = serde_json::json!({
        "inbox": view::inbox_view(&inbox, "", true),
        "sources": inbox.sources(),
        "accounts": inbox.account_infos(),
        "preferences": inbox.preferences,
    });
    println!("{}", serde_json::to_string_pretty(&output).unwrap());
}

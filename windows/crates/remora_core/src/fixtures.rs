//! Test helpers shared by the rule tests, mirroring the macOS `Fixtures.swift`.
use crate::*;
use chrono::{DateTime, TimeZone, Utc};

pub fn now() -> DateTime<Utc> {
    Utc.timestamp_opt(1_800_000_000, 0).unwrap()
}

pub fn item(id: &str, bundle: InboxBundle) -> InboxItem {
    InboxItem {
        id: id.into(),
        account_id: "account".into(),
        plugin_id: "test".into(),
        bundle,
        title: format!("Title {id}"),
        context: format!("repo #{id}"),
        preview: None,
        url: None,
        app_url: None,
        author: None,
        participants: vec![],
        badges: vec![],
        date: now(),
        needs_action: false,
        priority: None,
        due: None,
        expires: None,
        changes: None,
        suggested_people: None,
    }
}

pub fn approved() -> Badge {
    Badge::new("approved", "Approved", Tone::Accent).notifying("Approved")
}

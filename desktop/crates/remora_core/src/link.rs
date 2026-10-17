//! `remora://` links: what Shortcuts, Raycast, Alfred, a Stream Deck or a script may ask of Remora. Only
//! what the user then sees: open the inbox (on a tab), open the reminder panel filled in, refresh. Nothing that marks
//! an item done or snoozes it without the user. Same rule on macOS (`RemoraLink.swift`).

use chrono::{DateTime, Local, NaiveDateTime, TimeZone, Utc};
use serde::Serialize;

pub const LINK_SCHEME: &str = "remora";
/// A reminder's text from a link is cut there: a link can't fill the panel with a page of text.
pub const LINK_TEXT_LIMIT: usize = 200;

/// The tabs a link (or the `remora` command) may name, and the app's own names for them.
pub(crate) const TABS: [(&str, &str); 5] = [
    ("in-progress", "inProgress"),
    ("my-turn", "myTurn"),
    ("waiting", "waiting"),
    ("snoozed", "snoozed"),
    ("done", "done"),
];

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "action", rename_all = "camelCase")]
pub enum Link {
    /// `remora://open`, `remora://open?tab=waiting`: the app's tab name.
    Open { tab: Option<String> },
    /// `remora://remind?text=Call%20Erin&at=2026-10-14T09:00`: the reminder panel, filled in, for the user to save.
    Remind { text: String, at: Option<DateTime<Utc>> },
    /// `remora://refresh`.
    Refresh,
}

/// What a `remora://` link asks, or None for anything else (another scheme, an unknown action). A time already past
/// is left for the user to pick.
pub fn parse_link(url: &str, now: DateTime<Utc>) -> Option<Link> {
    let (scheme, rest) = url.trim().split_once(':')?;
    if !scheme.eq_ignore_ascii_case(LINK_SCHEME) {
        return None;
    }
    let rest = rest.trim_start_matches('/');
    let (action, query) = rest.split_once('?').unwrap_or((rest, ""));
    let value = |name: &str| {
        query
            .split('&')
            .filter_map(|pair| pair.split_once('='))
            .find(|(key, _)| decode(key) == name)
            .map(|(_, v)| decode(v))
    };
    match action.trim_end_matches('/').to_ascii_lowercase().as_str() {
        "open" => {
            let tab = value("tab").and_then(|tab| {
                TABS.iter().find(|(name, _)| name.eq_ignore_ascii_case(tab.trim())).map(|(_, id)| id.to_string())
            });
            Some(Link::Open { tab })
        }
        "remind" => {
            let text: String = value("text").unwrap_or_default().trim().chars().take(LINK_TEXT_LIMIT).collect();
            let at = value("at").and_then(|at| time(&at)).filter(|at| *at > now);
            Some(Link::Remind { text, at })
        }
        "refresh" => Some(Link::Refresh),
        _ => None,
    }
}

/// "2026-10-14T09:00:00+02:00", or "2026-10-14T09:00" on this computer's clock.
pub(crate) fn time(raw: &str) -> Option<DateTime<Utc>> {
    let raw = raw.trim();
    if let Ok(at) = DateTime::parse_from_rfc3339(raw) {
        return Some(at.with_timezone(&Utc));
    }
    ["%Y-%m-%dT%H:%M", "%Y-%m-%dT%H:%M:%S", "%Y-%m-%d %H:%M"]
        .iter()
        .find_map(|format| NaiveDateTime::parse_from_str(raw, format).ok())
        .and_then(|at| Local.from_local_datetime(&at).earliest())
        .map(|at| at.with_timezone(&Utc))
}

/// Percent-decoding, `+` as a space, as a query string has it. A broken escape stays as written.
fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => {
                let escape = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
                match escape.and_then(|hex| u8::from_str_radix(hex, 16).ok()) {
                    Some(byte) => {
                        out.push(byte);
                        i += 2;
                    }
                    None => out.push(b'%'),
                }
            }
            byte => out.push(byte),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, 13, 12, 0, 0).unwrap()
    }

    #[test]
    fn opens_the_inbox_on_a_tab() {
        assert_eq!(parse_link("remora://open", now()), Some(Link::Open { tab: None }));
        assert_eq!(parse_link("REMORA://open/?tab=Waiting", now()), Some(Link::Open { tab: Some("waiting".into()) }));
        assert_eq!(
            parse_link("remora://open?tab=in-progress", now()),
            Some(Link::Open { tab: Some("inProgress".into()) })
        );
        assert_eq!(
            parse_link("remora://open?tab=nowhere", now()),
            Some(Link::Open { tab: None }),
            "an unknown tab: the inbox"
        );
    }

    #[test]
    fn fills_in_the_reminder_panel_without_saving_it() {
        let link = parse_link("remora://remind?text=Call%20Erin+back&at=2026-10-14T09:00:00Z", now());
        assert_eq!(
            link,
            Some(Link::Remind {
                text: "Call Erin back".into(),
                at: Some(Utc.with_ymd_and_hms(2026, 10, 14, 9, 0, 0).unwrap())
            })
        );
        let local = parse_link("remora://remind?at=2026-10-14T09:00", now());
        let expected = Local.with_ymd_and_hms(2026, 10, 14, 9, 0, 0).unwrap().with_timezone(&Utc);
        assert_eq!(local, Some(Link::Remind { text: String::new(), at: Some(expected) }));
        let past = parse_link("remora://remind?text=x&at=2026-10-01T09:00:00Z", now());
        assert_eq!(past, Some(Link::Remind { text: "x".into(), at: None }), "a time already past: the user picks");
        let long = format!("remora://remind?text={}", "a".repeat(500));
        let Some(Link::Remind { text, .. }) = parse_link(&long, now()) else { panic!() };
        assert_eq!(text.len(), LINK_TEXT_LIMIT);
        assert_eq!(
            parse_link("remora://remind?text=%ZZ%25é%", now()),
            Some(Link::Remind { text: "%ZZ%é%".into(), at: None })
        );
    }

    #[test]
    fn refreshes_and_refuses_the_rest() {
        assert_eq!(parse_link("remora://refresh", now()), Some(Link::Refresh));
        for refused in ["remora://done?id=1", "remora://snooze", "https://remora/open", "remora:", "open"] {
            assert_eq!(parse_link(refused, now()), None, "{refused}");
        }
    }
}

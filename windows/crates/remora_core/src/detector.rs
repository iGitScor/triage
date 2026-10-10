use crate::InboxItem;
use serde::Serialize;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum NoticeKind {
    Arrival,
    StatusChange,
    Reminder,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Notice {
    pub kind: NoticeKind,
    pub item_id: String,
    /// English text, translated by the UI.
    pub title: String,
    pub subtitle: String,
    pub body: String,
    pub url: Option<String>,
}

impl Notice {
    /// Keeps what kind of notice it is and where it comes from, hides what was written.
    /// The body is English, translated by the UI like the title.
    pub fn redacted(&self) -> Notice {
        let body = if self.kind == NoticeKind::Reminder { "Reminder" } else { "New activity" };
        Notice { body: body.into(), ..self.clone() }
    }
}

/// Compares two snapshots of one account and tells what is worth a notification.
/// The first sync of an account (`previous` is None) is silent.
pub fn notices(previous: Option<&[InboxItem]>, current: &[InboxItem]) -> Vec<Notice> {
    let Some(previous) = previous else { return vec![] };
    let before: HashMap<&str, &InboxItem> = previous.iter().map(|i| (i.id.as_str(), i)).collect();
    let mut notices = vec![];
    for item in current {
        match before.get(item.id.as_str()) {
            None if item.needs_action && item.counts() => notices.push(Notice {
                kind: NoticeKind::Arrival,
                item_id: item.id.clone(),
                title: item.bundle.title.clone(),
                subtitle: item.context.clone(),
                body: item.title.clone(),
                url: item.url.clone(),
            }),
            None => {}
            Some(old) => {
                for badge in item.badges.iter().filter(|b| b.notify.is_some() && !old.has_badge(&b.id)) {
                    notices.push(Notice {
                        kind: NoticeKind::StatusChange,
                        item_id: item.id.clone(),
                        title: badge.notify.clone().unwrap_or_else(|| badge.label.clone()),
                        subtitle: item.context.clone(),
                        body: item.title.clone(),
                        url: item.url.clone(),
                    });
                }
            }
        }
    }
    notices
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::*;
    use crate::{Badge, InboxBundle, Tone};

    #[test]
    fn first_sync_is_silent() {
        let mut item = item("1", InboxBundle::reviews());
        item.needs_action = true;
        assert!(notices(None, &[item]).is_empty());
    }

    #[test]
    fn new_actionable_item_is_announced() {
        let mut actionable = item("1", InboxBundle::reviews());
        actionable.needs_action = true;
        let found = notices(Some(&[]), &[actionable, item("2", InboxBundle::reviews())]);
        assert_eq!(found.iter().map(|n| n.item_id.as_str()).collect::<Vec<_>>(), ["1"]);
        assert_eq!(found[0].kind, NoticeKind::Arrival);
    }

    #[test]
    fn notifying_badge_is_announced_once_and_silent_badges_are_ignored() {
        let before = vec![item("1", InboxBundle::authored())];
        let mut after = before.clone();
        after[0].badges = vec![approved(), Badge::new("draft", "Draft", Tone::Neutral)];
        let found = notices(Some(&before), &after);
        assert_eq!(found.iter().map(|n| n.title.as_str()).collect::<Vec<_>>(), ["Approved"]);
        assert!(notices(Some(&after), &after).is_empty());
    }

    #[test]
    fn redacted_notice_keeps_where_but_not_what() {
        let notice = Notice {
            kind: NoticeKind::Arrival,
            item_id: "1".into(),
            title: "Mentions".into(),
            subtitle: "#general".into(),
            body: "Salary review moved".into(),
            url: None,
        };
        let redacted = notice.redacted();
        assert_eq!(
            (redacted.title.as_str(), redacted.subtitle.as_str(), redacted.item_id.as_str()),
            ("Mentions", "#general", "1")
        );
        assert!(!redacted.body.contains("Salary"));
    }
}

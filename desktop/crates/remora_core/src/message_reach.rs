//! How directly a message reached you (Swift's `MessageReach`): To reply orders by it, and its rows say when someone
//! wrote again or a message has waited a day.
use crate::{badge, Badge, InboxBundle, InboxItem, Tone};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// First to last: a one-to-one conversation, a mention of you by name, a reply in a thread you started, a group
/// conversation, a reply in a thread you only wrote in. Only the order: everything stays in To reply and counted. A
/// message from another tool (a GitHub, Jira or Linear comment) stands with the mentions. As macOS.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum MessageReach {
    Direct,
    Mention,
    OwnThread,
    Group,
    Thread,
}

impl MessageReach {
    /// Unanswered this long, a one-to-one message or a mention of you says so.
    pub const LONG_WAIT: Duration = Duration::days(1);
}

impl InboxItem {
    /// What its row says of a message left in To reply: the same person wrote again since you last answered (only
    /// while their last message still asks something: a "never mind" went to Done), and a one-to-one message or a
    /// mention of you unanswered for a day. Elsewhere, neither: a follow-up that closes the conversation lifts
    /// nothing.
    pub(crate) fn marked_for_reply(mut self, now: DateTime<Utc>) -> InboxItem {
        if self.bundle != InboxBundle::reply() {
            self.followed_up = None;
            return self;
        }
        if self.followed_up == Some(true) {
            self.badges.push(Badge::new(badge::FOLLOWED_UP, "Followed up", Tone::Accent));
        }
        if self.reach.is_some_and(|reach| reach <= MessageReach::Mention) && now - self.date > MessageReach::LONG_WAIT {
            self.badges.push(Badge::new(badge::AGE, "Waiting over a day", Tone::Warning));
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::{item, now};
    use crate::{Prioritizer, VerbClassifier};

    /// The order of To reply, and what its rows say: a follow-up whose last message closes the conversation lifts
    /// nothing, and only a one-to-one message or a mention says it has waited a day. As macOS.
    #[test]
    fn followed_up_first_then_by_reach() {
        let message = |id: &str, reach: Option<MessageReach>, followed_up: bool, text: &str| {
            let mut item = item(id, InboxBundle::direct_messages());
            item.title = text.into();
            item.date = now() - Duration::days(2);
            item.reach = reach;
            item.followed_up = followed_up.then_some(true);
            VerbClassifier::classify_at(item, now())
        };
        let items = vec![
            message("thread", Some(MessageReach::Thread), false, "Can you?"),
            message("group", Some(MessageReach::Group), false, "Can you?"),
            message("own", Some(MessageReach::OwnThread), false, "Can you?"),
            message("other tool", None, false, "Can you?"),
            message("direct", Some(MessageReach::Direct), false, "Can you?"),
            message("again", Some(MessageReach::Thread), true, "Can you?"),
        ];
        let mut sorted = items.clone();
        Prioritizer::new(now()).sort(&mut sorted);
        let ids: Vec<&str> = sorted.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(ids, ["again", "direct", "other tool", "own", "group", "thread"]);
        assert!(items[5].has_badge(badge::FOLLOWED_UP) && items[4].has_badge(badge::AGE));
        assert!(!items[1].has_badge(badge::AGE));
        let closed = message("closed", Some(MessageReach::Direct), true, "Thanks, all good!");
        assert!(closed.bundle != InboxBundle::reply() && closed.followed_up.is_none());
        assert!(!closed.has_badge(badge::FOLLOWED_UP));
    }
}

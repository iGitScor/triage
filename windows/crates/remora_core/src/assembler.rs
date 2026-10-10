use crate::{InboxBundle, InboxItem, ItemState, PersonalScore, Prioritizer, SnoozeMode};
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Placement {
    InProgress,
    Inbox,
    Snoozed,
    Done,
    Cleared,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Group {
    pub bundle: InboxBundle,
    pub items: Vec<InboxItem>,
}

/// What the UI shows: what you're on, pinned items, your turn, waiting on others, snoozed, done.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InboxLayout {
    /// Started by the user and not finished yet. Kept out of every other section.
    pub in_progress: Vec<InboxItem>,
    pub pinned: Vec<InboxItem>,
    pub my_turn: Vec<Group>,
    pub waiting: Vec<Group>,
    pub snoozed: Vec<InboxItem>,
    pub done: Vec<InboxItem>,
}

impl InboxLayout {
    pub fn my_turn_items(&self) -> impl Iterator<Item = &InboxItem> {
        self.pinned.iter().chain(self.my_turn.iter().flat_map(|g| g.items.iter()))
    }

    pub fn waiting_count(&self) -> usize {
        self.waiting.iter().map(|g| g.items.len()).sum()
    }

    /// What the tray counts: your turn, without quiet "To read" and low-priority items.
    pub fn action_count(&self) -> usize {
        self.my_turn_items().filter(|i| i.counts()).count()
    }

    /// Items per plugin, the plugin holding the most pressing verb first, then the busiest.
    pub fn counts_by_source(&self) -> Vec<(String, usize)> {
        let mut counts: HashMap<&str, (usize, i32)> = HashMap::new();
        for item in self.my_turn_items().filter(|i| i.counts()) {
            let entry = counts.entry(&item.plugin_id).or_insert((0, i32::MAX));
            entry.0 += 1;
            entry.1 = entry.1.min(item.bundle.rank);
        }
        let mut sorted: Vec<(String, usize, i32)> = counts.into_iter().map(|(id, (n, rank))| (id.to_string(), n, rank)).collect();
        sorted.sort_by(|a, b| a.2.cmp(&b.2).then(b.1.cmp(&a.1)).then(a.0.cmp(&b.0)));
        sorted.into_iter().map(|(id, n, _)| (id, n)).collect()
    }
}

pub struct InboxAssembler {
    pub wake_on_activity: bool,
}

impl InboxAssembler {
    pub fn placement(&self, item: &InboxItem, state: Option<&ItemState>, now: DateTime<Utc>) -> Placement {
        let kept = state.is_some_and(|s| s.started_at.is_some() || s.pinned);
        if item.expires.is_some_and(|expires| expires <= now) && !kept {
            return Placement::Cleared;
        }
        let Some(state) = state else { return Placement::Inbox };
        if state.started_at.is_some() {
            return Placement::InProgress;
        }
        if let Some(snooze) = &state.snooze {
            if snooze.mode == SnoozeMode::Hide && snooze.until > now {
                let untouched = snooze.fingerprint == item.fingerprint();
                let wakes = self.wake_on_activity || snooze.until_news == Some(true);
                if untouched || !wakes {
                    return Placement::Snoozed;
                }
            }
        }
        if let Some(done) = &state.done {
            if done.fingerprint == item.fingerprint() {
                return if done.cleared_at.is_none() { Placement::Done } else { Placement::Cleared };
            }
        }
        Placement::Inbox
    }

    /// Your turn when its verb says so (or, for unclassified kinds, when it needs action), or when a
    /// reminder or snooze just brought it back.
    pub fn is_my_turn(item: &InboxItem, state: Option<&ItemState>) -> bool {
        if state.and_then(|s| s.reminded_at).is_some() {
            return true;
        }
        item.bundle.is_mine().unwrap_or(item.needs_action)
    }

    pub fn layout(&self, items: &[InboxItem], states: &HashMap<String, ItemState>, now: DateTime<Utc>, query: &str) -> InboxLayout {
        self.layout_ranked(items, states, now, query, None)
    }

    /// The layout with your habits breaking ties inside each group (`PersonalRanker`).
    pub fn layout_ranked(
        &self,
        items: &[InboxItem],
        states: &HashMap<String, ItemState>,
        now: DateTime<Utc>,
        query: &str,
        personal: Option<PersonalScore>,
    ) -> InboxLayout {
        let mut layout = InboxLayout::default();
        let mut mine: HashMap<InboxBundle, Vec<InboxItem>> = HashMap::new();
        let mut theirs: HashMap<InboxBundle, Vec<InboxItem>> = HashMap::new();
        let mut matching: Vec<&InboxItem> = items.iter().filter(|i| matches(i, query)).collect();
        matching.sort_by_key(|i| std::cmp::Reverse(i.date));

        for item in matching {
            let state = states.get(&item.id);
            match self.placement(item, state, now) {
                Placement::InProgress => layout.in_progress.push(item.clone()),
                Placement::Snoozed => layout.snoozed.push(item.clone()),
                Placement::Done => layout.done.push(item.clone()),
                Placement::Cleared => {}
                Placement::Inbox if state.map(|s| s.pinned).unwrap_or(false) => layout.pinned.push(item.clone()),
                Placement::Inbox if Self::is_my_turn(item, state) => mine.entry(item.bundle.clone()).or_default().push(item.clone()),
                Placement::Inbox => theirs.entry(item.bundle.clone()).or_default().push(item.clone()),
            }
        }
        layout.my_turn = groups(mine, now, personal);
        layout.waiting = groups(theirs, now, personal);
        let started = |i: &InboxItem| states.get(&i.id).and_then(|s| s.started_at).unwrap_or(now);
        layout.in_progress.sort_by_key(started);
        let until = |i: &InboxItem| states.get(&i.id).and_then(|s| s.snooze.as_ref()).map(|s| s.until).unwrap_or(now);
        layout.snoozed.sort_by_key(until);
        let done_at = |i: &InboxItem| states.get(&i.id).and_then(|s| s.done.as_ref()).map(|d| d.at).unwrap_or(now);
        layout.done.sort_by_key(|i| std::cmp::Reverse(done_at(i)));
        layout
    }
}

fn groups(items: HashMap<InboxBundle, Vec<InboxItem>>, now: DateTime<Utc>, personal: Option<PersonalScore>) -> Vec<Group> {
    let prioritizer = Prioritizer::with_personal(now, personal);
    let mut groups: Vec<Group> = items
        .into_iter()
        .map(|(bundle, mut items)| {
            prioritizer.sort(&mut items);
            Group { bundle, items }
        })
        .collect();
    groups.sort_by(|a, b| a.bundle.rank.cmp(&b.bundle.rank).then(a.bundle.title.cmp(&b.bundle.title)));
    groups
}

fn matches(item: &InboxItem, query: &str) -> bool {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return true;
    }
    [item.title.as_str(), item.context.as_str(), item.author.as_ref().map(|a| a.name.as_str()).unwrap_or(""), item.preview.as_deref().unwrap_or("")]
        .iter()
        .any(|field| field.to_lowercase().contains(&query))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::*;
    use crate::{Mark, Priority, Snooze};
    use chrono::Duration;

    fn assembler() -> InboxAssembler {
        InboxAssembler { wake_on_activity: true }
    }

    fn snooze(item: &InboxItem, minutes: i64, mode: SnoozeMode, fingerprint: Option<&str>) -> ItemState {
        ItemState {
            snooze: Some(Snooze {
                until: now() + Duration::minutes(minutes),
                mode,
                note: None,
                fingerprint: fingerprint.map(String::from).unwrap_or_else(|| item.fingerprint()),
                reason: None,
                until_news: None,
            }),
            ..Default::default()
        }
    }

    #[test]
    fn done_stays_done_until_it_changes() {
        let item = item("1", InboxBundle::reviews());
        let state = ItemState { done: Some(Mark { at: now(), fingerprint: item.fingerprint(), cleared_at: None }), ..Default::default() };
        assert_eq!(assembler().placement(&item, Some(&state), now()), Placement::Done);
        let mut updated = item.clone();
        updated.badges = vec![approved()];
        assert_eq!(assembler().placement(&updated, Some(&state), now()), Placement::Inbox);
    }

    #[test]
    fn hidden_snooze_lasts_until_its_time_and_remind_stays_visible() {
        let item = item("1", InboxBundle::reviews());
        let hidden = snooze(&item, 1, SnoozeMode::Hide, None);
        assert_eq!(assembler().placement(&item, Some(&hidden), now()), Placement::Snoozed);
        assert_eq!(assembler().placement(&item, Some(&hidden), now() + Duration::minutes(2)), Placement::Inbox);
        let remind = snooze(&item, 1, SnoozeMode::Remind, None);
        assert_eq!(assembler().placement(&item, Some(&remind), now()), Placement::Inbox);
    }

    #[test]
    fn activity_wakes_a_snooze_only_when_enabled_or_until_news() {
        let item = item("1", InboxBundle::reviews());
        let stale = snooze(&item, 1, SnoozeMode::Hide, Some("stale"));
        assert_eq!(assembler().placement(&item, Some(&stale), now()), Placement::Inbox);
        let off = InboxAssembler { wake_on_activity: false };
        assert_eq!(off.placement(&item, Some(&stale), now()), Placement::Snoozed);
        let mut news = stale.clone();
        news.snooze.as_mut().unwrap().until_news = Some(true);
        assert_eq!(off.placement(&item, Some(&news), now()), Placement::Inbox);
    }

    #[test]
    fn started_items_leave_every_other_section() {
        let mut items: Vec<InboxItem> = ["1", "2", "3"].iter().map(|id| item(id, InboxBundle::reviews())).collect();
        items.iter_mut().for_each(|i| i.needs_action = true);
        let started = |minutes| ItemState { started_at: Some(now() - Duration::minutes(minutes)), ..Default::default() };
        let states = HashMap::from([("1".to_string(), started(1)), ("3".to_string(), started(10))]);
        let layout = assembler().layout(&items, &states, now(), "");
        assert_eq!(layout.in_progress.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(), ["3", "1"]);
        assert_eq!(layout.my_turn_items().map(|i| i.id.as_str()).collect::<Vec<_>>(), ["2"]);
        assert_eq!(layout.action_count(), 1);
    }

    #[test]
    fn splits_by_whose_turn_it_is_and_counts() {
        let mut review = item("1", InboxBundle::reviews());
        review.needs_action = true;
        let waiting = item("2", InboxBundle::awaiting());
        let reminder = item("3", InboxBundle::reminders());
        let back = item("4", InboxBundle::authored());
        let read = item("5", InboxBundle::read());
        let mut low = item("6", InboxBundle::tasks());
        low.priority = Some(Priority::Low);
        let states = HashMap::from([("4".to_string(), ItemState { reminded_at: Some(now()), ..Default::default() })]);
        let layout = assembler().layout(&[review, waiting, reminder, back, read, low], &states, now(), "");
        let mut mine: Vec<&str> = layout.my_turn_items().map(|i| i.id.as_str()).collect();
        mine.sort_unstable();
        assert_eq!(mine, ["1", "3", "4", "5", "6"]);
        assert_eq!(layout.waiting_count(), 1);
        assert_eq!(layout.action_count(), 3, "To read and low priority are shown but not counted");
    }

    #[test]
    fn sources_are_ordered_by_their_most_pressing_verb() {
        let mut review = item("1", InboxBundle::reviews());
        review.plugin_id = "github".into();
        let mut second = item("2", InboxBundle::reviews());
        second.plugin_id = "github".into();
        let mut question = item("3", InboxBundle::reply());
        question.plugin_id = "slack".into();
        let layout = assembler().layout(&[review, second, question], &HashMap::new(), now(), "");
        assert_eq!(layout.counts_by_source(), [("slack".to_string(), 1), ("github".to_string(), 2)]);
    }

    #[test]
    fn query_filters_on_title_and_context() {
        let layout = assembler().layout(&[item("1", InboxBundle::reviews()), item("22", InboxBundle::reviews())], &HashMap::new(), now(), "#22");
        assert_eq!(layout.my_turn_items().count(), 1);
    }
}

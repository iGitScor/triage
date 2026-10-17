use crate::{emoji, InboxItem};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// An emoji reaction on the message an item comes from (Slack). Not part of the item's fingerprint: a new 👍
/// neither brings a Done item back nor wakes a snooze. Same as macOS.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Reaction {
    /// The emoji's name as the tool writes it (`white_check_mark`), skin tone dropped.
    pub name: String,
    pub count: u32,
    /// Whether you are among the people who reacted.
    pub mine: bool,
    /// A workspace's own emoji has no character, only a picture.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    /// 👍, or `:name:` for a workspace's own emoji: what the window shows (macOS computes it).
    #[serde(default)]
    pub emoji: String,
}

/// What a reaction says about the message (the list is in `scripts/make-emoji.py`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ReactionMeaning {
    /// ✅: handled. Yours moves the item to Done; theirs on your question answers it.
    Done,
    /// 👀: being looked at. Yours moves the item to In progress.
    Started,
    /// 👍: seen, fine.
    Ack,
    /// ❓: they need something from you.
    Question,
}

impl ReactionMeaning {
    pub const ALL: [ReactionMeaning; 4] =
        [ReactionMeaning::Done, ReactionMeaning::Started, ReactionMeaning::Ack, ReactionMeaning::Question];

    /// A standard emoji's meaning, else a word of a workspace emoji's name (`:approved:`, `:merci-beaucoup:`).
    pub fn of(name: &str) -> Option<ReactionMeaning> {
        let name = name.to_lowercase();
        if let Some((_, meaning)) = emoji::MEANINGS.iter().find(|(n, _)| *n == name) {
            return Some(*meaning);
        }
        let words: Vec<&str> = name.split(['_', '-', '+']).chain([name.as_str()]).collect();
        emoji::MEANING_WORDS
            .iter()
            .find(|(_, list)| words.iter().any(|w| list.contains(w)))
            .map(|(meaning, _)| *meaning)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            ReactionMeaning::Done => "done",
            ReactionMeaning::Started => "started",
            ReactionMeaning::Ack => "ack",
            ReactionMeaning::Question => "question",
        }
    }
}

impl Reaction {
    pub fn new(name: &str, count: u32, mine: bool) -> Self {
        let name = Self::base(name);
        let emoji =
            emoji::TABLE.iter().find(|(n, _)| *n == name).map_or_else(|| format!(":{name}:"), |(_, e)| e.to_string());
        Reaction { name, count: count.max(u32::from(mine)), mine, image: None, emoji }
    }

    /// `+1::skin-tone-2` → `+1`.
    fn base(name: &str) -> String {
        name.split("::").next().unwrap_or(name).to_string()
    }

    pub fn meaning(&self) -> Option<ReactionMeaning> {
        ReactionMeaning::of(&self.name)
    }

    /// How many people other than you reacted.
    pub fn others(&self) -> u32 {
        self.count - u32::from(self.mine)
    }

    /// One reaction per emoji: skin tones of the same emoji are added up.
    pub fn merged(reactions: Vec<Reaction>) -> Vec<Reaction> {
        let mut out: Vec<Reaction> = vec![];
        for reaction in reactions {
            match out.iter_mut().find(|r| r.name == reaction.name) {
                Some(existing) => {
                    existing.count += reaction.count;
                    existing.mine |= reaction.mine;
                    existing.image = existing.image.take().or(reaction.image);
                }
                None => out.push(reaction),
            }
        }
        out
    }
}

impl InboxItem {
    fn reaction_list(&self) -> &[Reaction] {
        self.reactions.as_deref().unwrap_or_default()
    }

    /// What your own reaction says, if it says something: handled beats being looked at.
    pub fn your_reaction(&self) -> Option<ReactionMeaning> {
        let yours: Vec<ReactionMeaning> =
            self.reaction_list().iter().filter(|r| r.mine).filter_map(Reaction::meaning).collect();
        if yours.contains(&ReactionMeaning::Done) {
            Some(ReactionMeaning::Done)
        } else if yours.contains(&ReactionMeaning::Started) {
            Some(ReactionMeaning::Started)
        } else {
            None
        }
    }

    /// Whether someone other than you reacted with this meaning.
    pub fn others_reacted(&self, meaning: ReactionMeaning) -> bool {
        self.reaction_list().iter().any(|r| r.others() > 0 && r.meaning() == Some(meaning))
    }

    /// The reactions as the assistant reads them: meanings and counts, never who.
    pub fn reaction_notes(&self) -> Vec<String> {
        let mut notes = vec![];
        if let Some(yours) = self.your_reaction() {
            notes.push(format!("you reacted: {}", yours.as_str()));
        }
        for meaning in ReactionMeaning::ALL {
            let count: u32 =
                self.reaction_list().iter().filter(|r| r.meaning() == Some(meaning)).map(Reaction::others).sum();
            if count > 0 {
                notes.push(format!("others reacted: {} ×{count}", meaning.as_str()));
            }
        }
        notes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::{item, now};
    use crate::{InboxAssembler, InboxBundle, ItemState, Mark, PersonalRanker, Placement, VerbClassifier};

    fn reacted(reactions: Vec<Reaction>) -> InboxItem {
        let mut item = item("r", InboxBundle::reply());
        item.needs_action = true;
        item.reactions = Some(reactions);
        item
    }

    fn yours(name: &str) -> Reaction {
        Reaction::new(name, 1, true)
    }

    /// Same cases as macOS (`ReactionTests`).
    #[test]
    fn meanings() {
        assert_eq!(ReactionMeaning::of("white_check_mark"), Some(ReactionMeaning::Done));
        assert_eq!(Reaction::new("+1::skin-tone-3", 1, false).meaning(), Some(ReactionMeaning::Ack), "skin tone");
        assert_eq!(ReactionMeaning::of("eyes"), Some(ReactionMeaning::Started));
        assert_eq!(ReactionMeaning::of("thinking_face"), Some(ReactionMeaning::Question));
        assert_eq!(ReactionMeaning::of("approved-by-design"), Some(ReactionMeaning::Done), "a word of its name");
        assert_eq!(ReactionMeaning::of("merci_beaucoup"), Some(ReactionMeaning::Ack));
        assert_eq!(ReactionMeaning::of("hmm"), Some(ReactionMeaning::Question));
        assert_eq!(ReactionMeaning::of("tada"), Some(ReactionMeaning::Ack));
        assert_eq!(ReactionMeaning::of("pizza"), None);
    }

    #[test]
    fn emoji_by_name() {
        assert_eq!(Reaction::new("+1::skin-tone-2", 1, false).emoji, "👍");
        assert_eq!(Reaction::new("approved", 1, false).emoji, ":approved:");
    }

    #[test]
    fn skin_tones_add_up() {
        let merged = Reaction::merged(vec![
            Reaction::new("+1", 2, false),
            Reaction::new("+1::skin-tone-2", 1, true),
            Reaction::new("eyes", 1, false),
        ]);
        assert_eq!(merged.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), ["+1", "eyes"]);
        assert!(merged[0].count == 3 && merged[0].mine && merged[0].others() == 2);
    }

    /// Your ✅ is Done, your 👀 In progress; a pin or a reminder keeps the item where you put it.
    #[test]
    fn your_reaction_places_the_item() {
        let assembler = InboxAssembler { wake_on_activity: true, group_bots: false };
        let at = |item: &InboxItem, state: Option<&ItemState>| assembler.placement(item, state, now());
        assert_eq!(at(&reacted(vec![yours("white_check_mark")]), None), Placement::Done);
        assert_eq!(at(&reacted(vec![yours("eyes"), yours("white_check_mark")]), None), Placement::Done);
        assert_eq!(at(&reacted(vec![yours("eyes")]), None), Placement::InProgress);
        assert_eq!(at(&reacted(vec![Reaction::new("white_check_mark", 1, false)]), None), Placement::Inbox);
        let pinned = ItemState { pinned: true, ..Default::default() };
        assert_eq!(at(&reacted(vec![yours("white_check_mark")]), Some(&pinned)), Placement::Inbox);
        let item = reacted(vec![yours("white_check_mark")]);
        let cleared = ItemState {
            done: Some(Mark { at: now(), fingerprint: item.fingerprint(), cleared_at: Some(now()) }),
            ..Default::default()
        };
        assert_eq!(at(&item, Some(&cleared)), Placement::Cleared, "Clear all wins");
    }

    /// A new 👍 neither brings a Done item back nor wakes a snooze.
    #[test]
    fn reactions_are_not_in_the_fingerprint() {
        assert_eq!(reacted(vec![]).fingerprint(), reacted(vec![Reaction::new("+1", 4, false)]).fingerprint());
    }

    /// Your own message waits on the others only when it asked something.
    #[test]
    fn your_questions_wait() {
        let mut question = item("q", InboxBundle::asked());
        question.title = "Can someone review the mockups?".into();
        let waiting = VerbClassifier::classify(question);
        assert!(waiting.bundle == InboxBundle::awaiting() && !waiting.needs_action && waiting.expires.is_none());
        assert!(!InboxAssembler::is_my_turn(&waiting, None), "in Waiting");

        let mut update = item("u", InboxBundle::asked());
        update.title = "Deployed to staging, FYI".into();
        let cleared = VerbClassifier::classify(update);
        assert_eq!(cleared.expires, Some(cleared.date));
        assert_eq!(
            InboxAssembler { wake_on_activity: true, group_bots: false }.placement(&cleared, None, now()),
            Placement::Cleared
        );
    }

    #[test]
    fn the_ranker_and_the_assistant_see_what_reactions_mean() {
        let item = reacted(vec![yours("white_check_mark"), Reaction::new("+1", 3, true)]);
        let features = PersonalRanker::features(&item);
        assert!(features.contains(&"reacted:done".to_string()) && features.contains(&"reacted:ack".to_string()));
        let theirs = PersonalRanker::features(&reacted(vec![Reaction::new("eyes", 1, false)]));
        assert!(theirs.contains(&"their:started".to_string()));
        assert_eq!(item.reaction_notes(), ["you reacted: done", "others reacted: ack ×2"]);
    }
}

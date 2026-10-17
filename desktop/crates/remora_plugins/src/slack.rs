use crate::{
    decode, url_with_query, AccountCache, HttpClient, PluginConfig, PluginError, Request, SourcePlugin, SourceSnapshot,
    IDENTITY_LIFETIME,
};
use async_trait::async_trait;
use chrono::{DateTime, Datelike, Duration, FixedOffset, Local, TimeZone, Utc};
use regex::Regex;
use remora_core::{ConfigField, Egress, InboxBundle, InboxItem, Person, PluginManifest, Reaction, ReactionMeaning};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock};

pub fn manifest() -> PluginManifest {
    PluginManifest {
        id: "slack".into(),
        name: "Slack".into(),
        summary: "Mentions, direct messages and replies in your threads from the last few days.".into(),
        fields: vec![
            ConfigField::token(
                "User OAuth token",
                "A user token (xoxp-…) with search:read, plus channels:history and groups:history for thread replies, and reactions:read and emoji:read for reactions.",
            ),
            ConfigField {
                key: "days".into(),
                label: "Look back (days)".into(),
                placeholder: "3".into(),
                default_value: "3".into(),
                is_secret: false,
                is_optional: false,
                help: None,
                managed_key: None,
                is_toggle: false,
            },
        ],
        setup_steps: vec![
            "Click “Create the Slack app”, pick your workspace, then Next and Create. The app comes pre-filled.".into(),
            "In the app page, open “Install App” and click “Install to <workspace>”, then Allow.".into(),
            "Copy the “User OAuth Token” (it starts with xoxp-) and paste it below.".into(),
        ],
        setup_label: "Create the Slack app".into(),
        setup_url: app_manifest_url(),
        egress: Egress {
            hosts: vec!["slack.com".into()],
            description:
                "Searches your recent mentions and direct messages in Slack, and reads the threads you wrote in.".into(),
            external_ai: false,
        },
        logo: Some("slack".into()),
        sign_in: None,
    }
}

/// Opens Slack's "create app" flow pre-filled with the read-only scopes Remora needs: search, the channels'
/// history for replies in your threads, and reactions.
pub fn app_manifest_url() -> Option<String> {
    let manifest = r#"{"display_information":{"name":"Remora","description":"Mentions and DMs in your inbox"},"oauth_config":{"scopes":{"user":["search:read","channels:history","groups:history","reactions:read","emoji:read"]}},"settings":{"org_deploy_enabled":false,"socket_mode_enabled":false,"token_rotation_enabled":false}}"#;
    url_with_query("https://api.slack.com", "apps", &[("new_app", "1"), ("manifest_json", manifest)]).ok()
}

/// Recent mentions and direct messages, found with Slack's search API.
/// Slack's errors for a token that no longer works: revoked, expired, or its account deactivated. Same list as macOS.
pub const REJECTED_TOKEN: [&str; 5] =
    ["invalid_auth", "not_authed", "token_revoked", "token_expired", "account_inactive"];

pub struct SlackPlugin {
    account_id: String,
    token: String,
    days: i64,
    http: Arc<dyn HttpClient>,
    cache: Arc<AccountCache>,
}

impl SlackPlugin {
    pub fn new(config: &PluginConfig, http: Arc<dyn HttpClient>) -> Result<Self, PluginError> {
        Ok(SlackPlugin {
            account_id: config.account_id.clone(),
            token: config.required("token")?,
            days: config.get("days").parse().unwrap_or(3).max(1),
            http,
            cache: config.cache.clone(),
        })
    }

    async fn call<T: DeserializeOwned>(&self, method: &str, query: &[(&str, &str)]) -> Result<T, PluginError> {
        let mut query = query.to_vec();
        if method == "search.messages" {
            query.extend([("sort", "timestamp"), ("sort_dir", "desc"), ("count", "40")]);
        }
        let request = Request::get(url_with_query("https://slack.com/api", method, &query)?)
            .header("Authorization", format!("Bearer {}", self.token));
        let envelope: serde_json::Value = decode(self.http.as_ref(), request).await?;
        if envelope.get("ok").and_then(|v| v.as_bool()) != Some(true) {
            let error = envelope.get("error").and_then(|v| v.as_str()).unwrap_or("unknown error");
            // A token that no longer works, whatever Slack calls it: the user reconnects. Same list as macOS.
            return Err(if REJECTED_TOKEN.contains(&error) {
                PluginError::Unauthorized
            } else {
                PluginError::Api(format!("Slack: {error}"))
            });
        }
        serde_json::from_value(envelope).map_err(|e| PluginError::Decode(e.to_string()))
    }

    fn snapshot(
        heads: &[Head],
        identity: String,
        me: &str,
        team: Option<&str>,
        account_id: &str,
        reactions: &HashMap<String, Vec<Reaction>>,
    ) -> SourceSnapshot {
        let items = heads
            .iter()
            .map(|head| {
                let m = head.message;
                let mut item = m.item(account_id, head.bundle.clone(), &head.id, me, team);
                item.reactions = reactions.get(&format!("{}/{}", m.channel.id, m.ts)).cloned();
                let mine = [InboxBundle::asked(), InboxBundle::answered(), InboxBundle::finished()];
                if !mine.contains(&head.bundle) {
                    return item;
                }
                item.bundle = head.bundle.clone();
                item.expires = None;
                item.needs_action = false;
                if head.bundle == InboxBundle::finished() {
                    item
                } else {
                    weighed(item, &m.channel.label())
                }
            })
            .collect();
        SourceSnapshot { identity, items, remarks: vec![] }
    }

    /// The workspace's own emoji, by name: `alias:white_check_mark` for another name of a standard one, else the
    /// picture's address. Asked once a day; without the emoji:read permission there are none, and reactions with
    /// a workspace emoji show as `:name:`, their meaning taken from the name.
    async fn custom_emoji(&self) -> HashMap<String, String> {
        let load = || async {
            Ok(self.call::<CustomEmoji>("emoji.list", &[]).await.map(|list| list.emoji).unwrap_or_default())
        };
        self.cache.value("emoji.list", EMOJI_LIFETIME, load).await.unwrap_or_default()
    }

    /// The messages you reacted to lately, by `channel/ts`, with everyone's reactions: one request for every item.
    /// Without the reactions:read permission, a remark says how to add it.
    async fn reacted_by_you(
        &self,
        me: &str,
        custom: &HashMap<String, String>,
    ) -> (HashMap<String, Vec<Reaction>>, Option<String>) {
        match self.call::<Reacted>("reactions.list", &[("count", "100"), ("full", "true")]).await {
            Ok(reacted) => {
                let by_message = reacted
                    .items
                    .into_iter()
                    .filter_map(|item| serde_json::from_value::<ReactedItem>(item).ok())
                    .filter_map(|item| {
                        let message = item.message?;
                        let reactions = reactions(&message.reactions, me, custom)?;
                        Some((format!("{}/{}", item.channel?, message.ts?), reactions))
                    })
                    .collect();
                (by_message, None)
            }
            Err(PluginError::Api(message)) if message.ends_with("missing_scope") => {
                (HashMap::new(), Some(MISSING_REACTIONS.to_string()))
            }
            Err(_) => (HashMap::new(), None),
        }
    }

    /// The reactions on these messages, by `channel/ts`. A message whose reactions can't be read just has none.
    async fn reactions_on(
        &self,
        messages: &[&Match],
        me: &str,
        custom: &HashMap<String, String>,
    ) -> HashMap<String, Vec<Reaction>> {
        let asked = messages.iter().map(|m| async move {
            let key = format!("{}/{}", m.channel.id, m.ts);
            let query = [("channel", m.channel.id.as_str()), ("timestamp", m.ts.as_str()), ("full", "true")];
            let got = self
                .cache
                .value(&format!("reactions.get {key}"), REACTIONS_LIFETIME, || {
                    self.call::<Got>("reactions.get", &query)
                })
                .await
                .ok();
            (key, got.and_then(|g| g.message).and_then(|m| reactions(&m.reactions, me, custom)))
        });
        futures::future::join_all(asked).await.into_iter().filter_map(|(key, r)| Some((key, r?))).collect()
    }

    /// The threads you wrote in lately, as of your last message there: what search can't find, since nobody
    /// mentioned you. The 10 most recent channel threads, one `conversations.replies` each. A newer reply is to
    /// read or answer (one that mentions you is already a mention); without one, their reactions on your message
    /// speak: ❓ is a question for you, ✅ an answer, and otherwise your message waits (if it asked something, as
    /// `VerbClassifier` decides). Without the history scopes, a remark says how to add them. Same rules as macOS.
    async fn thread_replies(
        &self,
        mine: Vec<Match>,
        me: &str,
        team: Option<&str>,
        custom: &HashMap<String, String>,
    ) -> Result<(Vec<InboxItem>, Vec<remora_core::Message>), PluginError> {
        let mut failed = None;
        let mut latest: HashMap<(String, String), Match> = HashMap::new();
        for message in mine.into_iter().filter(|m| !m.channel.is_direct()) {
            let key = (message.channel.id.clone(), message.thread_ts());
            if latest.get(&key).is_none_or(|known| known.date() < message.date()) {
                latest.insert(key, message);
            }
        }
        let mut threads: Vec<Match> = latest.into_values().collect();
        threads.sort_by_key(|m| std::cmp::Reverse(m.date()));
        let mut items = vec![];
        for thread in threads.into_iter().take(10) {
            let thread_ts = thread.thread_ts();
            let query = [
                ("channel", thread.channel.id.as_str()),
                ("ts", thread_ts.as_str()),
                ("oldest", thread.ts.as_str()),
                ("inclusive", "true"),
                ("limit", "50"),
            ];
            let replies = match self.call::<Replies>("conversations.replies", &query).await {
                Ok(replies) => replies,
                Err(PluginError::Api(message)) if message.ends_with("missing_scope") => {
                    return Ok((items, vec![MISSING_HISTORY.into()]))
                }
                Err(error) => {
                    if let Some(limit) = crate::rate_limit_of(&error) {
                        return Err(limit);
                    }
                    failed = failed.or(Some(error));
                    continue;
                }
            };
            let mention = format!("<@{me}>");
            // Slack always sends the thread's first message too, whatever `oldest` says: only what came after yours.
            let after: Vec<&Reply> = replies
                .messages
                .iter()
                .filter(|r| r.date() > thread.date() && r.user.as_deref() != Some(me) && r.bot_id.is_none())
                .collect();
            if let Some(reply) = after.iter().filter(|r| !r.text.contains(&mention)).max_by_key(|r| r.date()) {
                let mut item = reply.item(&thread, &self.account_id, me, team);
                item.reactions = reactions(&reply.reactions, me, custom);
                items.push(item);
                continue;
            }
            if !after.is_empty() {
                continue;
            }
            let yours = replies.messages.iter().find(|r| r.ts == thread.ts);
            let on_yours = yours.and_then(|r| reactions(&r.reactions, me, custom));
            // Someone else wrote before you (the thread's first message, at least): yours is an answer.
            let answered = replies
                .messages
                .iter()
                .any(|r| r.date() < thread.date() && r.user.as_deref().is_some_and(|u| u != me) && r.bot_id.is_none());
            items.push(unanswered(&thread, answered, on_yours, &self.account_id, me, team));
        }
        // A thread that couldn't be read is said, not passed over.
        Ok((items, failed.iter().map(|e| crate::partial("Replies in your threads", e)).collect()))
    }
}

/// How many messages that need you get their reactions asked for, each kept 10 minutes: Slack's search doesn't
/// return them, and one request per message adds up.
pub const REACTIONS_ASKED: usize = 15;
const REACTIONS_LIFETIME: std::time::Duration = std::time::Duration::from_secs(600);
const EMOJI_LIFETIME: std::time::Duration = std::time::Duration::from_secs(86_400);

/// Shown when the token can't read reactions. Same text on macOS.
const MISSING_REACTIONS: &str = "Slack: reactions need the reactions:read permission. Add it to the Remora app in Slack, reinstall it, and paste the new token.";

/// The newest messages that become items, theirs or your answers. Apps' messages aren't asked about.
fn needing_you<'a>(heads: &[Head<'a>]) -> Vec<&'a Match> {
    let mut all: Vec<&Match> = heads.iter().map(|h| h.message).filter(|m| !m.is_from_app()).collect();
    all.sort_by_key(|m| std::cmp::Reverse(m.date()));
    all.truncate(REACTIONS_ASKED);
    all
}

/// The message a conversation's item shows: theirs when they spoke last, yours when you did.
struct Head<'a> {
    message: &'a Match,
    bundle: InboxBundle,
    id: String,
}

/// How long after a mention a message of yours in the same channel counts as the answer, when it doesn't name the
/// person: in a busy channel, a later message of yours may be about something else.
const ANSWER_WINDOW: Duration = Duration::hours(2);

/// Whose turn each conversation is. A direct conversation is theirs to answer when they wrote last, else yours
/// waits (`answered`, or `asked` when they never wrote). A mention is answered by your later message in its
/// thread, which the thread's own item then follows (`finished` here), or, outside a thread, by your later message
/// in the channel that names them or comes within `ANSWER_WINDOW`. Same rules as macOS.
fn conversations<'a>(mentions: &'a [Match], direct: &'a [Match], sent: &'a [Match], me: &str) -> Vec<Head<'a>> {
    let yours: Vec<&Match> = sent.iter().filter(|m| m.user.as_deref().is_none_or(|u| u == me)).collect();
    let mut heads = vec![];
    for mention in mentions.iter().filter(|m| !m.channel.is_direct()) {
        let id = format!("{}/{}", mention.channel.id, mention.ts);
        let later: Vec<&&Match> =
            yours.iter().filter(|m| m.channel.id == mention.channel.id && m.date() > mention.date()).collect();
        let in_its_thread = later.iter().any(|m| {
            let thread = m.thread_ts();
            thread != m.ts && (thread == mention.thread_ts() || thread == mention.ts)
        });
        if in_its_thread {
            heads.push(Head { message: mention, bundle: InboxBundle::finished(), id });
            continue;
        }
        let answer = later
            .iter()
            .filter(|m| {
                m.thread_ts() == m.ts
                    && (mention.user.as_ref().is_some_and(|u| m.text.contains(&format!("<@{u}>")))
                        || m.date() - mention.date() <= ANSWER_WINDOW)
            })
            .max_by_key(|m| m.date());
        heads.push(match answer {
            Some(answer) => Head { message: answer, bundle: InboxBundle::answered(), id },
            None => Head { message: mention, bundle: InboxBundle::mentions(), id },
        });
    }
    let owned: Vec<&Match> = yours.clone();
    let theirs: HashMap<&str, &Match> =
        latest_per_conversation(direct).into_iter().map(|m| (m.channel.id.as_str(), m)).collect();
    let ours: HashMap<&str, &Match> = latest_of(&owned).into_iter().map(|m| (m.channel.id.as_str(), m)).collect();
    let mut channels: Vec<&str> = theirs.keys().chain(ours.keys()).copied().collect();
    channels.sort_unstable();
    channels.dedup();
    for channel in channels {
        let their = theirs.get(channel).copied();
        match ours.get(channel).copied() {
            Some(mine) if their.is_none_or(|t| mine.date() > t.date()) => {
                let bundle = if their.is_none() { InboxBundle::asked() } else { InboxBundle::answered() };
                heads.push(Head { message: mine, bundle, id: channel.to_string() });
            }
            _ => {
                if let Some(message) = their {
                    heads.push(Head { message, bundle: InboxBundle::direct_messages(), id: channel.to_string() });
                }
            }
        }
    }
    heads
}

/// The latest of your messages in each direct conversation.
fn latest_of<'a>(messages: &[&'a Match]) -> Vec<&'a Match> {
    let mut latest: HashMap<&str, &Match> = HashMap::new();
    for m in messages.iter().filter(|m| m.channel.is_direct()) {
        let entry = latest.entry(m.channel.id.as_str()).or_insert(m);
        if m.date() > entry.date() {
            *entry = m;
        }
    }
    latest.into_values().collect()
}

/// Your message, last in a conversation: their ✅ finishes it, their ❓ or 🤔 makes it yours to answer, and otherwise
/// `VerbClassifier` decides whether it waits on them.
fn weighed(mut item: InboxItem, label: &str) -> InboxItem {
    let theirs: Vec<&Reaction> = item.reactions.iter().flatten().filter(|r| r.others() > 0).collect();
    if theirs.iter().any(|r| r.meaning() == Some(ReactionMeaning::Done)) {
        item.bundle = InboxBundle::finished();
        return item;
    }
    let questions: Vec<String> =
        theirs.iter().filter(|r| r.meaning() == Some(ReactionMeaning::Question)).map(|r| r.emoji.clone()).collect();
    if !questions.is_empty() {
        item.context = format!("{} on your message in {label}", questions.join(" "));
        item.bundle = InboxBundle::reply();
        item.needs_action = true;
    }
    item
}

fn latest_per_conversation(direct: &[Match]) -> Vec<&Match> {
    let mut latest: HashMap<&str, &Match> = HashMap::new();
    for m in direct.iter().filter(|m| m.channel.is_direct()) {
        let entry = latest.entry(m.channel.id.as_str()).or_insert(m);
        if m.date() > entry.date() {
            *entry = m;
        }
    }
    latest.into_values().collect()
}

/// Slack's reactions as items carry them, one per emoji, a workspace emoji resolved; None when there are none.
fn reactions(wire: &[WireReaction], me: &str, custom: &HashMap<String, String>) -> Option<Vec<Reaction>> {
    if wire.is_empty() {
        return None;
    }
    let count = |r: &WireReaction| r.count.unwrap_or(r.users.len() as u32);
    Some(Reaction::merged(
        wire.iter()
            .map(|r| resolve(Reaction::new(&r.name, count(r), r.users.iter().any(|u| u == me)), custom))
            .collect(),
    ))
}

/// A reaction with a workspace emoji: an alias becomes the emoji it stands for, another gets its picture.
pub fn resolve(reaction: Reaction, custom: &HashMap<String, String>) -> Reaction {
    let mut name = reaction.name.clone();
    for _ in 0..3 {
        match custom.get(&name).and_then(|value| value.strip_prefix("alias:")) {
            Some(target) => name = target.to_string(),
            None => break,
        }
    }
    let image = custom.get(&name).filter(|value| !value.starts_with("alias:")).cloned();
    Reaction { image, ..Reaction::new(&name, reaction.count, reaction.mine) }
}

/// Your last message in a thread nobody wrote back in, as the item: `answered` when someone else wrote before you,
/// else `asked`; their reactions on it are weighed (`weighed`).
fn unanswered(
    yours: &Match,
    answered: bool,
    reactions: Option<Vec<Reaction>>,
    account_id: &str,
    me: &str,
    team: Option<&str>,
) -> InboxItem {
    let id = format!("{}/thread/{}", yours.channel.id, yours.thread_ts());
    let mut item = yours.item(account_id, InboxBundle::asked(), &id, me, team);
    item.bundle = if answered { InboxBundle::answered() } else { InboxBundle::asked() };
    item.expires = None;
    item.context = format!("Thread in {}", yours.channel.label());
    item.needs_action = false;
    item.reactions = reactions;
    weighed(item, &yours.channel.label())
}

/// Shown when the token can't read threads. Same text on macOS.
const MISSING_HISTORY: &str = "Slack: replies in your threads need the channels:history and groups:history permissions. Add them to the Remora app in Slack, reinstall it, and paste the new token.";

#[async_trait]
impl SourcePlugin for SlackPlugin {
    async fn fetch(&self) -> Result<SourceSnapshot, PluginError> {
        let auth: Auth = self.cache.value("auth.test", IDENTITY_LIFETIME, || self.call("auth.test", &[])).await?;
        let since = (Utc::now() - Duration::days(self.days)).format("%Y-%m-%d").to_string();
        let me = format!("<@{}>", auth.user_id);
        let mentions_query = format!("{me} -from:{me} after:{since}");
        let direct_query = format!("to:me -from:{me} after:{since}");
        let mentions_params = [("query", mentions_query.as_str())];
        let direct_params = [("query", direct_query.as_str())];
        let mine_query = format!("from:{me} is:thread after:{since}");
        let mine_params = [("query", mine_query.as_str())];
        // Your own recent messages: an answer of yours ends a conversation's turn.
        let sent_query = format!("from:{me} after:{since}");
        let sent_params = [("query", sent_query.as_str())];
        // The workspace's own emoji first (kept a day): an alias of ✅ must count as ✅ wherever reactions decide.
        let custom = self.custom_emoji().await;
        let (mentions, direct, mine, sent, (by_you, reactions_remark)) = futures::join!(
            self.call::<Search>("search.messages", &mentions_params),
            self.call::<Search>("search.messages", &direct_params),
            self.call::<Search>("search.messages", &mine_params),
            self.call::<Search>("search.messages", &sent_params),
            self.reacted_by_you(&auth.user_id, &custom)
        );
        let (mentions, direct) = (mentions?.messages.matches, direct?.messages.matches);
        let mut partial = vec![];
        // Your own messages decide whose turn it is and which threads to read: best-effort, but said when missing.
        let mut yours = |search: Result<Search, PluginError>| match search {
            Ok(search) => Ok(search.messages.matches),
            Err(error) => {
                if let Some(limit) = crate::rate_limit_of(&error) {
                    return Err(limit);
                }
                if partial.is_empty() {
                    partial.push(crate::partial("Your recent messages", &error));
                }
                Ok(vec![])
            }
        };
        let sent = yours(sent)?;
        let mine = yours(mine)?;
        let heads = conversations(&mentions, &direct, &sent, &auth.user_id);
        // Search results carry no reactions: ask for those of the newest messages that matter, unless the token
        // can't read reactions at all.
        let mut reactions = by_you;
        if reactions_remark.is_none() {
            reactions.extend(self.reactions_on(&needing_you(&heads), &auth.user_id, &custom).await);
        }
        let mut snapshot = Self::snapshot(
            &heads,
            format!("{} · {}", auth.user, auth.team),
            &auth.user_id,
            auth.team_id.as_deref(),
            &self.account_id,
            &reactions,
        );
        let (threads, remarks) = self.thread_replies(mine, &auth.user_id, auth.team_id.as_deref(), &custom).await?;
        snapshot.items.extend(threads);
        snapshot.remarks.extend(remarks);
        snapshot.remarks.extend(partial);
        snapshot.remarks.extend(reactions_remark.map(Into::into));
        Ok(snapshot)
    }
}

#[derive(Deserialize, Clone)]
struct Auth {
    user_id: String,
    user: String,
    team: String,
    team_id: Option<String>,
}

#[derive(Deserialize)]
struct Search {
    messages: Messages,
}

/// A reaction as Slack sends it: on a message from `conversations.replies`, `reactions.list` or `reactions.get`.
#[derive(Deserialize, Clone)]
struct WireReaction {
    name: String,
    #[serde(default)]
    users: Vec<String>,
    count: Option<u32>,
}

/// `reactions.list`: the messages you reacted to, newest first. Files and other kinds have no message; each
/// item is read on its own, so one odd item doesn't drop the others.
#[derive(Deserialize)]
struct Reacted {
    #[serde(default)]
    items: Vec<serde_json::Value>,
}

#[derive(Deserialize)]
struct ReactedItem {
    channel: Option<String>,
    message: Option<Reactions>,
}

/// A message's reactions, from `reactions.list` or `reactions.get`.
#[derive(Deserialize, Clone)]
struct Reactions {
    ts: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    reactions: Vec<WireReaction>,
}

/// `reactions.get`.
#[derive(Deserialize, Clone)]
struct Got {
    message: Option<Reactions>,
}

/// `emoji.list`: the workspace's own emoji.
#[derive(Deserialize)]
struct CustomEmoji {
    #[serde(default)]
    emoji: HashMap<String, String>,
}

/// A thread's messages from `conversations.replies`, the root first.
#[derive(Deserialize)]
struct Replies {
    messages: Vec<Reply>,
}

#[derive(Deserialize)]
struct Reply {
    ts: String,
    #[serde(default)]
    text: String,
    user: Option<String>,
    bot_id: Option<String>,
    #[serde(default)]
    user_profile: Option<Profile>,
    #[serde(default, deserialize_with = "lenient")]
    reactions: Vec<WireReaction>,
}

#[derive(Deserialize)]
struct Profile {
    display_name: Option<String>,
    real_name: Option<String>,
}

impl Reply {
    fn date(&self) -> DateTime<Utc> {
        let seconds = self.ts.parse::<f64>().unwrap_or(0.0);
        Utc.timestamp_opt(seconds as i64, 0).single().unwrap_or_default()
    }

    /// "Thread in #design": one item per thread, its latest reply, opening the thread where you wrote.
    fn item(&self, thread: &Match, account_id: &str, me: &str, team: Option<&str>) -> InboxItem {
        let plain = plain_text(&self.text, me, &DateStyle::system());
        let lines: Vec<&str> = plain.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
        let preview = lines.iter().skip(1).copied().collect::<Vec<_>>().join(" ");
        let name = self
            .user_profile
            .as_ref()
            .and_then(|p| [&p.display_name, &p.real_name].into_iter().flatten().find(|n| !n.is_empty()).cloned());
        InboxItem {
            id: InboxItem::make_id(account_id, format!("{}/thread/{}", thread.channel.id, thread.thread_ts())),
            account_id: account_id.into(),
            plugin_id: "slack".into(),
            bundle: InboxBundle::mentions(),
            title: lines.first().map(|l| l.to_string()).unwrap_or_else(|| "(no text)".into()),
            context: format!("Thread in {}", thread.channel.label()),
            preview: (!preview.is_empty()).then_some(preview),
            url: thread.permalink.clone(),
            app_url: thread.app_url(team),
            author: Some(Person::named(&name.unwrap_or_else(|| "Someone".into()))),
            participants: vec![],
            badges: vec![],
            date: self.date(),
            needs_action: true,
            priority: None,
            due: None,
            expires: None,
            changes: None,
            suggested_people: None,
            recurrence: None,
            reactions: None,
            reason: None,
        }
    }
}

#[derive(Deserialize)]
struct Messages {
    matches: Vec<Match>,
}

#[derive(Deserialize)]
struct Channel {
    id: String,
    name: Option<String>,
    is_im: Option<bool>,
    is_mpim: Option<bool>,
}

impl Channel {
    fn is_direct(&self) -> bool {
        self.is_im == Some(true) || self.is_mpim == Some(true)
    }

    fn label(&self) -> String {
        if self.is_im == Some(true) {
            "Direct message".into()
        } else if self.is_mpim == Some(true) {
            "Group message".into()
        } else {
            format!("#{}", self.name.as_deref().unwrap_or(&self.id))
        }
    }
}

/// What apps put in a message besides (or instead of) `text`.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Attachment {
    fallback: Option<String>,
    pretext: Option<String>,
    title: Option<String>,
    text: Option<String>,
}

impl Attachment {
    fn words(&self) -> String {
        let parts: Vec<&str> = [&self.pretext, &self.title, &self.text]
            .into_iter()
            .flatten()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        if parts.is_empty() {
            self.fallback.clone().unwrap_or_default()
        } else {
            parts.join("\n")
        }
    }
}

#[derive(Deserialize)]
struct Match {
    ts: String,
    #[serde(default)]
    text: String,
    permalink: Option<String>,
    username: Option<String>,
    /// Who wrote it, by Slack id.
    #[serde(default)]
    user: Option<String>,
    channel: Channel,
    bot_id: Option<String>,
    subtype: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    attachments: Vec<Attachment>,
    #[serde(default)]
    blocks: serde_json::Value,
}

/// Attachments or reactions in a shape we don't expect are ignored rather than failing the whole search.
fn lenient<'de, D: serde::Deserializer<'de>, T: DeserializeOwned>(deserializer: D) -> Result<Vec<T>, D::Error> {
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(serde_json::from_value(value).unwrap_or_default())
}

/// Every string under a "text" key, at any depth: the words of Slack's Block Kit blocks.
fn text_leaves(value: &serde_json::Value, out: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, child) in map {
                match (key.as_str(), child) {
                    ("text", serde_json::Value::String(s)) => out.push(s.clone()),
                    _ => text_leaves(child, out),
                }
            }
        }
        serde_json::Value::Array(items) => items.iter().for_each(|i| text_leaves(i, out)),
        _ => {}
    }
}

impl Match {
    /// The thread this message is in: its permalink carries `thread_ts` for a reply; a thread's first message is
    /// its own root.
    fn thread_ts(&self) -> String {
        self.permalink
            .as_deref()
            .and_then(|link| reqwest::Url::parse(link).ok())
            .and_then(|url| url.query_pairs().find(|(k, _)| k == "thread_ts").map(|(_, v)| v.into_owned()))
            .unwrap_or_else(|| self.ts.clone())
    }

    fn date(&self) -> DateTime<Utc> {
        let seconds = self.ts.parse::<f64>().unwrap_or(0.0);
        Utc.timestamp_opt(seconds as i64, 0).single().unwrap_or_default()
    }

    /// Slack documents app links for channels and DMs, not single messages. The ids come from the server, so
    /// they're encoded as query values: an id can't add a parameter or change the link.
    fn app_url(&self, team: Option<&str>) -> Option<String> {
        let mut url = reqwest::Url::parse("slack://channel").ok()?;
        url.query_pairs_mut().append_pair("team", team?).append_pair("id", &self.channel.id);
        Some(url.to_string())
    }

    /// Apps (Google Calendar, Jira, GitHub…) post with a bot id, or with an empty `text` and their content in
    /// attachments or blocks. They inform; nobody waits for a reply.
    fn is_from_app(&self) -> bool {
        self.bot_id.is_some() || self.subtype.as_deref() == Some("bot_message") || self.text.trim().is_empty()
    }

    /// The message's words: its text, else what its attachments or blocks say.
    fn words(&self) -> String {
        if !self.text.trim().is_empty() {
            return self.text.clone();
        }
        let from_attachments: Vec<String> =
            self.attachments.iter().map(Attachment::words).filter(|w| !w.is_empty()).collect();
        if !from_attachments.is_empty() {
            return from_attachments.join("\n");
        }
        let mut leaves = vec![];
        text_leaves(&self.blocks, &mut leaves);
        leaves.join("\n")
    }

    fn item(&self, account_id: &str, bundle: InboxBundle, id: &str, me: &str, team: Option<&str>) -> InboxItem {
        let from_app = self.is_from_app();
        let plain = plain_text(&self.words(), me, &DateStyle::system());
        let lines: Vec<&str> = plain.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
        let preview = lines.iter().skip(1).copied().collect::<Vec<_>>().join(" ");
        InboxItem {
            id: InboxItem::make_id(account_id, id),
            account_id: account_id.into(),
            plugin_id: "slack".into(),
            bundle: if from_app { InboxBundle::read() } else { bundle },
            title: lines.first().map(|l| l.to_string()).unwrap_or_else(|| "(no text)".into()),
            context: self.channel.label(),
            preview: (!preview.is_empty()).then_some(preview),
            url: self.permalink.clone(),
            app_url: self.app_url(team),
            author: self.username.as_deref().map(Person::named),
            participants: vec![],
            badges: vec![],
            date: self.date(),
            needs_action: !from_app,
            priority: None,
            due: None,
            expires: self.expires(),
            changes: None,
            suggested_people: None,
            recurrence: None,
            reactions: None,
            reason: None,
        }
    }

    /// An app's message about a time to come (a calendar reminder) is over once that time comes: the first time
    /// it mentions after it was posted. People's messages never expire.
    fn expires(&self) -> Option<DateTime<Utc>> {
        if !self.is_from_app() {
            return None;
        }
        times(&self.words()).into_iter().filter(|t| *t > self.date()).min()
    }
}

/// How Slack's date tags read: in the system's language and time zone.
pub struct DateStyle {
    /// None: the system's time zone.
    pub offset: Option<FixedOffset>,
    pub french: bool,
    /// US English: 12-hour times and month-first dates.
    pub us: bool,
    /// What `{ago}` counts from.
    pub now: DateTime<Utc>,
}

impl DateStyle {
    pub fn system() -> Self {
        let locale = sys_locale::get_locale().unwrap_or_default().to_lowercase().replace('_', "-");
        DateStyle {
            offset: None,
            french: locale.starts_with("fr"),
            us: locale == "en-us" || locale == "en",
            now: Utc::now(),
        }
    }
}

static RULES: LazyLock<Vec<(Regex, &str)>> = LazyLock::new(|| {
    [
        (r"<@[A-Z0-9]+\|([^>]+)>", "@$1"),
        (r"<@([A-Z0-9]+)>", "@someone"),
        (r"<#[A-Z0-9]+\|([^>]*)>", "#$1"),
        (r"<!(here|channel|everyone)[^>]*>", "@$1"),
        (r"<!subteam\^[A-Z0-9]+\|([^>]+)>", "$1"),
        (r"<!subteam\^[A-Z0-9]+>", "@team"),
        (r"<!date\^[^>]*>", ""),
        (r"<(?:https?|mailto|tel):[^|>]+\|([^>]+)>", "$1"),
        (r"<(https?://[^>]+)>", "$1"),
        (r"<(?:mailto|tel):([^>]+)>", "$1"),
    ]
    .into_iter()
    .map(|(pattern, template)| (Regex::new(pattern).expect("valid pattern"), template))
    .collect()
});
/// `<!date^seconds^format|fallback>` and `<!date^seconds^format^link|fallback>`.
static DATE_TAG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<!date\^(\d+)\^([^^|>]*)(?:\^[^|>]*)?(?:\|([^>]*))?>").expect("valid pattern"));
static TOKEN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\{([a-z_]+)\}").expect("valid pattern"));

/// Slack mrkdwn to readable text: people, channels, links, emails and phone numbers by their label, dates in your
/// language and time zone (`<!date^1791555300^{time}|4:15 PM>` → 16:15), then `text::readable` (emoji, no marks).
/// Same rules on macOS (`SlackText`).
pub fn plain_text(text: &str, me: &str, style: &DateStyle) -> String {
    let result = text.replace(&format!("<@{me}>"), "@you");
    let mut result = DATE_TAG
        .replace_all(&result, |c: &regex::Captures| {
            let fallback = c.get(3).map(|m| m.as_str());
            c[1].parse::<i64>().ok().and_then(|s| Utc.timestamp_opt(s, 0).single()).map_or_else(
                || fallback.unwrap_or_default().to_string(),
                |date| formatted(date, &c[2], fallback, style),
            )
        })
        .into_owned();
    for (pattern, template) in RULES.iter() {
        result = pattern.replace_all(&result, *template).into_owned();
    }
    remora_core::text::readable(&result.replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&"))
}

/// The times a message's date tags point at.
pub fn times(text: &str) -> Vec<DateTime<Utc>> {
    DATE_TAG
        .captures_iter(text)
        .filter_map(|c| c[1].parse::<i64>().ok())
        .filter_map(|s| Utc.timestamp_opt(s, 0).single())
        .collect()
}

/// Slack's tokens ({time}, {date_short}, {date_pretty}, {ago}…) in place; an unknown one gives the fallback.
pub fn formatted(date: DateTime<Utc>, format: &str, fallback: Option<&str>, style: &DateStyle) -> String {
    let mut unknown = false;
    let result = TOKEN.replace_all(format, |c: &regex::Captures| {
        token(&c[1], date, style).unwrap_or_else(|| {
            unknown = true;
            String::new()
        })
    });
    if unknown {
        fallback.unwrap_or_default().to_string()
    } else {
        result.into_owned()
    }
}

const MONTHS_FR: [&str; 12] = [
    "janvier",
    "février",
    "mars",
    "avril",
    "mai",
    "juin",
    "juillet",
    "août",
    "septembre",
    "octobre",
    "novembre",
    "décembre",
];
const MONTHS_FR_SHORT: [&str; 12] =
    ["janv.", "févr.", "mars", "avr.", "mai", "juin", "juil.", "août", "sept.", "oct.", "nov.", "déc."];
const DAYS_FR: [&str; 7] = ["lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi", "dimanche"];

fn token(name: &str, date: DateTime<Utc>, style: &DateStyle) -> Option<String> {
    let local = |d: DateTime<Utc>| d.with_timezone(&style.offset.unwrap_or_else(|| *Local::now().offset()));
    let at = local(date);
    let (month, day, year) = (at.month0() as usize, at.day(), at.year());
    let date_text = |long: bool, weekday: bool| {
        let base = match (style.french, style.us, long) {
            (true, _, true) => format!("{day} {} {year}", MONTHS_FR[month]),
            (true, _, false) => format!("{day} {} {year}", MONTHS_FR_SHORT[month]),
            (false, true, true) => at.format("%B %-d, %Y").to_string(),
            (false, true, false) => at.format("%b %-d, %Y").to_string(),
            (false, false, true) => at.format("%-d %B %Y").to_string(),
            (false, false, false) => at.format("%-d %b %Y").to_string(),
        };
        match (weekday, style.french) {
            (false, _) => base,
            (true, true) => format!("{} {base}", DAYS_FR[at.weekday().num_days_from_monday() as usize]),
            (true, false) => format!("{}, {base}", at.format("%A")),
        }
    };
    let pretty = |fallback: String| {
        let days = (at.date_naive() - local(style.now).date_naive()).num_days();
        match (days, style.french) {
            (0, false) => "Today".into(),
            (1, false) => "Tomorrow".into(),
            (-1, false) => "Yesterday".into(),
            (0, true) => "aujourd’hui".into(),
            (1, true) => "demain".into(),
            (-1, true) => "hier".into(),
            _ => fallback,
        }
    };
    Some(match name {
        "time" if style.us => at.format("%-I:%M %p").to_string(),
        "time" => at.format("%H:%M").to_string(),
        "time_secs" if style.us => at.format("%-I:%M:%S %p").to_string(),
        "time_secs" => at.format("%H:%M:%S").to_string(),
        "date_num" => at.format("%Y-%m-%d").to_string(),
        "date" => date_text(true, false),
        "date_short" => date_text(false, false),
        "date_long" => date_text(true, true),
        "date_pretty" => pretty(date_text(true, false)),
        "date_short_pretty" => pretty(date_text(false, false)),
        "date_long_pretty" => pretty(date_text(true, true)),
        "ago" => ago(style.now - date, style.french),
        _ => return None,
    })
}

/// "2 hours ago", "in 5 minutes"; "il y a 2 heures", "dans 5 minutes".
fn ago(elapsed: Duration, french: bool) -> String {
    let seconds = elapsed.num_seconds();
    let n = seconds.abs();
    let (count, unit_en, unit_fr) = match n {
        0..60 => return if french { "maintenant".into() } else { "now".into() },
        60..3_600 => (n / 60, "minute", "minute"),
        3_600..86_400 => (n / 3_600, "hour", "heure"),
        _ => (n / 86_400, "day", "jour"),
    };
    let plural = if count > 1 { "s" } else { "" };
    match (french, seconds > 0) {
        (false, true) => format!("{count} {unit_en}{plural} ago"),
        (false, false) => format!("in {count} {unit_en}{plural}"),
        (true, true) => format!("il y a {count} {unit_fr}{plural}"),
        (true, false) => format!("dans {count} {unit_fr}{plural}"),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::stub::*;

    #[test]
    fn cleans_mrkdwn() {
        let text = "<@UME> see <https://x.io|the doc> &amp; ping <@U2|erin> in <#C1|general> <!here>";
        assert_eq!(plain_text(text, "UME", &paris()), "@you see the doc & ping @erin in #general @here");
    }

    /// Ids from the server can't add parameters to the app link.
    #[test]
    fn app_links_encode_the_ids() {
        let message: Match =
            serde_json::from_str(r#"{"ts": "1.0", "channel": {"id": "D1&team=EVIL#x", "is_im": true}}"#).unwrap();
        let url = message.app_url(Some("T42")).unwrap();
        assert_eq!(url, "slack://channel?team=T42&id=D1%26team%3DEVIL%23x");
        assert!(crate::links::is_app_link(&url));
    }

    /// The event in the screenshot: 16:15 to 17:15 in Paris on 9 October 2026.
    fn start() -> DateTime<Utc> {
        Utc.timestamp_opt(1_791_555_300, 0).unwrap()
    }

    fn paris() -> DateStyle {
        DateStyle { offset: FixedOffset::east_opt(2 * 3_600), french: false, us: false, now: start() }
    }

    /// A Google Calendar reminder, as Slack sends it. Same case as macOS.
    #[test]
    fn calendar_reminder_reads_in_your_language_and_time_zone() {
        let text = ":loudspeaker: _1 minute until this event:_\n<!date^1791555300^{time}|4:15 PM> - <!date^1791558900^{time}|5:15 PM> \
                    Staging Movidone *Guests:* You _(organizer)_, <mailto:lou@acme.io|Lou Martin>";
        let french = DateStyle { french: true, ..paris() };
        assert_eq!(
            plain_text(text, "UME", &french),
            "📢 1 minute until this event:\n16:15 - 17:15 Staging Movidone Guests: You (organizer), Lou Martin"
        );
        let us = DateStyle { offset: FixedOffset::west_opt(4 * 3_600), us: true, ..paris() };
        assert!(plain_text(text, "UME", &us).contains("10:15 AM - 11:15 AM"));
    }

    #[test]
    fn date_tokens() {
        let style = DateStyle { now: start() + Duration::hours(2), ..paris() };
        let format = |f: &str| formatted(start(), f, Some("fallback"), &style);
        assert_eq!(format("{date_num} {time}"), "2026-10-09 16:15");
        assert_eq!(format("{date_short}"), "9 Oct 2026");
        assert!(format("{date_long}").starts_with("Friday"));
        assert_eq!(format("{date_pretty}"), "Today");
        assert_eq!(format("{ago}"), "2 hours ago");
        assert_eq!(format("on {nonsense}"), "fallback", "a token Slack may add later");
        assert_eq!(
            formatted(start(), "{date_long}", None, &DateStyle { french: true, ..paris() }),
            "vendredi 9 octobre 2026"
        );
        assert_eq!(plain_text("<!date^1791555300^{date_num}^https://x.io|x>", "", &style), "2026-10-09", "with a link");
        assert_eq!(
            plain_text("<tel:+33100000000|Call Lou>, <!subteam^S1|@design>, <mailto:a@b.io>", "", &style),
            "Call Lou, @design, a@b.io"
        );
    }

    /// An app's reminder is over once the event starts; a person's message never expires.
    #[tokio::test]
    async fn app_reminders_expire_when_the_event_starts() {
        let search = r#"{"ok": true, "messages": {"matches": [
          {"ts": "1791555240.0", "text": "_1 minute until this event:_ <!date^1791555300^{time}|4:15 PM> - <!date^1791558900^{time}|5:15 PM>",
           "username": "Google Calendar", "bot_id": "B1", "channel": {"id": "D7", "is_im": true}},
          {"ts": "1791555000.0", "text": "Shall we meet at <!date^1791555300^{time}|4:15 PM>?", "username": "carol", "channel": {"id": "D1", "is_im": true}}
        ]}}"#;
        let http = StubHttp::paths(&[
            ("/auth.test", r#"{"ok": true, "user_id": "UME", "user": "alice", "team": "Acme"}"#),
            ("/search.messages", search),
        ]);
        let items =
            SlackPlugin::new(&config(&[("token", "xoxp")]), Arc::new(http)).unwrap().fetch().await.unwrap().items;
        let calendar = items.iter().find(|i| i.author.as_ref().is_some_and(|a| a.name == "Google Calendar")).unwrap();
        assert_eq!(calendar.expires, Some(start()));
        assert_eq!(items.iter().find(|i| i.author.as_ref().is_some_and(|a| a.name == "carol")).unwrap().expires, None);

        let assembler = remora_core::InboxAssembler { wake_on_activity: true };
        assert_eq!(assembler.placement(calendar, None, start() - Duration::seconds(30)), remora_core::Placement::Inbox);
        assert_eq!(assembler.placement(calendar, None, start()), remora_core::Placement::Cleared);
        let pinned = remora_core::ItemState { pinned: true, ..Default::default() };
        assert_eq!(
            assembler.placement(calendar, Some(&pinned), start()),
            remora_core::Placement::Inbox,
            "pinned stays"
        );
    }

    #[tokio::test]
    async fn keeps_the_latest_message_per_conversation_with_app_links() {
        let search = r#"{"ok": true, "messages": {"matches": [
          {"ts": "1700000100.0", "text": "second", "username": "carol", "channel": {"id": "D1", "is_im": true}},
          {"ts": "1700000000.0", "text": "first", "username": "carol", "channel": {"id": "D1", "is_im": true}},
          {"ts": "1700000050.0", "text": "hey <@UME>\nmore", "username": "erin", "channel": {"id": "C1", "name": "dev"}}
        ]}}"#;
        let http = StubHttp::paths(&[
            ("/auth.test", r#"{"ok": true, "user_id": "UME", "user": "alice", "team": "Acme", "team_id": "T42"}"#),
            ("/search.messages", search),
        ]);
        let snapshot = SlackPlugin::new(&config(&[("token", "xoxp")]), Arc::new(http)).unwrap().fetch().await.unwrap();

        let direct: Vec<&InboxItem> =
            snapshot.items.iter().filter(|i| i.bundle == InboxBundle::direct_messages()).collect();
        assert_eq!(direct.iter().map(|i| i.title.as_str()).collect::<Vec<_>>(), ["second"]);
        assert_eq!(direct[0].app_url.as_deref(), Some("slack://channel?team=T42&id=D1"));
        let mention = snapshot.items.iter().find(|i| i.bundle == InboxBundle::mentions()).unwrap();
        assert_eq!(
            (mention.title.as_str(), mention.preview.as_deref(), mention.context.as_str()),
            ("hey @you", Some("more"), "#dev")
        );
    }

    /// Google Calendar's app DMs event updates with an empty `text`: the words are in attachments or blocks.
    #[tokio::test]
    async fn app_messages_are_readable_and_to_read() {
        let search = r#"{"ok": true, "messages": {"matches": [
          {"ts": "1700000300.0", "text": "", "username": "Google Calendar", "bot_id": "B1", "channel": {"id": "D7", "is_im": true},
           "attachments": [{"fallback": "Event updated", "pretext": "Event updated", "title": "Design review", "text": "Tomorrow 10:00"}]},
          {"ts": "1700000200.0", "text": "", "username": "Jira Cloud", "channel": {"id": "D8", "is_im": true},
           "blocks": [{"type": "section", "text": {"type": "mrkdwn", "text": "*PAY-12* moved to Done"}},
                      {"type": "context", "elements": [{"type": "mrkdwn", "text": "by Erin"}, {"type": "image", "image_url": "x"}]}]},
          {"ts": "1700000100.0", "username": "Mystery", "channel": {"id": "D9", "is_im": true}, "attachments": "odd"},
          {"ts": "1700000000.0", "text": "can you look?", "username": "carol", "channel": {"id": "D1", "is_im": true}}
        ]}}"#;
        let http = StubHttp::paths(&[
            ("/auth.test", r#"{"ok": true, "user_id": "UME", "user": "alice", "team": "Acme"}"#),
            ("/search.messages", search),
        ]);
        let items: Vec<InboxItem> = SlackPlugin::new(&config(&[("token", "xoxp")]), Arc::new(http))
            .unwrap()
            .fetch()
            .await
            .unwrap()
            .items
            .into_iter()
            .map(remora_core::VerbClassifier::classify)
            .collect();
        let by = |name: &str| items.iter().find(|i| i.author.as_ref().is_some_and(|a| a.name == name)).unwrap();
        let calendar = by("Google Calendar");
        assert_eq!(
            (calendar.title.as_str(), calendar.preview.as_deref()),
            ("Event updated", Some("Design review Tomorrow 10:00"))
        );
        assert_eq!(calendar.bundle, InboxBundle::read());
        assert!(!calendar.needs_action);
        let jira = by("Jira Cloud");
        assert_eq!((jira.title.as_str(), jira.preview.as_deref()), ("PAY-12 moved to Done", Some("by Erin")));
        assert_eq!(jira.bundle, InboxBundle::read());
        let empty = by("Mystery");
        assert_eq!((empty.title.as_str(), &empty.bundle), ("(no text)", &InboxBundle::read()));
        let person = by("carol");
        assert_eq!(person.bundle, InboxBundle::reply());
        assert!(person.needs_action);
    }

    #[tokio::test]
    async fn reports_slack_errors() {
        // Every way Slack says the token no longer works asks for reconnecting, as on macOS.
        for error in REJECTED_TOKEN {
            let answer = format!(r#"{{"ok": false, "error": "{error}"}}"#);
            let http = StubHttp::paths(&[("/auth.test", &answer)]);
            let result = SlackPlugin::new(&config(&[("token", "bad")]), Arc::new(http)).unwrap().fetch().await;
            assert_eq!(result.err(), Some(PluginError::Unauthorized), "{error}");
        }
        let http = StubHttp::paths(&[("/auth.test", r#"{"ok": false, "error": "ratelimited_by_admin"}"#)]);
        let result = SlackPlugin::new(&config(&[("token", "t")]), Arc::new(http)).unwrap().fetch().await;
        assert_eq!(result.err(), Some(PluginError::Api("Slack: ratelimited_by_admin".into())));
    }

    #[test]
    fn app_manifest_link_requests_search_scope() {
        let url = reqwest::Url::parse(&app_manifest_url().unwrap()).unwrap();
        let manifest = url.query_pairs().find(|(k, _)| k == "manifest_json").map(|(_, v)| v.into_owned()).unwrap();
        let json: serde_json::Value = serde_json::from_str(&manifest).unwrap();
        assert_eq!(
            json["oauth_config"]["scopes"]["user"],
            serde_json::json!(["search:read", "channels:history", "groups:history", "reactions:read", "emoji:read"])
        );
    }

    /// Answers Slack by method, and the searches by their query: your own thread messages, or nothing.
    /// Answers Slack by method, and the searches by their query: your own thread messages, mentions of you, or
    /// nothing. Reactions: what `reactions.list` and `emoji.list` say, and `reactions.get` by message.
    #[derive(Default)]
    struct SlackStub {
        mine: &'static str,
        replies: &'static str,
        asked: std::sync::Mutex<Vec<String>>,
        mentions: Option<String>,
        reacted: Option<&'static str>,
        got: HashMap<&'static str, &'static str>,
        emoji: Option<&'static str>,
        direct: Option<String>,
        sent: Option<String>,
        methods: std::sync::Mutex<Vec<String>>,
    }

    impl SlackStub {
        fn calls(&self, method: &str) -> usize {
            self.methods.lock().unwrap().iter().filter(|m| *m == method).count()
        }
    }

    #[async_trait]
    impl HttpClient for SlackStub {
        async fn send(&self, request: Request) -> Result<crate::Response, PluginError> {
            let url = reqwest::Url::parse(&request.url).unwrap();
            let value = |name: &str| {
                url.query_pairs().find(|(k, _)| k == name).map(|(_, v)| v.into_owned()).unwrap_or_default()
            };
            let method = url.path().rsplit('/').next().unwrap_or_default().to_string();
            self.methods.lock().unwrap().push(method.clone());
            let none = r#"{"ok": true, "messages": {"matches": []}}"#.to_string();
            let body = match method.as_str() {
                "auth.test" => {
                    r#"{"ok": true, "user_id": "UME", "user": "alice", "team": "Acme", "team_id": "T1"}"#.to_string()
                }
                "conversations.replies" => {
                    self.asked.lock().unwrap().push(format!(
                        "{} {} {}",
                        value("channel"),
                        value("ts"),
                        value("oldest")
                    ));
                    self.replies.to_string()
                }
                "reactions.list" => self.reacted.unwrap_or(r#"{"ok": true, "items": []}"#).to_string(),
                "reactions.get" => {
                    let key = format!("{}/{}", value("channel"), value("timestamp"));
                    self.got.get(key.as_str()).unwrap_or(&r#"{"ok": true, "message": {}}"#).to_string()
                }
                "emoji.list" => self.emoji.unwrap_or(r#"{"ok": true, "emoji": {}}"#).to_string(),
                _ if value("query").contains("is:thread") => self.mine.to_string(),
                _ if value("query").starts_with("<@UME>") => self.mentions.clone().unwrap_or(none),
                _ if value("query").starts_with("to:me") => self.direct.clone().unwrap_or(none),
                _ if value("query").starts_with("from:<@UME>") => self.sent.clone().unwrap_or(none),
                _ => none,
            };
            Ok(crate::Response { status: 200, body, etag: None, reset: None })
        }
    }

    /// A reply in a thread you wrote in, without a mention, reaches the inbox; one that mentions you is
    /// already a mention, and your own or older replies don't count. Same cases as macOS.
    #[tokio::test]
    async fn replies_in_your_threads() {
        let mine = r#"{"ok": true, "messages": {"matches": [
          {"ts": "1700000200.0", "text": "I can take it", "channel": {"id": "C1", "name": "design"},
           "permalink": "https://acme.slack.com/archives/C1/p1700000200?thread_ts=1700000100.0"},
          {"ts": "1700000050.0", "text": "older one", "channel": {"id": "C1", "name": "design"},
           "permalink": "https://acme.slack.com/archives/C1/p1700000050?thread_ts=1700000100.0"},
          {"ts": "1700000300.0", "text": "in a DM", "channel": {"id": "D1", "is_im": true}}
        ]}}"#;
        let replies = r#"{"ok": true, "messages": [
          {"ts": "1700000100.0", "text": "Who can review the mockups?", "user": "U2"},
          {"ts": "1700000200.0", "text": "I can take it", "user": "UME"},
          {"ts": "1700000400.0", "text": "Great, *thanks*! Tomorrow then", "user": "U2", "user_profile": {"display_name": "erin"}},
          {"ts": "1700000500.0", "text": "<@UME> also this", "user": "U3"}
        ]}"#;
        let http = Arc::new(SlackStub { mine, replies, ..Default::default() });
        let items = SlackPlugin::new(&config(&[("token", "xoxp")]), http.clone()).unwrap().fetch().await.unwrap().items;
        let thread = items.iter().find(|i| i.context.starts_with("Thread")).unwrap();
        assert_eq!(thread.title, "Great, thanks! Tomorrow then");
        assert_eq!(thread.author.as_ref().unwrap().name, "erin");
        assert_eq!(thread.context, "Thread in #design");
        assert!(thread.id.ends_with("/C1/thread/1700000100.0"));
        assert!(thread.bundle == InboxBundle::mentions() && thread.needs_action);
        assert_eq!(
            *http.asked.lock().unwrap(),
            ["C1 1700000100.0 1700000200.0"],
            "one call, from your last message, no DM"
        );
    }

    /// Slack sends the thread's first message whatever `oldest` says; it isn't a reply to you.
    #[tokio::test]
    async fn the_first_message_of_a_thread_is_not_a_reply() {
        let mine = r#"{"ok": true, "messages": {"matches": [
          {"ts": "1700000200.0", "text": "I can take it", "channel": {"id": "C1", "name": "design"},
           "permalink": "https://acme.slack.com/archives/C1/p1700000200?thread_ts=1700000100.0"}
        ]}}"#;
        let replies = r#"{"ok": true, "messages": [
          {"ts": "1700000100.0", "text": "Who can review the mockups?", "user": "U2"},
          {"ts": "1700000200.0", "text": "I can take it", "user": "UME"}
        ]}"#;
        let http = Arc::new(SlackStub { mine, replies, ..Default::default() });
        let items = SlackPlugin::new(&config(&[("token", "xoxp")]), http).unwrap().fetch().await.unwrap().items;
        let thread = items.iter().find(|i| i.context.starts_with("Thread")).unwrap();
        assert!(
            thread.bundle == InboxBundle::answered() && !thread.needs_action,
            "the ball is in their court: your answer stands"
        );
    }

    /// Your last message in a thread, as Slack returns it with `inclusive`: the reactions on it decide.
    const YOUR_THREAD: &str = r#"{"ok": true, "messages": {"matches": [
      {"ts": "1700000200.0", "text": "Can someone review the mockups?", "channel": {"id": "C1", "name": "design"},
       "permalink": "https://acme.slack.com/archives/C1/p1700000200?thread_ts=1700000100.0"}
    ]}}"#;

    fn replies_with(reactions: &str) -> &'static str {
        Box::leak(
            format!(
                r#"{{"ok": true, "messages": [
              {{"ts": "1700000100.0", "text": "Mockups are up", "user": "U2"}},
              {{"ts": "1700000200.0", "text": "Can someone review the mockups?", "user": "UME", "reactions": {reactions}}}
            ]}}"#
            )
            .into_boxed_str(),
        )
    }

    /// The fetch `registry`'s egress test runs through the guard.
    pub(crate) fn egress_fixture() -> EgressFixture {
        EgressFixture {
            settings: vec![("token", "xoxp")],
            http: Arc::new(SlackStub { mine: YOUR_THREAD, replies: replies_with("[]"), ..Default::default() }),
        }
    }

    async fn thread(reactions: &str, emoji: Option<&'static str>) -> Option<InboxItem> {
        let http =
            Arc::new(SlackStub { mine: YOUR_THREAD, replies: replies_with(reactions), emoji, ..Default::default() });
        let items = SlackPlugin::new(&config(&[("token", "xoxp")]), http).unwrap().fetch().await.unwrap().items;
        items.into_iter().find(|i| i.id.contains("/thread/"))
    }

    /// Nobody wrote back: their reactions on your message speak for them. Same cases as macOS.
    #[tokio::test]
    async fn their_reactions_on_your_message() {
        let waiting = thread("[]", None).await.unwrap();
        assert!(waiting.bundle == InboxBundle::answered() && !waiting.needs_action, "someone else started it");
        assert_eq!(waiting.title, "Can someone review the mockups?");
        assert!(waiting.id.ends_with("/C1/thread/1700000100.0"), "the same item as a reply would be");

        let seen = thread(r#"[{"name": "+1::skin-tone-2", "users": ["U2"], "count": 1}]"#, None).await.unwrap();
        assert!(seen.bundle == InboxBundle::answered() && seen.others_reacted(ReactionMeaning::Ack));

        let question = thread(r#"[{"name": "thinking_face", "users": ["U2", "U3"], "count": 2}]"#, None).await.unwrap();
        assert!(question.bundle == InboxBundle::reply() && question.needs_action);
        assert_eq!(question.context, "🤔 on your message in #design");
        assert_eq!(question.title, "Can someone review the mockups?");

        let closed = thread(r#"[{"name": "white_check_mark", "users": ["U2"], "count": 1}]"#, None).await.unwrap();
        assert_eq!(closed.bundle, InboxBundle::finished(), "their ✅: finished, in Done");
        let own = thread(r#"[{"name": "white_check_mark", "users": ["UME"], "count": 1}]"#, None).await.unwrap();
        assert_eq!(own.bundle, InboxBundle::answered(), "your own ✅ answers nothing");
    }

    /// A reply keeps its reactions; your ✅ on it is what moves it to Done.
    #[tokio::test]
    async fn a_reply_carries_its_reactions() {
        let replies = r#"{"ok": true, "messages": [
          {"ts": "1700000200.0", "text": "Can someone review the mockups?", "user": "UME"},
          {"ts": "1700000400.0", "text": "On it tomorrow", "user": "U2",
           "reactions": [{"name": "white_check_mark", "users": ["UME"], "count": 1}]}
        ]}"#;
        let http = Arc::new(SlackStub { mine: YOUR_THREAD, replies, ..Default::default() });
        let items = SlackPlugin::new(&config(&[("token", "xoxp")]), http).unwrap().fetch().await.unwrap().items;
        let reply = items.iter().find(|i| i.context.starts_with("Thread")).unwrap();
        assert_eq!(reply.title, "On it tomorrow");
        assert_eq!(reply.your_reaction(), Some(ReactionMeaning::Done));
    }

    fn mention(ts: &str) -> String {
        format!(
            r#"{{"ts": "{ts}", "text": "<@UME> can you look?", "username": "erin", "channel": {{"id": "C9", "name": "ops"}}}}"#
        )
    }

    /// Search returns no reactions: the messages you reacted to come in one request, and the newest that need you
    /// are asked about one by one, at most 15, each kept 10 minutes.
    #[tokio::test]
    async fn reactions_on_mentions() {
        let matches: Vec<String> = (1..=20).map(|n| mention(&format!("17000000{}.0", n + 10))).collect();
        let reacted = r#"{"ok": true, "items": [
          {"type": "message", "channel": "C9", "message": {"ts": "1700000011.0",
           "reactions": [{"name": "white_check_mark", "users": ["UME", "U2"], "count": 2}]}},
          {"type": "file", "file": {"id": "F1"}}
        ]}"#;
        let http = Arc::new(SlackStub {
            mine: r#"{"ok": true, "messages": {"matches": []}}"#,
            replies: "{}",
            mentions: Some(format!(r#"{{"ok": true, "messages": {{"matches": [{}]}}}}"#, matches.join(","))),
            reacted: Some(reacted),
            got: HashMap::from([(
                "C9/1700000030.0",
                r#"{"ok": true, "message": {"reactions": [{"name": "eyes", "users": ["U4"], "count": 1}]}}"#,
            )]),
            ..Default::default()
        });
        let plugin = SlackPlugin::new(&config(&[("token", "xoxp")]), http.clone()).unwrap();
        let items = plugin.fetch().await.unwrap().items;
        let handled = items.iter().find(|i| i.id.ends_with("C9/1700000011.0")).unwrap();
        assert_eq!(handled.your_reaction(), Some(ReactionMeaning::Done));
        assert_eq!(handled.reactions.as_ref().unwrap()[0].count, 2);
        let looked = items.iter().find(|i| i.id.ends_with("C9/1700000030.0")).unwrap();
        assert!(looked.your_reaction().is_none() && looked.others_reacted(ReactionMeaning::Started));
        assert_eq!(http.calls("reactions.get"), REACTIONS_ASKED);
        assert_eq!(http.calls("reactions.list"), 1);

        plugin.fetch().await.unwrap();
        assert_eq!(http.calls("reactions.get"), REACTIONS_ASKED, "kept");
    }

    #[tokio::test]
    async fn reactions_without_their_permission_say_so() {
        let http = Arc::new(SlackStub {
            mine: r#"{"ok": true, "messages": {"matches": []}}"#,
            replies: "{}",
            mentions: Some(format!(r#"{{"ok": true, "messages": {{"matches": [{}]}}}}"#, mention("1700000011.0"))),
            reacted: Some(r#"{"ok": false, "error": "missing_scope"}"#),
            ..Default::default()
        });
        let snapshot = SlackPlugin::new(&config(&[("token", "xoxp")]), http.clone()).unwrap().fetch().await.unwrap();
        assert!(snapshot.remarks.len() == 1 && snapshot.remarks[0].text.contains("reactions:read"));
        assert_eq!(snapshot.items.len(), 1, "everything else works");
        assert_eq!(http.calls("reactions.get"), 0);
    }

    /// A workspace's own emoji: an alias is the emoji it stands for; another has its picture and a meaning from
    /// its name.
    #[tokio::test]
    async fn workspace_emoji() {
        let picture = "https://emoji.slack-edge.com/T1/approved-by-design/abc.png";
        let emoji = r#"{"ok": true, "emoji": {"yes": "alias:white_check_mark",
          "approved-by-design": "https://emoji.slack-edge.com/T1/approved-by-design/abc.png"}}"#;
        assert!(
            thread(r#"[{"name": "yes", "users": ["U2"], "count": 1}]"#, Some(emoji)).await.unwrap().bundle
                == InboxBundle::finished(),
            "✅ by alias"
        );

        let custom = HashMap::from([("approved-by-design".to_string(), picture.to_string())]);
        let resolved = resolve(Reaction::new("approved-by-design", 1, false), &custom);
        assert_eq!(resolved.image.as_deref(), Some(picture));
        assert_eq!(resolved.meaning(), Some(ReactionMeaning::Done));
        assert_eq!(resolved.emoji, ":approved-by-design:");
        assert_eq!(resolve(Reaction::new("yes", 1, false), &HashMap::new()).meaning(), Some(ReactionMeaning::Ack));
    }

    fn matches(messages: &[String]) -> String {
        format!(r#"{{"ok": true, "messages": {{"matches": [{}]}}}}"#, messages.join(","))
    }

    fn dm(ts: &str, text: &str, user: &str) -> String {
        format!(
            r#"{{"ts": "{ts}", "text": "{text}", "user": "{user}", "username": "{user}", "channel": {{"id": "D1", "is_im": true}}}}"#
        )
    }

    async fn items(mentions: Option<String>, direct: Option<String>, sent: Option<String>) -> Vec<InboxItem> {
        let none = r#"{"ok": true, "messages": {"matches": []}}"#;
        let http = Arc::new(SlackStub { mine: none, replies: "{}", mentions, direct, sent, ..Default::default() });
        SlackPlugin::new(&config(&[("token", "xoxp")]), http).unwrap().fetch().await.unwrap().items
    }

    /// Whose turn a direct conversation is: theirs to answer when they wrote last, else your message stands. Same
    /// cases as macOS.
    #[tokio::test]
    async fn who_wrote_last_in_a_direct_conversation() {
        let question = dm("1700000100.0", "Can you send the deck?", "U2");
        let answer = dm("1700000200.0", "Here it is", "UME");
        let earlier = dm("1700000050.0", "Hi!", "UME");

        let theirs =
            items(None, Some(matches(std::slice::from_ref(&question))), Some(matches(std::slice::from_ref(&earlier))))
                .await;
        assert!(theirs.len() == 1 && theirs[0].bundle == InboxBundle::direct_messages());
        assert_eq!(theirs[0].title, "Can you send the deck?");

        let answered = items(None, Some(matches(&[question])), Some(matches(&[answer, earlier]))).await;
        assert!(answered.len() == 1 && answered[0].bundle == InboxBundle::answered());
        assert_eq!(answered[0].title, "Here it is");
        assert!(answered[0].id.ends_with("/D1") && !answered[0].needs_action && answered[0].expires.is_none());

        let asked = items(None, None, Some(matches(&[dm("1700000300.0", "Free for lunch?", "UME")]))).await;
        assert!(asked.len() == 1 && asked[0].bundle == InboxBundle::asked(), "they never wrote: your question waits");
    }

    fn in_channel(ts: &str, text: &str, user: &str, thread: Option<&str>) -> String {
        let link = thread
            .map(|t| format!(r#", "permalink": "https://acme.slack.com/archives/C9/p1?thread_ts={t}""#))
            .unwrap_or_default();
        format!(
            r#"{{"ts": "{ts}", "text": "{text}", "user": "{user}", "username": "{user}", "channel": {{"id": "C9", "name": "ops"}}{link}}}"#
        )
    }

    /// A mention is answered by your later message in its thread (which the thread's item then follows), or in the
    /// channel when it names them or comes soon after; a later message of yours about something else doesn't count.
    #[tokio::test]
    async fn when_a_mention_is_answered() {
        let mention = || Some(matches(&[in_channel("1700000000.0", "<@UME> can you check the logs?", "U2", None)]));
        let named = in_channel("1700020000.0", "<@U2> logs look fine", "UME", None);
        let soon = in_channel("1700001000.0", "Looking, give me a sec", "UME", None);
        let unrelated = in_channel("1700020000.0", "Deploying the web app now", "UME", None);
        let in_thread = in_channel("1700000500.0", "On it", "UME", Some("1700000000.0"));

        assert_eq!(items(mention(), None, Some(matches(&[unrelated]))).await[0].bundle, InboxBundle::mentions());
        let answer = &items(mention(), None, Some(matches(&[named]))).await[0];
        assert!(answer.bundle == InboxBundle::answered() && answer.title.ends_with("logs look fine"));
        assert_eq!(items(mention(), None, Some(matches(&[soon]))).await[0].bundle, InboxBundle::answered());
        assert_eq!(items(mention(), None, Some(matches(&[in_thread]))).await[0].bundle, InboxBundle::finished());
    }

    #[tokio::test]
    async fn threads_without_the_history_scope_say_so() {
        let mine = r#"{"ok": true, "messages": {"matches": [{"ts": "1.0", "text": "x", "channel": {"id": "C1", "name": "a"}}]}}"#;
        let http =
            Arc::new(SlackStub { mine, replies: r#"{"ok": false, "error": "missing_scope"}"#, ..Default::default() });
        let snapshot = SlackPlugin::new(&config(&[("token", "xoxp")]), http).unwrap().fetch().await.unwrap();
        assert!(snapshot.remarks.len() == 1 && snapshot.remarks[0].text.contains("channels:history"));
        assert!(!snapshot.items.iter().any(|i| i.context.starts_with("Thread")));
    }

    /// Who the token belongs to is asked once across refreshes, as GitLab's.
    #[tokio::test]
    async fn the_identity_is_asked_once_across_refreshes() {
        let stub = StubHttp::paths(&[
            ("/auth.test", r#"{"ok": true, "user_id": "UME", "user": "alice", "team": "Acme"}"#),
            ("/search.messages", r#"{"ok": true, "messages": {"matches": []}}"#),
        ]);
        let http = Arc::new(Recording::new(stub));
        let config = config(&[("token", "xoxp")]);
        for _ in 0..2 {
            SlackPlugin::new(&config, http.clone()).unwrap().fetch().await.unwrap();
        }
        assert_eq!(http.paths().iter().filter(|p| p.ends_with("/auth.test")).count(), 1);
    }
}

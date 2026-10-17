use crate::linear::due_badge;
use crate::{
    decode, oauth, url_with_query, AccountCache, HttpClient, PluginConfig, PluginError, Request, SourcePlugin,
    SourceSnapshot, IDENTITY_LIFETIME,
};
use async_trait::async_trait;
use chrono::{DateTime, Duration, Local, NaiveDateTime, Utc};
use regex::Regex;
use remora_core::badge;
use remora_core::{Badge, ConfigField, Egress, InboxBundle, InboxItem, Person, PluginManifest, Priority, SignIn, Tone};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::sync::{Arc, LazyLock};

const GRAPH: &str = "https://graph.microsoft.com/v1.0";

/// What Remora asks Microsoft for: read-only, and `offline_access` for the refresh token.
pub const SCOPES: [&str; 5] = ["offline_access", "User.Read", "Mail.Read", "Calendars.Read", "Chat.Read"];

pub fn sign_in() -> SignIn {
    SignIn {
        authorize_url: "https://login.microsoftonline.com/{tenant}/oauth2/v2.0/authorize".into(),
        token_url: "https://login.microsoftonline.com/{tenant}/oauth2/v2.0/token".into(),
        client_id_key: "clientId".into(),
        scopes: SCOPES.iter().map(|s| s.to_string()).collect(),
    }
}

pub fn manifest() -> PluginManifest {
    PluginManifest {
        id: "microsoft".into(),
        name: "Microsoft 365".into(),
        summary: "Preview: flagged mail and unread mail sent to you, invitations you haven’t answered, and unread Teams chats."
            .into(),
        fields: vec![
            ConfigField {
                key: "tenant".into(),
                label: "Tenant".into(),
                placeholder: "your-company.onmicrosoft.com".into(),
                default_value: String::new(),
                is_secret: false,
                is_optional: false,
                help: Some("Your organization’s domain or tenant ID.".into()),
                managed_key: Some("MicrosoftTenant".into()),
                is_toggle: false,
            },
            ConfigField {
                key: "clientId".into(),
                label: "Client ID".into(),
                placeholder: "00000000-0000-0000-0000-000000000000".into(),
                default_value: String::new(),
                is_secret: false,
                is_optional: false,
                help: Some("The ID of the app your IT team registered for Remora in Microsoft Entra.".into()),
                managed_key: Some("MicrosoftClientID".into()),
                is_toggle: false,
            },
        ],
        setup_steps: vec![
            "Ask your IT team for the tenant and the client ID of the app they registered for Remora.".into(),
            "Click “Sign in with Microsoft 365” and sign in in your browser.".into(),
        ],
        setup_label: String::new(),
        setup_url: None,
        egress: Egress {
            hosts: vec!["login.microsoftonline.com".into(), "graph.microsoft.com".into()],
            description: "Signs in with Microsoft, then reads your flagged and unread mail, unanswered invitations \
                          and unread Teams chats from Microsoft Graph."
                .into(),
            external_ai: false,
        },
        logo: None,
        sign_in: Some(sign_in()),
    }
}

/// Outlook mail, Calendar invitations and Teams chats, through Microsoft Graph with a browser sign-in.
pub struct MicrosoftPlugin {
    config: PluginConfig,
    http: Arc<dyn HttpClient>,
    cache: Arc<AccountCache>,
}

impl MicrosoftPlugin {
    pub fn new(config: &PluginConfig, http: Arc<dyn HttpClient>) -> Result<Self, PluginError> {
        config.required("tenant")?;
        config.required("clientId")?;
        config.required("refreshToken")?;
        let config = PluginConfig {
            account_id: config.account_id.clone(),
            values: config.values.clone(),
            cache: config.cache.clone(),
        };
        Ok(MicrosoftPlugin { cache: config.cache.clone(), config, http })
    }

    async fn get<T: DeserializeOwned>(
        &self,
        token: &str,
        path: &str,
        query: &[(&str, &str)],
    ) -> Result<T, PluginError> {
        let request = Request::get(url_with_query(GRAPH, path, query)?)
            .header("Authorization", format!("Bearer {token}"))
            .header("Accept", "application/json")
            // Times in UTC, whatever the mailbox's time zone.
            .header("Prefer", "outlook.timezone=\"UTC\"");
        decode(self.http.as_ref(), request).await
    }

    pub fn snapshot(
        me: &Me,
        flagged: &Page<Message>,
        unread: &Page<Message>,
        events: &Page<Event>,
        chats: &Page<Chat>,
        account_id: &str,
        now: DateTime<Utc>,
    ) -> SourceSnapshot {
        let mut items: Vec<InboxItem> = flagged.value.iter().map(|m| m.flagged_item(account_id)).collect();
        items.extend(
            unread
                .value
                .iter()
                .filter(|m| !flagged.value.iter().any(|f| f.id == m.id) && m.is_to(me))
                .map(|m| m.unread_item(account_id)),
        );
        items.extend(events.value.iter().filter(|e| e.awaits_answer(now)).map(|e| e.item(account_id)));
        let week_ago = now - Duration::days(7);
        items.extend(chats.value.iter().filter_map(|c| c.item(me, account_id, week_ago)));
        let cut = [flagged.next.is_some(), unread.next.is_some()].contains(&true);
        SourceSnapshot {
            identity: me.address().unwrap_or(&me.display_name).to_string(),
            items,
            remarks: if cut { vec![crate::truncated("Microsoft 365")] } else { vec![] },
        }
    }
}

#[async_trait]
impl SourcePlugin for MicrosoftPlugin {
    async fn fetch(&self) -> Result<SourceSnapshot, PluginError> {
        let token = oauth::access_token(&self.config, self.http.as_ref(), &sign_in()).await?;
        let token = token.as_str();
        let me: Me = self
            .cache
            .value("me", IDENTITY_LIFETIME, || {
                self.get(token, "me", &[("$select", "id,displayName,mail,userPrincipalName")])
            })
            .await?;
        let now = Utc::now();
        let (since, until) = (graph_time(now - Duration::days(7)), graph_time(now + Duration::days(14)));
        let (from, filter_unread) = (graph_time(now), format!("isRead eq false and receivedDateTime ge {since}"));
        let fields = "id,subject,bodyPreview,receivedDateTime,webLink,importance,from,toRecipients,flag";
        let flagged_query = [("$filter", "flag/flagStatus eq 'flagged'"), ("$select", fields), ("$top", "25")];
        let unread_query = [("$filter", filter_unread.as_str()), ("$select", fields), ("$top", "25")];
        let events_query = [
            ("startDateTime", from.as_str()),
            ("endDateTime", until.as_str()),
            (
                "$select",
                "id,subject,webLink,isOrganizer,isCancelled,responseRequested,lastModifiedDateTime,start,organizer,\
                 responseStatus",
            ),
            ("$top", "50"),
        ];
        let chats_query = [
            ("$expand", "lastMessagePreview"),
            ("$orderby", "lastMessagePreview/createdDateTime desc"),
            ("$top", "50"),
        ];
        let (flagged, unread, events, chats) = futures::join!(
            self.get::<Page<Message>>(token, "me/messages", &flagged_query),
            self.get::<Page<Message>>(token, "me/mailFolders/inbox/messages", &unread_query),
            self.get::<Page<Event>>(token, "me/calendarView", &events_query),
            self.get::<Page<Chat>>(token, "me/chats", &chats_query),
        );
        // Mail is what the account is for; the calendar and Teams are a bonus (a personal account has no Teams, an
        // organization may not grant every permission): their failure is a remark.
        let mut remarks = vec![];
        let events = bonus(events, "Calendar invitations", &mut remarks)?;
        let chats = bonus(chats, "Teams chats", &mut remarks)?;
        let mut snapshot = Self::snapshot(&me, &flagged?, &unread?, &events, &chats, &self.config.account_id, now);
        snapshot.remarks.extend(remarks);
        Ok(snapshot)
    }
}

/// A part that may fail without failing the account: an empty page and a remark, except a rate limit.
fn bonus<T>(
    result: Result<Page<T>, PluginError>,
    what: &str,
    remarks: &mut Vec<remora_core::Message>,
) -> Result<Page<T>, PluginError> {
    match result {
        Ok(page) => Ok(page),
        Err(error) => match crate::rate_limit_of(&error) {
            Some(limit) => Err(limit),
            None => {
                remarks.push(crate::partial(what, &error));
                Ok(Page::default())
            }
        },
    }
}

/// "2026-10-13T08:00:00Z", as Graph's filters and calendar views take it.
fn graph_time(date: DateTime<Utc>) -> String {
    date.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

/// Graph's times: "2026-10-12T09:14:03Z", or a `dateTimeTimeZone`'s "2026-10-14T00:00:00.0000000" (in UTC, as
/// asked with `Prefer`).
pub fn graph_date(raw: Option<&str>) -> Option<DateTime<Utc>> {
    let raw = raw?;
    DateTime::parse_from_rfc3339(raw)
        .map(|d| d.with_timezone(&Utc))
        .or_else(|_| NaiveDateTime::parse_from_str(raw, "%Y-%m-%dT%H:%M:%S%.f").map(|d| d.and_utc()))
        .ok()
}

static TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)<[^>]*>").expect("valid pattern"));
static BREAK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)<br\s*/?>|</p>|</div>").expect("valid pattern"));

/// A Teams message's HTML as text.
pub fn plain(html: &str) -> String {
    let lines = BREAK.replace_all(html, "\n");
    let text = TAG.replace_all(&lines, "");
    let text = text
        .replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&");
    remora_core::text::readable(text.trim())
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Me {
    id: String,
    display_name: String,
    mail: Option<String>,
    user_principal_name: Option<String>,
}

impl Me {
    fn address(&self) -> Option<&str> {
        self.mail.as_deref().or(self.user_principal_name.as_deref())
    }

    fn owns(&self, address: &str) -> bool {
        [self.mail.as_deref(), self.user_principal_name.as_deref()]
            .into_iter()
            .flatten()
            .any(|mine| mine.eq_ignore_ascii_case(address))
    }
}

#[derive(Deserialize)]
pub struct Page<T> {
    #[serde(default = "Vec::new")]
    value: Vec<T>,
    #[serde(rename = "@odata.nextLink")]
    next: Option<String>,
}

impl<T> Default for Page<T> {
    fn default() -> Self {
        Page { value: vec![], next: None }
    }
}

#[derive(Deserialize)]
struct Address {
    name: Option<String>,
    address: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Recipient {
    email_address: Address,
}

impl Recipient {
    fn person(&self) -> Option<Person> {
        let address = &self.email_address;
        address.name.as_deref().or(address.address.as_deref()).map(Person::named)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ZonedTime {
    date_time: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Flag {
    due_date_time: Option<ZonedTime>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    id: String,
    subject: Option<String>,
    body_preview: Option<String>,
    received_date_time: Option<String>,
    web_link: Option<String>,
    importance: Option<String>,
    from: Option<Recipient>,
    #[serde(default)]
    to_recipients: Vec<Recipient>,
    flag: Option<Flag>,
}

impl Message {
    /// Sent to you, not only copied or sent to a list you're on.
    fn is_to(&self, me: &Me) -> bool {
        self.to_recipients.iter().any(|r| r.email_address.address.as_deref().is_some_and(|a| me.owns(a)))
    }

    fn base(&self, account_id: &str, bundle: InboxBundle) -> InboxItem {
        let important = self.importance.as_deref() == Some("high");
        let preview = self.body_preview.as_deref().map(|p| p.split_whitespace().collect::<Vec<_>>().join(" "));
        InboxItem {
            id: InboxItem::make_id(account_id, format!("mail/{}", self.id)),
            account_id: account_id.into(),
            plugin_id: "microsoft".into(),
            bundle,
            title: remora_core::text::readable(
                self.subject.as_deref().filter(|s| !s.trim().is_empty()).unwrap_or("(no subject)"),
            ),
            context: "Outlook".into(),
            preview: preview.filter(|p| !p.is_empty()),
            url: self.web_link.clone(),
            app_url: None,
            author: self.from.as_ref().and_then(Recipient::person),
            participants: vec![],
            badges: if important { vec![Badge::new(badge::PRIORITY_HIGH, "High", Tone::Warning)] } else { vec![] },
            date: graph_date(self.received_date_time.as_deref()).unwrap_or_else(Utc::now),
            needs_action: true,
            priority: important.then_some(Priority::High),
            due: None,
            expires: None,
            changes: None,
            suggested_people: None,
            recurrence: None,
            reactions: None,
        }
    }

    /// Flagged: a task you set yourself, with its due date if it has one.
    fn flagged_item(&self, account_id: &str) -> InboxItem {
        let mut item = self.base(account_id, InboxBundle::tasks());
        item.badges.push(Badge::new(badge::FLAGGED, "Flagged", Tone::Accent));
        item.due =
            self.flag.as_ref().and_then(|f| f.due_date_time.as_ref()).and_then(|d| graph_date(Some(&d.date_time)));
        if let Some(due) = item.due {
            item.badges.push(due_badge(due));
        }
        item
    }

    /// Unread and sent to you: sorted by what it asks, as a direct message is.
    fn unread_item(&self, account_id: &str) -> InboxItem {
        self.base(account_id, InboxBundle::direct_messages())
    }
}

#[derive(Deserialize)]
struct ResponseStatus {
    response: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    id: String,
    subject: Option<String>,
    web_link: Option<String>,
    #[serde(default)]
    is_organizer: bool,
    #[serde(default)]
    is_cancelled: bool,
    response_requested: Option<bool>,
    last_modified_date_time: Option<String>,
    start: Option<ZonedTime>,
    organizer: Option<Recipient>,
    response_status: Option<ResponseStatus>,
}

impl Event {
    fn start(&self) -> Option<DateTime<Utc>> {
        self.start.as_ref().and_then(|s| graph_date(Some(&s.date_time)))
    }

    /// An invitation you haven't answered, for a meeting still to come.
    fn awaits_answer(&self, now: DateTime<Utc>) -> bool {
        let response = self.response_status.as_ref().and_then(|r| r.response.as_deref());
        response == Some("notResponded")
            && !self.is_organizer
            && !self.is_cancelled
            && self.response_requested != Some(false)
            && self.start().is_some_and(|start| start > now)
    }

    fn item(&self, account_id: &str) -> InboxItem {
        let start = self.start();
        let mut badges = vec![Badge::new(badge::INVITATION, "Invitation", Tone::Accent)];
        if let Some(start) = start {
            badges.push(Badge::new(
                badge::EVENT,
                &start.with_timezone(&Local).format("%Y-%m-%d %H:%M").to_string(),
                Tone::Neutral,
            ));
        }
        InboxItem {
            id: InboxItem::make_id(account_id, format!("event/{}", self.id)),
            account_id: account_id.into(),
            plugin_id: "microsoft".into(),
            bundle: InboxBundle::reply(),
            title: remora_core::text::readable(
                self.subject.as_deref().filter(|s| !s.trim().is_empty()).unwrap_or("(no subject)"),
            ),
            context: "Outlook".into(),
            preview: None,
            url: self.web_link.clone(),
            app_url: None,
            author: self.organizer.as_ref().and_then(Recipient::person),
            participants: vec![],
            badges,
            // Moved or renamed: news again.
            date: graph_date(self.last_modified_date_time.as_deref()).unwrap_or_else(Utc::now),
            needs_action: true,
            priority: None,
            due: start,
            // Once the meeting starts, answering is moot.
            expires: start,
            changes: None,
            suggested_people: None,
            recurrence: None,
            reactions: None,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Viewpoint {
    #[serde(default)]
    is_hidden: bool,
    last_message_read_date_time: Option<String>,
}

#[derive(Deserialize)]
struct Body {
    content: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Sender {
    id: Option<String>,
    display_name: Option<String>,
}

#[derive(Deserialize)]
struct From {
    user: Option<Sender>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatMessage {
    created_date_time: Option<String>,
    #[serde(default)]
    is_deleted: bool,
    body: Option<Body>,
    from: Option<From>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Chat {
    id: String,
    topic: Option<String>,
    chat_type: String,
    web_url: Option<String>,
    viewpoint: Option<Viewpoint>,
    last_message_preview: Option<ChatMessage>,
}

impl Chat {
    /// The last message, when someone else wrote it this week and you haven't read it: one-to-one chats as direct
    /// messages, group chats as mentions. Meeting chats are left out.
    fn item(&self, me: &Me, account_id: &str, since: DateTime<Utc>) -> Option<InboxItem> {
        let message = self.last_message_preview.as_ref().filter(|m| !m.is_deleted)?;
        let sender = message.from.as_ref()?.user.as_ref()?;
        let date = graph_date(message.created_date_time.as_deref())?;
        let viewpoint = self.viewpoint.as_ref();
        let read = viewpoint.and_then(|v| graph_date(v.last_message_read_date_time.as_deref()));
        let bundle = match self.chat_type.as_str() {
            "oneOnOne" => InboxBundle::direct_messages(),
            "group" => InboxBundle::mentions(),
            _ => return None,
        };
        if sender.id.as_deref() == Some(me.id.as_str())
            || date <= since
            || read.is_some_and(|read| read >= date)
            || viewpoint.is_some_and(|v| v.is_hidden)
        {
            return None;
        }
        let text = plain(message.body.as_ref().and_then(|b| b.content.as_deref()).unwrap_or_default());
        let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty());
        let title = lines.next().unwrap_or("(no text)").to_string();
        let preview = lines.collect::<Vec<_>>().join(" ");
        let name = sender.display_name.clone();
        Some(InboxItem {
            id: InboxItem::make_id(account_id, format!("chat/{}", self.id)),
            account_id: account_id.into(),
            plugin_id: "microsoft".into(),
            bundle,
            title,
            context: self.topic.clone().filter(|t| !t.trim().is_empty()).or(name.clone()).unwrap_or("Teams".into()),
            preview: (!preview.is_empty()).then_some(preview),
            url: self.web_url.clone(),
            app_url: None,
            author: name.as_deref().map(Person::named),
            participants: vec![],
            badges: vec![],
            date,
            needs_action: true,
            priority: None,
            due: None,
            expires: None,
            changes: None,
            suggested_people: None,
            recurrence: None,
            reactions: None,
        })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::stub::{config, Recording, StubHttp};
    use chrono::TimeZone;

    const ME: &str = r#"{"id": "u-me", "displayName": "Alice Martin", "mail": "alice@acme.com",
        "userPrincipalName": "alice@acme.onmicrosoft.com"}"#;

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, 13, 9, 0, 0).unwrap()
    }

    fn page<T: DeserializeOwned>(json: &str) -> Page<T> {
        serde_json::from_str(json).unwrap()
    }

    fn me() -> Me {
        serde_json::from_str(ME).unwrap()
    }

    const FLAGGED: &str = r#"{"value": [
        {"id": "m1", "subject": "Budget 2027", "bodyPreview": "Can you  check\r\nthe numbers?",
         "receivedDateTime": "2026-10-12T16:00:00Z", "webLink": "https://outlook.office365.com/owa/?ItemID=m1",
         "importance": "high", "from": {"emailAddress": {"name": "Bob Stone", "address": "bob@acme.com"}},
         "toRecipients": [{"emailAddress": {"address": "alice@acme.com"}}],
         "flag": {"flagStatus": "flagged", "dueDateTime": {"dateTime": "2026-10-20T00:00:00.0000000", "timeZone": "UTC"}}}
    ]}"#;

    const UNREAD: &str = r#"{"value": [
        {"id": "m1", "subject": "Budget 2027", "receivedDateTime": "2026-10-12T16:00:00Z",
         "toRecipients": [{"emailAddress": {"address": "alice@acme.com"}}]},
        {"id": "m2", "subject": "Lunch on Friday?", "bodyPreview": "Are you in?", "receivedDateTime": "2026-10-13T08:00:00Z",
         "from": {"emailAddress": {"name": "Carol", "address": "carol@acme.com"}},
         "toRecipients": [{"emailAddress": {"address": "ALICE@acme.onmicrosoft.com"}}]},
        {"id": "m3", "subject": "Newsletter", "receivedDateTime": "2026-10-13T07:00:00Z",
         "toRecipients": [{"emailAddress": {"address": "everyone@acme.com"}}]}
    ], "@odata.nextLink": "https://graph.microsoft.com/v1.0/me/messages?$skip=25"}"#;

    const EVENTS: &str = r#"{"value": [
        {"id": "e1", "subject": "Design review", "webLink": "https://outlook.office365.com/calendar/item/e1",
         "isOrganizer": false, "isCancelled": false, "responseRequested": true,
         "lastModifiedDateTime": "2026-10-12T10:00:00Z", "start": {"dateTime": "2026-10-14T13:00:00.0000000", "timeZone": "UTC"},
         "organizer": {"emailAddress": {"name": "Dan", "address": "dan@acme.com"}}, "responseStatus": {"response": "notResponded"}},
        {"id": "e2", "subject": "Accepted already", "start": {"dateTime": "2026-10-14T13:00:00.0000000"},
         "responseStatus": {"response": "accepted"}},
        {"id": "e3", "subject": "My own meeting", "isOrganizer": true, "start": {"dateTime": "2026-10-14T13:00:00.0000000"},
         "responseStatus": {"response": "notResponded"}},
        {"id": "e4", "subject": "Cancelled", "isCancelled": true, "start": {"dateTime": "2026-10-14T13:00:00.0000000"},
         "responseStatus": {"response": "notResponded"}},
        {"id": "e5", "subject": "Already started", "start": {"dateTime": "2026-10-13T08:30:00.0000000"},
         "responseStatus": {"response": "notResponded"}}
    ]}"#;

    const CHATS: &str = r#"{"value": [
        {"id": "c1", "chatType": "oneOnOne", "webUrl": "https://teams.microsoft.com/l/chat/c1",
         "viewpoint": {"isHidden": false, "lastMessageReadDateTime": "2026-10-12T08:00:00Z"},
         "lastMessagePreview": {"createdDateTime": "2026-10-13T08:30:00Z", "isDeleted": false,
          "body": {"contentType": "html", "content": "<p>Could you send the deck?</p><p>Thanks &amp; see you</p>"},
          "from": {"user": {"id": "u-erin", "displayName": "Erin"}}}},
        {"id": "c2", "topic": "Launch", "chatType": "group",
         "viewpoint": {"lastMessageReadDateTime": "2026-10-13T08:00:00Z"},
         "lastMessagePreview": {"createdDateTime": "2026-10-13T08:40:00Z",
          "body": {"content": "Shipped!"}, "from": {"user": {"id": "u-frank", "displayName": "Frank"}}}},
        {"id": "c3", "chatType": "oneOnOne", "viewpoint": {"lastMessageReadDateTime": "2026-10-13T09:00:00Z"},
         "lastMessagePreview": {"createdDateTime": "2026-10-13T08:50:00Z", "body": {"content": "read already"},
          "from": {"user": {"id": "u-erin", "displayName": "Erin"}}}},
        {"id": "c4", "chatType": "oneOnOne",
         "lastMessagePreview": {"createdDateTime": "2026-10-13T08:50:00Z", "body": {"content": "mine"},
          "from": {"user": {"id": "u-me", "displayName": "Alice Martin"}}}},
        {"id": "c5", "chatType": "meeting",
         "lastMessagePreview": {"createdDateTime": "2026-10-13T08:50:00Z", "body": {"content": "meeting chat"},
          "from": {"user": {"id": "u-erin", "displayName": "Erin"}}}},
        {"id": "c6", "chatType": "oneOnOne",
         "lastMessagePreview": {"createdDateTime": "2026-10-01T08:50:00Z", "body": {"content": "old"},
          "from": {"user": {"id": "u-erin", "displayName": "Erin"}}}}
    ]}"#;

    fn snapshot() -> SourceSnapshot {
        MicrosoftPlugin::snapshot(&me(), &page(FLAGGED), &page(UNREAD), &page(EVENTS), &page(CHATS), "acc", now())
    }

    fn item<'a>(snapshot: &'a SourceSnapshot, id: &str) -> &'a InboxItem {
        snapshot.items.iter().find(|i| i.id == format!("acc/{id}")).unwrap()
    }

    #[test]
    fn flagged_mail_is_a_task_with_its_due_date() {
        let snapshot = snapshot();
        let budget = item(&snapshot, "mail/m1");
        assert_eq!(budget.bundle, InboxBundle::tasks());
        assert_eq!(budget.title, "Budget 2027");
        assert_eq!(budget.preview.as_deref(), Some("Can you check the numbers?"));
        assert_eq!(budget.author.as_ref().map(|a| a.name.as_str()), Some("Bob Stone"));
        assert_eq!(budget.priority, Some(Priority::High));
        assert!(budget.has_badge("flagged") && budget.has_badge("priority.high"));
        assert_eq!(budget.due, Some(Utc.with_ymd_and_hms(2026, 10, 20, 0, 0, 0).unwrap()));
        assert_eq!(snapshot.items.iter().filter(|i| i.id == "acc/mail/m1").count(), 1, "flagged and unread: once");
    }

    #[test]
    fn unread_mail_counts_only_when_sent_to_you() {
        let snapshot = snapshot();
        let lunch = item(&snapshot, "mail/m2");
        assert_eq!(lunch.bundle, InboxBundle::direct_messages());
        assert!(snapshot.items.iter().all(|i| i.id != "acc/mail/m3"), "a list you're on isn't you");
        assert_eq!(snapshot.identity, "alice@acme.com");
        assert_eq!(snapshot.remarks.len(), 1, "more unread mail than Remora reads");
    }

    #[test]
    fn invitations_you_havent_answered_until_the_meeting_starts() {
        let snapshot = snapshot();
        let review = item(&snapshot, "event/e1");
        assert_eq!(review.bundle, InboxBundle::reply());
        assert_eq!(review.author.as_ref().map(|a| a.name.as_str()), Some("Dan"));
        let start = Utc.with_ymd_and_hms(2026, 10, 14, 13, 0, 0).unwrap();
        assert_eq!((review.due, review.expires), (Some(start), Some(start)));
        assert!(review.has_badge("invitation") && review.has_badge("event"));
        let events: Vec<_> = snapshot.items.iter().filter(|i| i.id.starts_with("acc/event/")).collect();
        assert_eq!(events.len(), 1, "answered, yours, cancelled or started: no invitation");
    }

    #[test]
    fn unread_teams_chats_from_others_this_week() {
        let snapshot = snapshot();
        let erin = item(&snapshot, "chat/c1");
        assert_eq!(erin.bundle, InboxBundle::direct_messages());
        assert_eq!(erin.title, "Could you send the deck?");
        assert_eq!(erin.preview.as_deref(), Some("Thanks & see you"));
        assert_eq!(erin.context, "Erin");
        let launch = item(&snapshot, "chat/c2");
        assert_eq!((launch.bundle.clone(), launch.context.as_str()), (InboxBundle::mentions(), "Launch"));
        let chats: Vec<_> = snapshot.items.iter().filter(|i| i.id.starts_with("acc/chat/")).collect();
        assert_eq!(chats.len(), 2, "read, yours, a meeting's or old: left out");
    }

    #[test]
    fn needs_the_tenant_the_app_and_a_sign_in() {
        let http: Arc<dyn HttpClient> = Arc::new(StubHttp::paths(&[]));
        for missing in ["tenant", "clientId", "refreshToken"] {
            let mut values = vec![("tenant", "acme.com"), ("clientId", "app-1"), ("refreshToken", "r1")];
            values.retain(|(k, _)| *k != missing);
            assert_eq!(
                MicrosoftPlugin::new(&config(&values), http.clone()).err(),
                Some(PluginError::MissingField(missing.into()))
            );
        }
    }

    /// The fetch `registry`'s egress test runs through the guard: the sign-in's token, then Graph.
    pub(crate) fn egress_fixture() -> crate::stub::EgressFixture {
        let tokens = r#"{"access_token": "a1", "refresh_token": "r2", "expires_in": 3600}"#;
        crate::stub::EgressFixture {
            settings: vec![("tenant", "acme.com"), ("clientId", "app-1"), ("refreshToken", "r1")],
            http: Arc::new(StubHttp::paths(&[
                ("/oauth2/v2.0/token", tokens),
                ("/me", ME),
                ("/me/messages", FLAGGED),
                ("/inbox/messages", UNREAD),
                ("/me/calendarView", EVENTS),
                ("/me/chats", CHATS),
            ])),
        }
    }

    #[tokio::test]
    async fn fetches_with_one_token_and_keeps_mail_when_teams_fails() {
        let tokens = r#"{"access_token": "a1", "refresh_token": "r2", "expires_in": 3600}"#;
        let http = Arc::new(Recording::new(StubHttp::paths(&[
            ("/oauth2/v2.0/token", tokens),
            ("/me", ME),
            ("/me/messages", FLAGGED),
            ("/inbox/messages", UNREAD),
            ("/me/calendarView", EVENTS),
        ])));
        let config = config(&[("tenant", "acme.com"), ("clientId", "app-1"), ("refreshToken", "r1")]);
        let plugin = MicrosoftPlugin::new(&config, http.clone()).unwrap();
        let snapshot = plugin.fetch().await.unwrap();
        assert!(snapshot.items.iter().any(|i| i.id.ends_with("/mail/m2")));
        assert!(snapshot.remarks.iter().any(|r| r.text.starts_with("Teams chats couldn’t be read")));
        assert_eq!(http.paths().iter().filter(|p| p.ends_with("/token")).count(), 1);
        assert_eq!(config.cache.take_renewed().get("refreshToken").map(String::as_str), Some("r2"));
    }

    #[test]
    fn reads_graph_times_and_teams_html() {
        assert_eq!(
            graph_date(Some("2026-10-14T13:00:00.0000000")),
            Some(Utc.with_ymd_and_hms(2026, 10, 14, 13, 0, 0).unwrap())
        );
        assert_eq!(
            graph_date(Some("2026-10-12T16:00:00Z")),
            Some(Utc.with_ymd_and_hms(2026, 10, 12, 16, 0, 0).unwrap())
        );
        assert_eq!(plain("<div>Hi <at id=\"0\">Alice</at>,<br>ok?</div>"), "Hi Alice,\nok?");
    }
}

//! The `remora` command: what a launcher, a status bar or a script may ask the running app, over a socket
//! only this user can open (a named pipe on Windows). The count, a tab's items, a new reminder: never Done, a snooze or
//! anything that removes. Items whose tool hides message content come without their title. One JSON object per
//! line each way. Same rule on macOS (`RemoraCommand.swift`), with the same tests.

use crate::link::{time, LINK_TEXT_LIMIT, TABS};
use crate::{InboxItem, InboxLayout};
use chrono::{DateTime, Duration, Local, NaiveTime, TimeZone, Utc};
use serde_json::{json, Value};

/// A request longer than this is refused: it can only be a mistake.
pub const COMMAND_LINE_LIMIT: usize = 4096;

pub const COMMAND_USAGE: &str = "Usage: remora count [--json]
       remora list [--tab in-progress|my-turn|waiting|snoozed|done] [--json]
       remora remind \"Call Erin\" --at 15:00 [--json]
Asks the Remora app running on this computer; nothing goes over the network.";

/// Saves a reminder (its text, its time) and returns its id.
pub type SaveReminder<'a> = dyn FnMut(&str, DateTime<Utc>) -> Result<String, String> + 'a;

/// What the app answers one request line with: a JSON object, `"ok": false` with an `error` when refused.
/// `hides`: whether a tool's message content is hidden (Settings, or the organization's policy). `remind` saves a
/// reminder and returns its id.
pub fn answer_command(
    line: &str,
    layout: &InboxLayout,
    hides: &dyn Fn(&str) -> bool,
    now: DateTime<Utc>,
    remind: &mut SaveReminder,
) -> String {
    match respond(line, layout, hides, now, remind) {
        Ok(value) => value.to_string(),
        Err(error) => json!({ "ok": false, "error": error }).to_string(),
    }
}

fn respond(
    line: &str,
    layout: &InboxLayout,
    hides: &dyn Fn(&str) -> bool,
    now: DateTime<Utc>,
    remind: &mut SaveReminder,
) -> Result<Value, String> {
    // Refusals are English: they answer a terminal, not the interface.
    let unknown = || "Not a request Remora knows.".to_string();
    if line.len() > COMMAND_LINE_LIMIT {
        return Err(unknown());
    }
    let request: Value = serde_json::from_str(line.trim()).map_err(|_| unknown())?;
    let text = |name: &str| request.get(name).and_then(Value::as_str);
    match text("command") {
        Some("count") => {
            let tabs: serde_json::Map<String, Value> =
                TABS.iter().map(|(name, _)| (name.to_string(), json!(tab_items(layout, name).len()))).collect();
            Ok(json!({ "ok": true, "count": layout.action_count(), "tabs": tabs }))
        }
        Some("list") => {
            let tab = text("tab").unwrap_or("my-turn").trim().to_ascii_lowercase();
            if !TABS.iter().any(|(name, _)| *name == tab) {
                return Err(format!("No tab {tab}: in-progress, my-turn, waiting, snoozed or done."));
            }
            let items: Vec<Value> = tab_items(layout, &tab).into_iter().map(|item| row(item, hides)).collect();
            Ok(json!({ "ok": true, "tab": tab, "items": items }))
        }
        Some("remind") => {
            let title: String = text("text").unwrap_or_default().trim().chars().take(LINK_TEXT_LIMIT).collect();
            if title.is_empty() {
                return Err(String::from("Type what to remember."));
            }
            let at = command_time(text("at").unwrap_or_default(), now)
                .ok_or_else(|| String::from("Say when: 15:00, or 2026-10-14T09:00."))?;
            if at <= now {
                return Err(String::from("That time has passed."));
            }
            let id = remind(&title, at)?;
            Ok(json!({ "ok": true, "id": id, "at": at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true) }))
        }
        _ => Err(unknown()),
    }
}

fn tab_items<'a>(layout: &'a InboxLayout, tab: &str) -> Vec<&'a InboxItem> {
    match tab {
        "in-progress" => layout.in_progress.iter().collect(),
        "my-turn" => layout.my_turn_items().collect(),
        "waiting" => layout.waiting.iter().flat_map(|g| &g.items).collect(),
        "snoozed" => layout.snoozed.iter().collect(),
        _ => layout.done.iter().collect(),
    }
}

/// Where and what kind, always; what was written (the title) only when the tool's content isn't hidden. `why`: why
/// Remora sorted a message there (`SortReason`), its words left out when the content is hidden; null for what the
/// source sorted. `bot`: written by an app, a bot or an agent account, as the source says.
fn row(item: &InboxItem, hides: &dyn Fn(&str) -> bool) -> Value {
    let hidden = hides(&item.plugin_id);
    json!({
        "id": item.id,
        "source": item.plugin_id,
        "kind": item.bundle.title,
        "title": if hidden { None } else { Some(&item.title) },
        "context": item.context,
        "url": item.url,
        "hidden": hidden,
        "why": item.reason.as_ref().map(|r| json!({
            "code": r.code,
            "detail": if hidden { None } else { r.detail.as_deref() },
        })),
        "bot": item.from_bot(),
    })
}

/// "15:00": the next 15:00 (tomorrow once it has passed). Otherwise as a link has it: "2026-10-14T09:00" on this
/// computer's clock, or with its offset.
pub fn command_time(raw: &str, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
    let raw = raw.trim();
    let Ok(clock) = NaiveTime::parse_from_str(raw, "%H:%M") else { return time(raw) };
    let today = now.with_timezone(&Local).date_naive();
    let at = |day: chrono::NaiveDate| Local.from_local_datetime(&day.and_time(clock)).earliest();
    let first = at(today)?.with_timezone(&Utc);
    if first > now {
        Some(first)
    } else {
        at(today + Duration::days(1)).map(|at| at.with_timezone(&Utc))
    }
}

/// What the command line asks: the request line to send, and whether to print the answer as JSON.
#[derive(Debug, PartialEq)]
pub struct CommandInvocation {
    pub request: String,
    pub json: bool,
}

/// `remora count`, `remora list --tab waiting --json`, `remora remind Call Erin --at 15:00`. Err: the usage.
pub fn command_invocation(args: &[String]) -> Result<CommandInvocation, String> {
    let usage = || COMMAND_USAGE.to_string();
    let (command, rest) = args.split_first().ok_or_else(usage)?;
    let (mut json, mut words, mut options) = (false, Vec::new(), serde_json::Map::new());
    let mut rest = rest.iter();
    while let Some(arg) = rest.next() {
        if arg == "--json" {
            json = true;
        } else if let Some(name) = arg.strip_prefix("--") {
            let (name, value) = match name.split_once('=') {
                Some((name, value)) => (name, value.to_string()),
                None => (name, rest.next().ok_or_else(usage)?.clone()),
            };
            options.insert(name.to_string(), json!(value));
        } else {
            words.push(arg.as_str());
        }
    }
    let allowed: &[&str] = match command.as_str() {
        "count" => &[],
        "list" => &["tab"],
        "remind" => &["at"],
        _ => return Err(usage()),
    };
    let only_words = command == "remind" || words.is_empty();
    if !only_words || options.keys().any(|name| !allowed.contains(&name.as_str())) {
        return Err(usage());
    }
    options.insert("command".into(), json!(command));
    if command == "remind" {
        options.insert("text".into(), json!(words.join(" ")));
    }
    Ok(CommandInvocation { request: Value::Object(options).to_string(), json })
}

/// What the command prints for the app's answer: (standard output, standard error, exit code). JSON prints the answer
/// as it came; otherwise a number, a line per item, or a sentence. 1: the app refused.
pub fn command_output(response: &str, json: bool) -> (String, String, i32) {
    let Ok(value) = serde_json::from_str::<Value>(response.trim()) else {
        return (String::new(), "Remora answered something this command doesn't understand.".into(), 1);
    };
    let ok = value["ok"].as_bool() == Some(true);
    let code = if ok { 0 } else { 1 };
    if json {
        return (value.to_string(), String::new(), code);
    }
    if !ok {
        return (String::new(), value["error"].as_str().unwrap_or("Remora refused.").to_string(), 1);
    }
    let text = if let Some(count) = value["count"].as_u64() {
        count.to_string()
    } else if let Some(items) = value["items"].as_array() {
        if items.is_empty() {
            "Nothing here.".into()
        } else {
            items
                .iter()
                .map(|item| {
                    let title = item["title"].as_str().unwrap_or("(content hidden)");
                    let kind = item["kind"].as_str().unwrap_or_default();
                    let context = item["context"].as_str().unwrap_or_default();
                    format!("{kind} · {context} · {title}")
                })
                .collect::<Vec<_>>()
                .join("\n")
        }
    } else if let Some(at) = value["at"].as_str().and_then(|at| DateTime::parse_from_rfc3339(at).ok()) {
        format!("Reminder set for {}.", at.with_timezone(&Local).format("%Y-%m-%d %H:%M"))
    } else {
        String::new()
    };
    (text, String::new(), 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Group, InboxBundle};

    fn now() -> DateTime<Utc> {
        Local.with_ymd_and_hms(2026, 10, 15, 12, 0, 0).unwrap().with_timezone(&Utc)
    }

    fn item(id: &str, plugin: &str, bundle: InboxBundle, title: &str, context: &str) -> InboxItem {
        InboxItem {
            plugin_id: plugin.into(),
            title: title.into(),
            context: context.into(),
            url: Some(format!("https://example.com/{id}")),
            ..crate::fixtures::item(id, bundle)
        }
    }

    /// Two items your turn (one from Slack, whose content is hidden), one waiting, one done.
    fn layout() -> InboxLayout {
        let review = item("1", "github", InboxBundle::reviews(), "Fix login", "acme/app #12");
        let mut mention = item("2", "slack", InboxBundle::mentions(), "Salary review at 3?", "#general");
        mention.reason = Some(crate::SortReason::new(crate::sort_reason::KEYWORD, Some("salary")));
        let waiting = item("3", "gitlab", InboxBundle::awaiting(), "Bump deps", "acme/api !4");
        let done = item("4", "linear", InboxBundle::reviews(), "Ship it", "ENG-1");
        InboxLayout {
            my_turn: vec![
                Group { bundle: InboxBundle::reviews(), items: vec![review] },
                Group { bundle: InboxBundle::mentions(), items: vec![mention] },
            ],
            waiting: vec![Group { bundle: InboxBundle::awaiting(), items: vec![waiting] }],
            done: vec![done],
            ..InboxLayout::default()
        }
    }

    fn ask(line: &str) -> (Value, Vec<(String, DateTime<Utc>)>) {
        let mut saved = vec![];
        let hides = |plugin: &str| plugin == "slack";
        let mut remind = |text: &str, at: DateTime<Utc>| {
            saved.push((text.to_string(), at));
            Ok("reminder/1".to_string())
        };
        let answer = answer_command(line, &layout(), &hides, now(), &mut remind);
        (serde_json::from_str(&answer).unwrap(), saved)
    }

    #[test]
    fn counts_what_needs_you_and_each_tab() {
        let (answer, _) = ask(r#"{"command":"count"}"#);
        assert_eq!(answer["ok"], true);
        assert_eq!(answer["count"], 2);
        assert_eq!(answer["tabs"], json!({ "in-progress": 0, "my-turn": 2, "waiting": 1, "snoozed": 0, "done": 1 }));
    }

    #[test]
    fn lists_a_tab_without_the_hidden_content() {
        let (answer, _) = ask(r#"{"command":"list"}"#);
        assert_eq!(answer["tab"], "my-turn", "your turn unless told");
        let items = answer["items"].as_array().unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0]["title"], "Fix login");
        assert_eq!(items[0]["kind"], "To review");
        assert_eq!(items[0]["url"], "https://example.com/1");
        assert_eq!(items[0]["hidden"], false);
        assert_eq!(items[1]["title"], Value::Null, "Slack's content is hidden");
        assert_eq!(items[1]["context"], "#general", "where, not what");
        assert_eq!(items[1]["hidden"], true);
        assert_eq!(items[0]["why"], Value::Null, "the source sorted it");
        assert_eq!(items[1]["why"], json!({ "code": "keyword", "detail": null }), "why, without the hidden words");
        assert!(!answer.to_string().to_lowercase().contains("salary"));

        let (waiting, _) = ask(r#"{"command":"list","tab":"Waiting"}"#);
        assert_eq!(waiting["items"][0]["title"], "Bump deps");
        let (nowhere, _) = ask(r#"{"command":"list","tab":"mine"}"#);
        assert_eq!(nowhere["ok"], false);
        assert_eq!(nowhere["error"], "No tab mine: in-progress, my-turn, waiting, snoozed or done.");
    }

    #[test]
    fn saves_a_reminder_at_the_next_such_time() {
        let local = |day, hour| Local.with_ymd_and_hms(2026, 10, day, hour, 0, 0).unwrap().with_timezone(&Utc);
        let (answer, saved) = ask(r#"{"command":"remind","text":"  Call Erin ","at":"15:00"}"#);
        assert_eq!(saved, [("Call Erin".to_string(), local(15, 15))]);
        assert_eq!(answer["id"], "reminder/1");
        assert_eq!(answer["at"], local(15, 15).to_rfc3339_opts(chrono::SecondsFormat::Secs, true));
        let (_, tomorrow) = ask(r#"{"command":"remind","text":"Stand-up","at":"09:00"}"#);
        assert_eq!(tomorrow[0].1, local(16, 9), "09:00 has passed: tomorrow");
        let (_, dated) = ask(r#"{"command":"remind","text":"x","at":"2026-10-20T09:00"}"#);
        assert_eq!(dated[0].1, local(20, 9));
        let long = format!(r#"{{"command":"remind","text":"{}","at":"15:00"}}"#, "a".repeat(500));
        assert_eq!(ask(&long).1[0].0.len(), LINK_TEXT_LIMIT);

        for (line, error) in [
            (r#"{"command":"remind","text":" ","at":"15:00"}"#, "Type what to remember."),
            (r#"{"command":"remind","text":"x"}"#, "Say when: 15:00, or 2026-10-14T09:00."),
            (r#"{"command":"remind","text":"x","at":"2026-10-01T09:00"}"#, "That time has passed."),
        ] {
            let (answer, saved) = ask(line);
            assert_eq!((answer["ok"].clone(), answer["error"].clone()), (json!(false), json!(error)), "{line}");
            assert!(saved.is_empty());
        }
    }

    /// Nothing marks an item done, snoozes it or removes anything.
    #[test]
    fn refuses_anything_else() {
        let long = format!(r#"{{"command":"count","pad":"{}"}}"#, "a".repeat(COMMAND_LINE_LIMIT));
        for line in [r#"{"command":"done","id":"1"}"#, r#"{"command":"snooze"}"#, "count", "", "[]", long.as_str()] {
            let (answer, saved) = ask(line);
            assert_eq!(answer, json!({ "ok": false, "error": "Not a request Remora knows." }), "{line}");
            assert!(saved.is_empty());
        }
    }

    #[test]
    fn reads_the_command_line() {
        let args = |line: &str| line.split(' ').filter(|a| !a.is_empty()).map(String::from).collect::<Vec<_>>();
        let request = |line: &str| {
            command_invocation(&args(line)).map(|i| (serde_json::from_str::<Value>(&i.request).unwrap(), i.json))
        };
        assert_eq!(request("count"), Ok((json!({ "command": "count" }), false)));
        assert_eq!(request("list --tab waiting --json"), Ok((json!({ "command": "list", "tab": "waiting" }), true)));
        assert_eq!(request("list --tab=done"), Ok((json!({ "command": "list", "tab": "done" }), false)));
        assert_eq!(
            request("remind Call Erin --at 15:00"),
            Ok((json!({ "command": "remind", "text": "Call Erin", "at": "15:00" }), false))
        );
        for wrong in ["", "help", "done 1", "list --tab", "list --at 15:00", "count extra", "list waiting"] {
            assert_eq!(request(wrong), Err(COMMAND_USAGE.to_string()), "{wrong}");
        }
    }

    #[test]
    fn prints_a_number_a_line_per_item_or_a_sentence() {
        let print = |line: &str, json| command_output(&answer_text(line), json);
        assert_eq!(print(r#"{"command":"count"}"#, false), ("2".into(), String::new(), 0));
        assert_eq!(
            print(r#"{"command":"list"}"#, false).0,
            "To review · acme/app #12 · Fix login\nMentions · #general · (content hidden)"
        );
        assert_eq!(print(r#"{"command":"list","tab":"snoozed"}"#, false).0, "Nothing here.");
        let at = Local.with_ymd_and_hms(2026, 10, 15, 15, 0, 0).unwrap();
        assert_eq!(
            print(r#"{"command":"remind","text":"x","at":"15:00"}"#, false).0,
            format!("Reminder set for {}.", at.format("%Y-%m-%d %H:%M"))
        );
        assert_eq!(print(r#"{"command":"list","tab":"x"}"#, false).2, 1);
        assert_eq!(print(r#"{"command":"done"}"#, false), (String::new(), "Not a request Remora knows.".into(), 1));
        let (out, _, code) = print(r#"{"command":"count"}"#, true);
        assert_eq!((serde_json::from_str::<Value>(&out).unwrap()["count"].clone(), code), (json!(2), 0));
        assert_eq!(command_output("garbage", false).2, 1);
    }

    fn answer_text(line: &str) -> String {
        ask(line).0.to_string()
    }
}

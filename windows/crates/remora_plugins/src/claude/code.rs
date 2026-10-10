//! The brief through the Claude Code already signed in on this computer: no API key needed. A port of the macOS
//! `ClaudeCodePlugin.swift`.

use super::prompt::{self, Schema};
use super::runner::{CommandError, CommandRunner, ProcessCommandRunner};
use crate::{AssistantPlugin, PluginConfig, PluginError};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use remora_core::{Brief, ConfigField, Egress, InboxItem, PluginManifest, SnoozedItem, TriageSuggestion};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf, Prefix};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::SystemTime;

pub const ID: &str = "claude-code";
pub const DEFAULT_DIGEST_MODEL: &str = "claude-haiku-5-5";

/// The same words as the macOS app, which the shared French dictionary translates.
pub fn manifest() -> PluginManifest {
    let field = |key: &str, label: &str| ConfigField {
        key: key.into(), label: label.into(), placeholder: String::new(), default_value: String::new(),
        is_secret: false, is_optional: false, help: None,
    };
    PluginManifest {
        id: ID.into(),
        name: "Claude Code".into(),
        summary: "The brief through the Claude Code signed in on this computer, on your Claude plan. No API key.".into(),
        fields: vec![
            ConfigField {
                placeholder: "Found automatically".into(),
                is_optional: true,
                help: Some("Only if Claude Code is installed somewhere unusual (run `where.exe claude` in a terminal).".into()),
                ..field("path", "Path to claude")
            },
            ConfigField { placeholder: "Claude Code’s default".into(), is_optional: true, ..field("model", "Model for the brief") },
            ConfigField {
                default_value: DEFAULT_DIGEST_MODEL.into(),
                help: Some("A lighter model is enough for two sentences and uses less of your plan.".into()),
                ..field("digestModel", "Model for bundle summaries")
            },
        ],
        setup_steps: vec![
            "Install Claude Code with Anthropic’s installer (recommended: it doesn’t need Node.js), then sign in once by running `claude` in a terminal.".into(),
            "Click Connect: Remora writes a first brief to check that everything works.".into(),
        ],
        setup_label: "Install Claude Code".into(),
        setup_url: Some("https://claude.com/product/claude-code".into()),
        egress: Egress {
            hosts: vec![],
            description: "Runs Claude Code on this computer, which sends the titles, contexts, authors and statuses of inbox items to Anthropic.".into(),
            external_ai: true,
        },
        logo: Some("claude".into()),
    }
}

// Errors, as the macOS app words them, for Windows: a terminal, `where.exe`, and the Windows trust rule.
const COULDNT_FIND: &str = "Couldn’t find Claude Code. Install it with Anthropic’s installer, or enter the path to claude (run `where.exe claude` in a terminal).";
const NOT_TRUSTED: &str = "Remora only runs a program named claude.exe or claude.cmd from a folder on this computer, which %@ isn’t.";
const NO_PROGRAM: &str = "There is no program at %@. Run `where.exe claude` in a terminal and paste the path it shows.";
const NOT_CLAUDE_CODE: &str = "%@ isn’t Claude Code. Run `where.exe claude` in a terminal and paste the path it shows.";
const UNEXPECTED: &str = "Claude Code returned an unexpected answer. Run `claude` in a terminal to check it’s signed in.";
const NEEDS_NODE: &str = "Claude Code needs Node.js, which Remora can’t find. Reinstall Claude Code with Anthropic’s installer, which doesn’t need Node.js, or enter the path to claude.";
const NOT_SIGNED_IN: &str = "Claude Code isn’t signed in. Run `claude` in a terminal once to sign in.";
const FAILED: &str = "Claude Code: %@";
const REQUEST_FAILED: &str = "the request failed.";

/// What the program may be called. Anthropic's installer puts a `claude.exe` on Windows; npm puts a `claude.cmd`.
#[cfg(windows)]
pub const PROGRAM_NAMES: &[&str] = &["claude.exe", "claude.cmd"];
#[cfg(not(windows))]
pub const PROGRAM_NAMES: &[&str] = &["claude"];

/// The folders where installs put `claude`, read from the environment (a parameter, so tests can set it).
#[derive(Clone, Debug, Default)]
pub struct SearchEnv {
    pub home: Option<PathBuf>,
    pub app_data: Option<PathBuf>,
    pub local_app_data: Option<PathBuf>,
    pub program_files: Option<PathBuf>,
    /// nvm-windows' link to the active Node.js.
    pub nvm_symlink: Option<PathBuf>,
    pub path: Option<OsString>,
}

impl SearchEnv {
    pub fn current() -> Self {
        let var = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty());
        SearchEnv {
            home: var("USERPROFILE").or_else(|| var("HOME")).map(PathBuf::from),
            app_data: var("APPDATA").map(PathBuf::from),
            local_app_data: var("LOCALAPPDATA").map(PathBuf::from),
            program_files: var("ProgramFiles").map(PathBuf::from),
            nvm_symlink: var("NVM_SYMLINK").map(PathBuf::from),
            path: var("PATH"),
        }
    }

    /// Where Windows installs put `claude`, in order: Anthropic's installer (recommended) and its older local
    /// install, npm's global folder, Volta, Bun, Scoop and mise, the Node.js that nvm-windows or Node's own installer
    /// made active, then the user's PATH (a tray app gets the full one, unlike a Mac app opened from the Dock).
    pub fn windows_folders(&self) -> Vec<PathBuf> {
        let under = |base: &Option<PathBuf>, path: &str| base.as_ref().map(|b| path.split('/').fold(b.clone(), |p, part| p.join(part)));
        let mut folders: Vec<PathBuf> = [
            under(&self.home, ".local/bin"),
            under(&self.home, ".claude/local"),
            under(&self.app_data, "npm"),
            under(&self.local_app_data, "Volta/bin"),
            under(&self.home, ".bun/bin"),
            under(&self.home, "scoop/shims"),
            under(&self.local_app_data, "mise/shims"),
            self.nvm_symlink.clone(),
            under(&self.program_files, "nodejs"),
        ]
        .into_iter()
        .flatten()
        .collect();
        if let Some(path) = &self.path {
            folders.extend(std::env::split_paths(path).filter(|p| p.is_absolute()));
        }
        folders
    }

    /// The macOS app's list, for running the tests and the app on a Mac or Linux: Anthropic's installer and its
    /// older local install, Homebrew, npm's user prefix, volta, bun, mise, asdf, then nvm's folders newest first.
    pub fn unix_folders(&self) -> Vec<PathBuf> {
        let Some(home) = &self.home else { return vec![PathBuf::from("/opt/homebrew/bin"), PathBuf::from("/usr/local/bin")] };
        let mut folders = vec![
            home.join(".local/bin"), home.join(".claude/local"),
            PathBuf::from("/opt/homebrew/bin"), PathBuf::from("/usr/local/bin"),
            home.join(".npm-global/bin"), home.join(".volta/bin"), home.join(".bun/bin"),
            home.join(".local/share/mise/shims"), home.join(".asdf/shims"),
        ];
        folders.extend(nvm_folders(home));
        folders
    }

    /// Every place `claude` may be, first match wins.
    pub fn candidates(&self) -> Vec<PathBuf> {
        let folders = if cfg!(windows) { self.windows_folders() } else { self.unix_folders() };
        folders.iter().flat_map(|folder| PROGRAM_NAMES.iter().map(move |name| folder.join(name))).collect()
    }
}

/// nvm keeps one folder per Node version: the newest first.
pub fn nvm_folders(home: &Path) -> Vec<PathBuf> {
    let root = home.join(".nvm/versions/node");
    let mut versions: Vec<String> = std::fs::read_dir(&root)
        .map(|entries| entries.filter_map(|e| e.ok()?.file_name().into_string().ok()).collect())
        .unwrap_or_default();
    let numbers = |v: &str| v.trim_start_matches('v').split('.').map(|n| n.parse::<u64>().unwrap_or(0)).collect::<Vec<_>>();
    versions.sort_by_key(|v| std::cmp::Reverse(numbers(v)));
    versions.into_iter().map(|v| root.join(v).join("bin")).collect()
}

/// "~/bin/claude" → the home folder's; quotes from Explorer's "Copy as path" dropped.
fn expand(configured: &str, home: Option<&Path>) -> PathBuf {
    let raw = configured.trim().trim_matches('"');
    match (raw.strip_prefix('~'), home) {
        (Some(rest), Some(home)) if rest.is_empty() || rest.starts_with(['/', '\\']) => home.join(rest.trim_start_matches(['/', '\\'])),
        _ => PathBuf::from(raw),
    }
}

/// The `claude` Remora runs: the one entered in the account, else the first found. Only a program it would trust
/// (`is_trustworthy`).
pub fn locate(configured: &str) -> Option<PathBuf> {
    locate_in(configured, &SearchEnv::current())
}

pub fn locate_in(configured: &str, env: &SearchEnv) -> Option<PathBuf> {
    let candidates = if configured.trim().is_empty() { env.candidates() } else { vec![expand(configured, env.home.as_deref())] };
    candidates.into_iter().find(|p| is_trustworthy(p))
}

fn is_program_name(name: &str) -> bool {
    PROGRAM_NAMES.iter().any(|p| if cfg!(windows) { name.eq_ignore_ascii_case(p) } else { name == *p })
}

/// An absolute path to a file named `claude` that only you or the system can change; then `is_claude_code`
/// asks it what it is before it gets any inbox content.
pub fn is_trustworthy(path: &Path) -> bool {
    let named = path.file_name().and_then(|n| n.to_str()).is_some_and(is_program_name);
    named && path.is_absolute() && is_executable_file(path) && only_you_can_change(path)
}

fn is_executable_file(path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else { return false };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        metadata.is_file()
    }
}

/// On macOS and Linux, as in the macOS app: the program, its folder and, through a link, the real file and its
/// folder belong to you or root and aren't writable by everyone.
#[cfg(unix)]
fn only_you_can_change(path: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    let Ok(real) = std::fs::canonicalize(path) else { return false };
    // SAFETY: getuid has no preconditions and cannot fail.
    let me = unsafe { libc::getuid() };
    let safe = |p: Option<&Path>| p.and_then(|p| std::fs::metadata(p).ok()).is_some_and(|m| (m.uid() == me || m.uid() == 0) && m.mode() & 0o002 == 0);
    safe(Some(&real)) && safe(real.parent()) && safe(path.parent())
}

/// On Windows, owners and modes don't say who may change a file: that is the folder's access list, which the
/// standard library can't read. The default ones protect what Remora looks for (your profile, Program Files), so it
/// refuses what they don't cover: a program on a network share, which others may replace. The `--version` check
/// then runs as on macOS.
#[cfg(not(unix))]
fn only_you_can_change(path: &Path) -> bool {
    let real = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    !is_network_path(path) && !is_network_path(&real)
}

/// `\\server\share\…`, in either spelling.
pub fn is_network_path(path: &Path) -> bool {
    matches!(path.components().next(), Some(Component::Prefix(p)) if matches!(p.kind(), Prefix::UNC(..) | Prefix::VerbatimUNC(..)))
}

/// `\\?\C:\…` (what Windows' canonicalize returns) → `C:\…`, which every program understands in PATH.
fn plain(path: PathBuf) -> PathBuf {
    match path.to_str().and_then(|s| s.strip_prefix(r"\\?\")) {
        Some(rest) if !rest.starts_with("UNC\\") => PathBuf::from(rest),
        _ => path,
    }
}

/// Programs that answered `--version` as Claude Code, with the file's date then: checked again after an update.
fn verified() -> &'static Mutex<HashMap<PathBuf, SystemTime>> {
    static VERIFIED: OnceLock<Mutex<HashMap<PathBuf, SystemTime>>> = OnceLock::new();
    VERIFIED.get_or_init(|| Mutex::new(HashMap::new()))
}

/// `claude --version` prints "2.1.295 (Claude Code)".
pub async fn is_claude_code(executable: &Path, runner: &dyn CommandRunner) -> bool {
    let real = std::fs::canonicalize(executable).unwrap_or_else(|_| executable.to_path_buf());
    let modified = std::fs::metadata(&real).and_then(|m| m.modified()).ok();
    if let Some(modified) = modified {
        if verified().lock().is_ok_and(|v| v.get(&real) == Some(&modified)) {
            return true;
        }
    }
    let environment = environment(executable, &std::env::vars().collect());
    let Ok(output) = runner.run(executable, &["--version".into()], Some(&environment), None).await else { return false };
    if !String::from_utf8_lossy(&output).contains("(Claude Code)") {
        return false;
    }
    if let (Some(modified), Ok(mut verified)) = (modified, verified().lock()) {
        verified.insert(real, modified);
    }
    true
}

/// An npm `claude` is a script that needs `node`, which sits next to it (npm, nvm, Volta, Homebrew): `claude`'s
/// folders go first on PATH, then the usual places for Node.js, then the PATH Remora has. Windows names the variable
/// `Path`: its spelling is kept.
pub fn environment(executable: &Path, base: &HashMap<String, String>) -> HashMap<String, String> {
    let key = base.keys().find(|k| k.eq_ignore_ascii_case("PATH")).cloned().unwrap_or_else(|| String::from("PATH"));
    let default = if cfg!(windows) { "" } else { "/usr/bin:/bin:/usr/sbin:/sbin" };
    let current = base.get(&key).map(String::as_str).unwrap_or(default);
    let mut folders: Vec<PathBuf> = [executable.parent().map(Path::to_path_buf), std::fs::canonicalize(executable).ok().and_then(|r| r.parent().map(|p| plain(p.to_path_buf())))]
        .into_iter()
        .flatten()
        .collect();
    if cfg!(windows) {
        let program_files = base.iter().find(|(k, _)| k.eq_ignore_ascii_case("ProgramFiles")).map(|(_, v)| PathBuf::from(v));
        folders.extend(program_files.map(|p| p.join("nodejs")));
    } else {
        folders.extend([PathBuf::from("/opt/homebrew/bin"), PathBuf::from("/usr/local/bin")]);
    }
    folders.extend(std::env::split_paths(current).filter(|p| !p.as_os_str().is_empty()));
    let mut seen = HashSet::new();
    folders.retain(|f| seen.insert(f.clone()));
    let mut environment = base.clone();
    if let Some(joined) = std::env::join_paths(folders).ok().and_then(|p| p.into_string().ok()) {
        environment.insert(key, joined);
    }
    environment
}

/// The arguments and the standard input of one `claude` run.
pub struct Command {
    pub arguments: Vec<String>,
    pub input: Vec<u8>,
}

/// One-shot, no tools, no session saved, and the user's plugins, hooks and MCP servers left out. The message, with
/// the inbox in it, goes on standard input; the command line only holds Remora's own fixed instructions.
pub fn command(message: &str, system: &str, schema: &Schema, model: &str) -> Command {
    let mut arguments: Vec<String> = [
        "--print", "--safe-mode", "--output-format", "json", "--no-session-persistence", "--tools", "",
        "--system-prompt", system, "--json-schema", &schema.json(),
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    if !model.is_empty() {
        arguments.extend(["--model".to_string(), model.to_string()]);
    }
    Command { arguments, input: message.as_bytes().to_vec() }
}

/// The brief's command line.
pub fn arguments(items: &[InboxItem], model: &str, now: DateTime<Utc>, language: &str) -> Vec<String> {
    command(&prompt::message(items, now), &prompt::system(language), &Schema::brief(), model).arguments
}

/// What `claude --print --output-format json` prints.
#[derive(Deserialize)]
struct CliResult<T> {
    is_error: bool,
    result: Option<String>,
    structured_output: Option<T>,
}

fn decode<T: DeserializeOwned>(data: &[u8]) -> Result<T, PluginError> {
    let result: CliResult<T> = serde_json::from_slice(data).map_err(|_| PluginError::Api(UNEXPECTED.into()))?;
    match result.structured_output {
        Some(output) if !result.is_error => Ok(output),
        _ => Err(PluginError::Api(FAILED.replace("%@", result.result.as_deref().unwrap_or(REQUEST_FAILED)))),
    }
}

/// A brief from `claude`'s output, without the items it made up.
pub fn parse(data: &[u8], known_ids: &HashSet<String>) -> Result<Brief, PluginError> {
    decode::<prompt::Output>(data).map(|o| o.brief(known_ids))
}

/// Turns what `claude` printed on failure into something the user can act on.
pub fn explain(message: &str) -> PluginError {
    let lowered = message.to_lowercase();
    // "env: node" (macOS), "'node' is not recognized" (Windows' cmd.exe running npm's claude.cmd).
    if ["env: node", "node: no such file", "node: command not found", "'node' is not recognized"].iter().any(|s| lowered.contains(s)) {
        return PluginError::Api(NEEDS_NODE.into());
    }
    if ["not logged in", "/login", "please log in"].iter().any(|s| lowered.contains(s)) {
        return PluginError::Api(NOT_SIGNED_IN.into());
    }
    PluginError::Api(FAILED.replace("%@", if message.is_empty() { REQUEST_FAILED } else { message }))
}

pub struct ClaudeCodePlugin {
    path: String,
    model: String,
    digest_model: String,
    language: String,
    runner: Arc<dyn CommandRunner>,
}

impl ClaudeCodePlugin {
    pub fn new(config: &PluginConfig, language: &str) -> Self {
        Self::with_runner(config, language, Arc::new(ProcessCommandRunner::default()))
    }

    pub fn with_runner(config: &PluginConfig, language: &str, runner: Arc<dyn CommandRunner>) -> Self {
        let digest_model = Some(config.get("digestModel")).filter(|m| !m.is_empty()).unwrap_or_else(|| DEFAULT_DIGEST_MODEL.into());
        ClaudeCodePlugin { path: config.get("path"), model: config.get("model"), digest_model, language: language.into(), runner }
    }

    async fn run(&self, command: Command) -> Result<Vec<u8>, PluginError> {
        let Some(executable) = locate(&self.path) else {
            let expanded = expand(&self.path, SearchEnv::current().home.as_deref());
            let message = if self.path.is_empty() {
                COULDNT_FIND.to_string()
            } else if is_executable_file(&expanded) {
                NOT_TRUSTED.replace("%@", &self.path)
            } else {
                NO_PROGRAM.replace("%@", &self.path)
            };
            return Err(PluginError::Api(message));
        };
        if !is_claude_code(&executable, self.runner.as_ref()).await {
            return Err(PluginError::Api(NOT_CLAUDE_CODE.replace("%@", &executable.display().to_string())));
        }
        let environment = environment(&executable, &std::env::vars().collect());
        self.runner.run(&executable, &command.arguments, Some(&environment), Some(&command.input)).await.map_err(|error| match error {
            CommandError::Failed(_, message) => explain(&message),
            other => PluginError::Api(other.to_string()),
        })
    }
}

#[async_trait]
impl AssistantPlugin for ClaudeCodePlugin {
    async fn brief(&self, items: &[InboxItem], now: DateTime<Utc>) -> Result<Brief, PluginError> {
        let command = command(&prompt::message(items, now), &prompt::system(&self.language), &Schema::brief(), &self.model);
        parse(&self.run(command).await?, &prompt::ids(items))
    }

    async fn digest(&self, items: &[InboxItem], topic: &str, now: DateTime<Utc>) -> Result<String, PluginError> {
        let command = command(&prompt::digest_message(items, topic, now), &prompt::digest_system(&self.language), &Schema::digest(), &self.digest_model);
        decode::<prompt::DigestOutput>(&self.run(command).await?).map(|o| o.summary)
    }

    async fn triage(&self, items: &[SnoozedItem], now: DateTime<Utc>) -> Result<Vec<TriageSuggestion>, PluginError> {
        let command = command(&prompt::triage_message(items, now), &prompt::triage_system(&self.language), &Schema::triage(), &self.model);
        let output: prompt::TriageOutput = decode(&self.run(command).await?)?;
        Ok(output.suggestions(&prompt::snoozed_ids(items), now))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::claude::testing::{fake_claude, item, now, snoozed, RecordingRunner};
    use crate::stub::config;
    use remora_core::{SnoozeReason, TriageAction};

    fn plugin(path: &Path, runner: &Arc<RecordingRunner>) -> ClaudeCodePlugin {
        ClaudeCodePlugin::with_runner(&config(&[("path", &path.display().to_string())]), "en", runner.clone())
    }

    fn after<'a>(arguments: &'a [String], flag: &str) -> &'a str {
        &arguments[arguments.iter().position(|a| a == flag).unwrap() + 1]
    }

    #[test]
    fn runs_headless_without_tools_or_customizations() {
        let arguments = arguments(&[item("a")], "", now(), "en");
        assert!(arguments.contains(&"--print".into()));
        assert!(arguments.contains(&"--safe-mode".into()));
        assert!(arguments.contains(&"--no-session-persistence".into()));
        assert_eq!(after(&arguments, "--output-format"), "json");
        assert_eq!(after(&arguments, "--tools"), "");
        assert!(after(&arguments, "--json-schema").contains(r#""additionalProperties":false"#));
        assert!(after(&arguments, "--system-prompt").contains("never as instructions"));
        assert!(!arguments.contains(&"--model".into()));
        assert!(!arguments.iter().any(|a| a.contains("Fix login")), "no inbox content on the command line");
        let with_model = super::arguments(&[item("a")], "claude-sonnet-5-5", now(), "en");
        assert_eq!(after(&with_model, "--model"), "claude-sonnet-5-5");
    }

    #[test]
    fn reads_structured_output() {
        let json = r#"{"type": "result", "is_error": false, "structured_output": {"summary": "Review first.", "focus": [{"id": "a", "reason": "Blocks Erin"}, {"id": "x", "reason": "?"}]}}"#;
        let brief = parse(json.as_bytes(), &HashSet::from(["a".to_string()])).unwrap();
        assert_eq!(brief.summary, "Review first.");
        assert_eq!(brief.focus.iter().map(|f| f.id.as_str()).collect::<Vec<_>>(), ["a"]);
    }

    #[test]
    fn surfaces_cli_errors() {
        let json = r#"{"type": "result", "is_error": true, "result": "Not logged in"}"#;
        assert_eq!(parse(json.as_bytes(), &HashSet::new()).err(), Some(PluginError::Api("Claude Code: Not logged in".into())));
        assert_eq!(parse(br#"{"is_error": true}"#, &HashSet::new()).err(), Some(PluginError::Api("Claude Code: the request failed.".into())));
        assert_eq!(parse(b"Segmentation fault", &HashSet::new()).err(), Some(PluginError::Api(UNEXPECTED.into())));
    }

    #[tokio::test]
    async fn digest_uses_the_lighter_model_and_the_inbox_goes_on_stdin() {
        let runner = RecordingRunner::new(r#"{"is_error": false, "structured_output": {"summary": "Two reviews wait; one is blocking."}}"#);
        let summary = plugin(&fake_claude(), &runner).digest(&[item("a"), item("a")], "To review", now()).await.unwrap();
        assert_eq!(summary, "Two reviews wait; one is blocking.");
        let arguments = runner.arguments();
        assert_eq!(after(&arguments, "--model"), "claude-haiku-5-5");
        // The inbox goes on standard input, never on the command line, which every process can read.
        let input = runner.input();
        assert!(input.starts_with("Group: To review."));
        assert!(input.contains("Fix login"));
        assert!(!arguments.iter().any(|a| a.contains("Fix login")), "no title on the command line");
    }

    #[tokio::test]
    async fn the_brief_drops_unknown_items() {
        let runner = RecordingRunner::new(r#"{"is_error": false, "structured_output": {"summary": "s", "focus": [{"id": "a", "reason": "r"}, {"id": "ghost", "reason": "?"}]}}"#);
        let brief = plugin(&fake_claude(), &runner).brief(&[item("a")], now()).await.unwrap();
        assert_eq!(brief.focus.len(), 1);
        assert!(runner.input().contains("<inbox_items>"));
    }

    #[tokio::test]
    async fn triage_parses_suggestions_and_drops_invalid_ones() {
        let later = prompt::iso8601(now() + chrono::Duration::days(1));
        let output = format!(
            r#"{{"is_error": false, "structured_output": {{"suggestions": [
              {{"id": "a", "action": "reschedule", "until": "{later}", "reason": "After Erin's review"}},
              {{"id": "b", "action": "done", "until": "", "reason": "Merged elsewhere"}},
              {{"id": "c", "action": "reschedule", "until": "yesterday", "reason": "bad date"}},
              {{"id": "d", "action": "snooze", "until": "", "reason": "unknown action"}},
              {{"id": "ghost", "action": "now", "until": "", "reason": "unknown item"}}
            ]}}}}"#
        );
        let runner = RecordingRunner::new(&output);
        let items: Vec<SnoozedItem> = ["a", "b", "c", "d"].iter().map(|id| snoozed(item(id), 3)).collect();
        let suggestions = plugin(&fake_claude(), &runner).triage(&items, now()).await.unwrap();
        assert_eq!(suggestions.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(), ["a", "b"]);
        assert_eq!((suggestions[0].action, suggestions[0].until), (TriageAction::Reschedule, prompt::parse_iso8601(&later)));
        assert_eq!((suggestions[1].action, suggestions[1].until), (TriageAction::Done, None));
        let message = runner.input();
        assert!(message.contains(r#""timesSnoozed":3"#) && message.contains(r#""reason":"motivation""#), "{message}");
        assert!(after(&runner.arguments(), "--json-schema").contains(r#""enum":["keep","reschedule","done","now"]"#));
        assert_eq!(items[0].snooze.reason, Some(SnoozeReason::Motivation));
    }

    // Finding and running claude.

    #[tokio::test]
    async fn runs_claude_with_its_own_folder_first_on_path() {
        let runner = RecordingRunner::new(r#"{"is_error": false, "structured_output": {"summary": "ok"}}"#);
        plugin(&fake_claude(), &runner).digest(&[item("a")], "x", now()).await.unwrap();
        let environment = runner.environment();
        let path = environment.iter().find(|(k, _)| k.eq_ignore_ascii_case("PATH")).map(|(_, v)| v.clone()).unwrap();
        assert_eq!(std::env::split_paths(&path).next().as_deref(), fake_claude().parent());
    }

    #[test]
    fn path_puts_the_folder_first_keeps_the_rest_once_and_keeps_its_spelling() {
        let folder = std::env::temp_dir().join("remora-env-test");
        let claude = folder.join(PROGRAM_NAMES[0]);
        let separator = if cfg!(windows) { ";" } else { ":" };
        let current = format!("{}{separator}/usr/bin{separator}/bin", folder.display());
        let key = if cfg!(windows) { "Path" } else { "PATH" };
        let base = HashMap::from([(key.to_string(), current), ("HOME".to_string(), "/Users/x".to_string())]);
        let environment = environment(&claude, &base);
        assert_eq!(environment.keys().filter(|k| k.eq_ignore_ascii_case("PATH")).count(), 1);
        let folders: Vec<PathBuf> = std::env::split_paths(&environment[key]).collect();
        assert_eq!(folders[0], folder);
        assert_eq!(folders.iter().filter(|f| **f == folder).count(), 1);
        assert_eq!(folders[folders.len() - 2..], [PathBuf::from("/usr/bin"), PathBuf::from("/bin")]);
        assert_eq!(environment["HOME"], "/Users/x");
    }

    /// An npm `claude` is a link into node_modules and starts with `env node`: Node sits next to the link.
    #[cfg(unix)]
    #[test]
    fn path_puts_the_link_folder_and_its_target_first() {
        let root = std::env::temp_dir().join(format!("remora-link-{}", std::process::id()));
        let bin = root.join("v22/bin");
        let package = root.join("v22/lib/node_modules/claude-code");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::create_dir_all(&package).unwrap();
        std::fs::write(package.join("cli.js"), "").unwrap();
        let _ = std::fs::remove_file(bin.join("claude"));
        std::os::unix::fs::symlink(package.join("cli.js"), bin.join("claude")).unwrap();
        let base = HashMap::from([("PATH".to_string(), "/usr/bin:/bin:/opt/homebrew/bin".to_string())]);
        let environment = environment(&bin.join("claude"), &base);
        let folders: Vec<PathBuf> = std::env::split_paths(&environment["PATH"]).collect();
        std::fs::remove_dir_all(&root).unwrap();
        assert_eq!(folders[0], bin);
        assert!(folders.iter().any(|f| f.ends_with("lib/node_modules/claude-code")));
        assert_eq!(folders.iter().filter(|f| *f == Path::new("/opt/homebrew/bin")).count(), 1);
        assert_eq!(folders[folders.len() - 2..], [PathBuf::from("/usr/bin"), PathBuf::from("/bin")]);
    }

    #[test]
    fn nvm_versions_are_searched_newest_first() {
        let home = std::env::temp_dir().join(format!("remora-nvm-{}", std::process::id()));
        for version in ["v9.11.2", "v22.12.0", "v18.20.4"] {
            std::fs::create_dir_all(home.join(".nvm/versions/node").join(version).join("bin")).unwrap();
        }
        let versions: Vec<String> = nvm_folders(&home).iter().map(|p| p.parent().unwrap().file_name().unwrap().to_string_lossy().into_owned()).collect();
        std::fs::remove_dir_all(&home).unwrap();
        assert_eq!(versions, ["v22.12.0", "v18.20.4", "v9.11.2"]);
    }

    #[test]
    fn windows_installs_are_searched_installer_first_then_path() {
        let env = SearchEnv {
            home: Some(PathBuf::from("/U/ada")),
            app_data: Some(PathBuf::from("/U/ada/AppData/Roaming")),
            local_app_data: Some(PathBuf::from("/U/ada/AppData/Local")),
            program_files: Some(PathBuf::from("/PF")),
            nvm_symlink: None,
            path: Some(std::env::join_paths(["/tools", "relative"].map(PathBuf::from)).unwrap()),
        };
        let folders = env.windows_folders();
        assert_eq!(folders[0], Path::new("/U/ada").join(".local").join("bin"), "Anthropic's installer first");
        assert!(folders.contains(&Path::new("/U/ada/AppData/Roaming").join("npm")));
        assert!(folders.contains(&Path::new("/PF").join("nodejs")));
        assert_eq!(folders.last(), Some(&PathBuf::from("/tools")), "PATH last, relative folders left out");
        assert!(SearchEnv::default().windows_folders().is_empty());
    }

    #[test]
    fn a_configured_path_is_expanded() {
        let home = Path::new("/Users/ada");
        assert_eq!(expand("~/bin/claude", Some(home)), home.join("bin/claude"));
        assert_eq!(expand("\"/opt/claude\"", Some(home)), PathBuf::from("/opt/claude"));
        assert_eq!(expand("~bob/claude", Some(home)), PathBuf::from("~bob/claude"));
    }

    #[test]
    fn network_shares_are_recognised() {
        assert!(!is_network_path(Path::new("/usr/bin/claude")));
        #[cfg(windows)]
        {
            assert!(is_network_path(Path::new(r"\\server\share\claude.exe")));
            assert!(is_network_path(Path::new(r"\\?\UNC\server\share\claude.exe")));
            assert!(!is_network_path(Path::new(r"C:\Users\ada\.local\bin\claude.exe")));
            assert_eq!(plain(PathBuf::from(r"\\?\C:\Users")), PathBuf::from(r"C:\Users"));
        }
    }

    #[tokio::test]
    async fn a_missing_claude_says_how_to_fix_it() {
        let runner = RecordingRunner::new("");
        let result = plugin(Path::new("/nowhere/claude"), &runner).digest(&[item("a")], "x", now()).await;
        assert_eq!(result.err(), Some(PluginError::Api(NO_PROGRAM.replace("%@", "/nowhere/claude"))));
        assert_eq!(locate(&fake_claude().display().to_string()), Some(fake_claude()));
        assert!(runner.arguments().is_empty());
    }

    // Only Claude Code.

    #[cfg(unix)]
    #[test]
    fn only_a_program_named_claude_that_only_you_can_change() {
        use crate::claude::testing::make_program;
        assert!(!is_trustworthy(Path::new("/bin/echo")), "not named claude");
        assert!(is_trustworthy(&fake_claude()));
        assert!(!is_trustworthy(Path::new("claude")), "not a relative path");
        assert!(!is_trustworthy(&make_program("claude", 0o777, 0o755)), "anyone can change it");
        assert!(!is_trustworthy(&make_program("claude", 0o755, 0o777)), "anyone can replace it");
        assert!(!is_trustworthy(&make_program("claude", 0o644, 0o755)), "not a program");
    }

    #[tokio::test]
    async fn a_program_that_isnt_claude_code_gets_nothing() {
        let runner = RecordingRunner::with_version("{}", "echo 1.0");
        let impostor = crate::claude::testing::make_program(PROGRAM_NAMES[0], 0o755, 0o755);
        let result = plugin(&impostor, &runner).digest(&[item("a")], "x", now()).await;
        assert_eq!(result.err(), Some(PluginError::Api(NOT_CLAUDE_CODE.replace("%@", &impostor.display().to_string()))));
        assert!(runner.arguments().is_empty(), "no inbox content was passed");
        assert!(runner.input().is_empty());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_program_not_named_claude_is_refused() {
        let runner = RecordingRunner::new("{}");
        let result = plugin(Path::new("/bin/echo"), &runner).digest(&[item("a")], "x", now()).await;
        assert_eq!(result.err(), Some(PluginError::Api(NOT_TRUSTED.replace("%@", "/bin/echo"))));
    }

    #[test]
    fn failures_are_explained() {
        assert_eq!(explain("env: node: No such file or directory"), PluginError::Api(NEEDS_NODE.into()));
        assert_eq!(explain("'node' is not recognized as an internal or external command"), PluginError::Api(NEEDS_NODE.into()));
        assert_eq!(explain("Invalid API key · Please run /login"), PluginError::Api(NOT_SIGNED_IN.into()));
        assert_eq!(explain("Error: rate limited"), PluginError::Api("Claude Code: Error: rate limited".into()));
        assert_eq!(explain(""), PluginError::Api("Claude Code: the request failed.".into()));
    }

    #[tokio::test]
    async fn a_failing_run_is_explained() {
        let runner = RecordingRunner::failing("Not logged in · Please run /login");
        let result = plugin(&fake_claude(), &runner).digest(&[item("a")], "x", now()).await;
        assert_eq!(result.err(), Some(PluginError::Api(NOT_SIGNED_IN.into())));
    }

    #[test]
    fn manifest_matches_the_macos_app() {
        let manifest = manifest();
        assert_eq!(manifest.id, "claude-code");
        assert!(manifest.egress.external_ai && manifest.egress.hosts.is_empty());
        assert_eq!(manifest.fields.iter().map(|f| f.key.as_str()).collect::<Vec<_>>(), ["path", "model", "digestModel"]);
        assert_eq!(manifest.fields[2].default_value, "claude-haiku-5-5");
        assert!(manifest.fields[0].is_optional && manifest.fields[1].is_optional && !manifest.fields[2].is_optional);
    }
}

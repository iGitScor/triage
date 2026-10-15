//! The brief through the Claude Code already signed in on this computer: no API key needed. A port of the macOS
//! `ClaudeCodePlugin.swift`.

use super::prompt::{self, Schema};
use super::runner::{CommandError, CommandRunner, ProcessCommandRunner};
use crate::assistant::{decode_reply, AssistantFailure, AssistantReply};
use crate::{AssistantPlugin, PluginConfig, PluginError};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use remora_core::{Brief, ConfigField, Egress, InboxItem, PluginManifest, SnoozedItem, TokenUsage, TriageSuggestion};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf, Prefix};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::SystemTime;

pub const ID: &str = "claude-code";
/// The same as the API's (one constant per role, AI-02).
pub const DEFAULT_DIGEST_MODEL: &str = super::api::DEFAULT_DIGEST_MODEL;

/// The same words as the macOS app, which the shared French dictionary translates.
pub fn manifest() -> PluginManifest {
    let field = |key: &str, label: &str| ConfigField {
        key: key.into(),
        label: label.into(),
        placeholder: String::new(),
        default_value: String::new(),
        is_secret: false,
        is_optional: false,
        help: None,
        managed_key: None,
        is_toggle: false,
    };
    PluginManifest {
        id: ID.into(),
        name: "Claude Code".into(),
        summary: "The brief through the Claude Code signed in on this computer, on your Claude plan. No API key.".into(),
        fields: vec![
            ConfigField {
                placeholder: "Found automatically".into(),
                is_optional: true,
                help: Some(PATH_HELP.into()),
                managed_key: None,
                is_toggle: false,
                ..field("path", "Path to claude")
            },
            ConfigField { placeholder: "Claude Code’s default".into(), is_optional: true, ..field("model", "Model for the brief") },
            ConfigField {
                default_value: DEFAULT_DIGEST_MODEL.into(),
                help: Some("A lighter model is enough for two sentences and uses less of your plan.".into()),
                managed_key: None,
                is_toggle: false,
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
        sign_in: None,
    }
}

// Errors, as the macOS app words them, for each system: Windows has `where.exe` and its trust rule; Linux (and a
// Mac while developing) `which` and a program named claude.
#[cfg(windows)]
mod wording {
    pub const PATH_HELP: &str =
        "Only if Claude Code is installed somewhere unusual (run `where.exe claude` in a terminal).";
    pub const COULDNT_FIND: &str = "Couldn’t find Claude Code. Install it with Anthropic’s installer, or enter the path to claude (run `where.exe claude` in a terminal).";
    pub const NOT_TRUSTED: &str =
        "Remora only runs a program named claude.exe or claude.cmd that only you or the system can change, which %@ isn’t.";
    pub const NO_PROGRAM: &str =
        "There is no program at %@. Run `where.exe claude` in a terminal and paste the path it shows.";
    pub const NOT_CLAUDE_CODE: &str =
        "%@ isn’t Claude Code. Run `where.exe claude` in a terminal and paste the path it shows.";
}
#[cfg(not(windows))]
mod wording {
    pub const PATH_HELP: &str =
        "Only if Claude Code is installed somewhere unusual (run `which claude` in a terminal).";
    pub const COULDNT_FIND: &str = "Couldn’t find Claude Code. Install it with Anthropic’s installer, or enter the path to claude (run `which claude` in a terminal).";
    pub const NOT_TRUSTED: &str =
        "Remora only runs a program named claude that you or the system own, from a folder on this computer, which %@ isn’t.";
    pub const NO_PROGRAM: &str =
        "There is no program at %@. Run `which claude` in a terminal and paste the path it shows.";
    pub const NOT_CLAUDE_CODE: &str =
        "%@ isn’t Claude Code. Run `which claude` in a terminal and paste the path it shows.";
}
use wording::{COULDNT_FIND, NOT_CLAUDE_CODE, NOT_TRUSTED, NO_PROGRAM, PATH_HELP};
const UNEXPECTED: &str =
    "Claude Code returned an unexpected answer. Run `claude` in a terminal to check it’s signed in.";
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
    pub program_files_x86: Option<PathBuf>,
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
            program_files_x86: var("ProgramFiles(x86)").map(PathBuf::from),
            nvm_symlink: var("NVM_SYMLINK").map(PathBuf::from),
            path: var("PATH"),
        }
    }

    /// Where Windows installs put `claude`, in order: Anthropic's installer (recommended) and its older local
    /// install, npm's global folder, Volta, Bun, Scoop and mise, the Node.js that nvm-windows or Node's own installer
    /// made active, then the user's PATH (a tray app gets the full one, unlike a Mac app opened from the Dock), only its
    /// folders in your profile or Program Files (SEC-30).
    pub fn windows_folders(&self) -> Vec<PathBuf> {
        let under = |base: &Option<PathBuf>, path: &str| {
            base.as_ref().map(|b| path.split('/').fold(b.clone(), |p, part| p.join(part)))
        };
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
            folders.extend(std::env::split_paths(path).filter(|p| p.is_absolute() && self.may_search(p)));
        }
        folders
    }

    /// A PATH folder Windows searches: in your profile or Program Files, which other users can't write to by default,
    /// unlike `C:\tools` or some all-users installs.
    pub fn may_search(&self, folder: &Path) -> bool {
        [&self.home, &self.program_files, &self.program_files_x86]
            .into_iter()
            .flatten()
            .any(|base| is_inside(folder, base))
    }

    /// The macOS app's list, for running the tests and the app on a Mac or Linux: Anthropic's installer and its
    /// older local install, Homebrew, npm's user prefix, volta, bun, mise, asdf, then nvm's folders newest first.
    pub fn unix_folders(&self) -> Vec<PathBuf> {
        let Some(home) = &self.home else {
            return vec![PathBuf::from("/opt/homebrew/bin"), PathBuf::from("/usr/local/bin")];
        };
        let mut folders = vec![
            home.join(".local/bin"),
            home.join(".claude/local"),
            PathBuf::from("/opt/homebrew/bin"),
            PathBuf::from("/usr/local/bin"),
            home.join(".npm-global/bin"),
            home.join(".volta/bin"),
            home.join(".bun/bin"),
            home.join(".local/share/mise/shims"),
            home.join(".asdf/shims"),
        ];
        folders.extend(nvm_folders(home));
        folders
    }

    /// Linux: the macOS list, then Linuxbrew, the system's own `/usr/bin` (a distribution's package) and the session's
    /// PATH, which a Linux desktop app inherits (a custom npm prefix, Nix). Each still has to be trustworthy.
    pub fn linux_folders(&self) -> Vec<PathBuf> {
        let mut folders = self.unix_folders();
        folders.push(PathBuf::from("/home/linuxbrew/.linuxbrew/bin"));
        if let Some(home) = &self.home {
            folders.push(home.join(".linuxbrew/bin"));
        }
        folders.push(PathBuf::from("/usr/bin"));
        if let Some(path) = &self.path {
            folders.extend(std::env::split_paths(path).filter(|p| p.is_absolute()));
        }
        let mut seen = std::collections::HashSet::new();
        folders.retain(|folder| seen.insert(folder.clone()));
        folders
    }

    /// Every place `claude` may be, first match wins.
    pub fn candidates(&self) -> Vec<PathBuf> {
        let folders = if cfg!(windows) {
            self.windows_folders()
        } else if cfg!(target_os = "linux") {
            self.linux_folders()
        } else {
            self.unix_folders()
        };
        folders.iter().flat_map(|folder| PROGRAM_NAMES.iter().map(move |name| folder.join(name))).collect()
    }
}

/// nvm keeps one folder per Node version: the newest first.
pub fn nvm_folders(home: &Path) -> Vec<PathBuf> {
    let root = home.join(".nvm/versions/node");
    let mut versions: Vec<String> = std::fs::read_dir(&root)
        .map(|entries| entries.filter_map(|e| e.ok()?.file_name().into_string().ok()).collect())
        .unwrap_or_default();
    let numbers =
        |v: &str| v.trim_start_matches('v').split('.').map(|n| n.parse::<u64>().unwrap_or(0)).collect::<Vec<_>>();
    versions.sort_by_key(|v| std::cmp::Reverse(numbers(v)));
    versions.into_iter().map(|v| root.join(v).join("bin")).collect()
}

/// `folder` is `base` or inside it, ignoring case as Windows does; a `..` is refused rather than resolved.
pub fn is_inside(folder: &Path, base: &Path) -> bool {
    let parts = |p: &Path| -> Option<Vec<String>> {
        p.components()
            .filter(|c| !matches!(c, Component::CurDir))
            .map(|c| match c {
                Component::ParentDir => None,
                c => Some(c.as_os_str().to_string_lossy().to_lowercase()),
            })
            .collect()
    };
    match (parts(folder), parts(base)) {
        (Some(folder), Some(base)) => !base.is_empty() && folder.starts_with(&base),
        _ => false,
    }
}

/// "~/bin/claude" → the home folder's; quotes from Explorer's "Copy as path" dropped.
fn expand(configured: &str, home: Option<&Path>) -> PathBuf {
    let raw = configured.trim().trim_matches('"');
    match (raw.strip_prefix('~'), home) {
        (Some(rest), Some(home)) if rest.is_empty() || rest.starts_with(['/', '\\']) => {
            home.join(rest.trim_start_matches(['/', '\\']))
        }
        _ => PathBuf::from(raw),
    }
}

/// The `claude` Remora runs: the one entered in the account, else the first found. Only a program it would trust
/// (`is_trustworthy`).
pub fn locate(configured: &str) -> Option<PathBuf> {
    locate_in(configured, &SearchEnv::current())
}

pub fn locate_in(configured: &str, env: &SearchEnv) -> Option<PathBuf> {
    let candidates =
        if configured.trim().is_empty() { env.candidates() } else { vec![expand(configured, env.home.as_deref())] };
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
    let safe = |p: Option<&Path>| {
        p.and_then(|p| std::fs::metadata(p).ok())
            .is_some_and(|m| (m.uid() == me || m.uid() == 0) && m.mode() & 0o002 == 0)
    };
    safe(Some(&real)) && safe(real.parent()) && safe(path.parent())
}

/// On Windows, owners and modes don't say who may change a file: access lists do. As on macOS, the program, its
/// folder and, through a link, the real file and its folder must be owned by and writable only by you or the system
/// (SEC-30), and not on a network share. The `--version` check then runs as on macOS.
#[cfg(windows)]
fn only_you_can_change(path: &Path) -> bool {
    let Ok(real) = std::fs::canonicalize(path).map(plain) else { return false };
    if is_network_path(path) || is_network_path(&real) {
        return false;
    }
    let Some(me) = acl::current_user() else { return false };
    let safe = |p: Option<&Path>| {
        p.and_then(acl::read).is_some_and(|(owner, entries)| only_trusted_can_write(&owner, &entries, &me))
    };
    safe(Some(&real)) && safe(real.parent()) && safe(path.parent())
}

/// SYSTEM, Administrators and TrustedInstaller, which own and change what Windows and installers put in Program Files.
const SYSTEM_SIDS: &[&str] =
    &["S-1-5-18", "S-1-5-32-544", "S-1-5-80-956008885-3418522649-1831038044-1853292631-2271478464"];

/// The rights that let someone replace a program or put another next to it: write or append data (in a folder, add a
/// file or a folder), delete it or what the folder holds, change its access list or owner, and the generic ones.
pub const WRITE_RIGHTS: u32 =
    0x0000_0002 | 0x0000_0004 | 0x0000_0040 | 0x0001_0000 | 0x0004_0000 | 0x0008_0000 | 0x1000_0000 | 0x4000_0000;

/// One entry of an access list: whom it names (a SID such as "S-1-5-32-545"), whether it allows or denies, the rights,
/// and whether it only applies to what is created inside (CREATOR OWNER's, for one).
#[derive(Clone, Debug, PartialEq)]
pub struct AccessEntry {
    pub sid: String,
    pub allows: bool,
    pub mask: u32,
    pub inherit_only: bool,
}

/// You, SYSTEM, Administrators or TrustedInstaller.
pub fn is_you_or_the_system(sid: &str, me: &str) -> bool {
    sid.eq_ignore_ascii_case(me) || SYSTEM_SIDS.iter().any(|s| sid.eq_ignore_ascii_case(s))
}

/// The owner, who may always change the access list, and everyone an entry lets write are you or the system.
/// Denials are ignored, which can only refuse more.
pub fn only_trusted_can_write(owner: &str, entries: &[AccessEntry], me: &str) -> bool {
    is_you_or_the_system(owner, me)
        && entries
            .iter()
            .filter(|e| e.allows && !e.inherit_only && e.mask & WRITE_RIGHTS != 0)
            .all(|e| is_you_or_the_system(&e.sid, me))
}

/// Reading the access lists, which only Windows has.
#[cfg(windows)]
mod acl {
    use super::AccessEntry;
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;
    use windows_sys::Win32::Foundation::{CloseHandle, LocalFree, ERROR_SUCCESS, HANDLE};
    use windows_sys::Win32::Security::Authorization::{ConvertSidToStringSidW, GetNamedSecurityInfoW, SE_FILE_OBJECT};
    use windows_sys::Win32::Security::{
        AclSizeInformation, GetAce, GetAclInformation, GetTokenInformation, TokenUser, ACCESS_ALLOWED_ACE, ACE_HEADER,
        ACL, ACL_SIZE_INFORMATION, DACL_SECURITY_INFORMATION, INHERIT_ONLY_ACE, OWNER_SECURITY_INFORMATION,
        PSECURITY_DESCRIPTOR, PSID, TOKEN_QUERY, TOKEN_USER,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    const ALLOWED: u8 = 0;
    /// Denied, denied object, denied callback and denied callback object entries.
    const DENIED: [u8; 4] = [1, 6, 10, 12];

    /// "S-1-5-21-…".
    ///
    /// SAFETY: `sid` points to a valid SID.
    unsafe fn sid_string(sid: PSID) -> Option<String> {
        let mut text: *mut u16 = std::ptr::null_mut();
        // SAFETY: Windows allocates `text`, freed below.
        if unsafe { ConvertSidToStringSidW(sid, &mut text) } == 0 || text.is_null() {
            return None;
        }
        // SAFETY: a nul-terminated string.
        let length = (0..).take_while(|&i| unsafe { *text.add(i) } != 0).count();
        let result = String::from_utf16(unsafe { std::slice::from_raw_parts(text, length) }).ok();
        unsafe { LocalFree(text.cast()) };
        result
    }

    /// The SID of the user Remora runs as.
    pub fn current_user() -> Option<String> {
        let mut token: HANDLE = std::ptr::null_mut();
        // SAFETY: this process's pseudo handle; the token is closed below.
        if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
            return None;
        }
        let mut size = 0u32;
        // SAFETY: the first call asks for the size, the second fills a buffer that big, aligned for TOKEN_USER.
        unsafe { GetTokenInformation(token, TokenUser, std::ptr::null_mut(), 0, &mut size) };
        let mut buffer = vec![0u64; (size as usize).div_ceil(8).max(1)];
        let filled = unsafe { GetTokenInformation(token, TokenUser, buffer.as_mut_ptr().cast(), size, &mut size) };
        let sid = (filled != 0).then(|| unsafe { sid_string((*buffer.as_ptr().cast::<TOKEN_USER>()).User.Sid) });
        unsafe { CloseHandle(token) };
        sid.flatten()
    }

    /// The owner and the entries of the access list; nothing if it can't be read, or if there is no list, which lets
    /// everyone in.
    pub fn read(path: &Path) -> Option<(String, Vec<AccessEntry>)> {
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let mut owner: PSID = std::ptr::null_mut();
        let mut dacl: *mut ACL = std::ptr::null_mut();
        let mut descriptor: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
        // SAFETY: a nul-terminated path; `owner` and `dacl` point into `descriptor`, freed below.
        let status = unsafe {
            GetNamedSecurityInfoW(
                wide.as_ptr(),
                SE_FILE_OBJECT,
                OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
                &mut owner,
                std::ptr::null_mut(),
                &mut dacl,
                std::ptr::null_mut(),
                &mut descriptor,
            )
        };
        if status != ERROR_SUCCESS {
            return None;
        }
        // SAFETY: both stay valid until the descriptor is freed.
        let result = unsafe { entries(owner, dacl) };
        unsafe { LocalFree(descriptor.cast()) };
        result
    }

    /// SAFETY: `owner` is a valid SID or null, `dacl` a valid access list or null.
    unsafe fn entries(owner: PSID, dacl: *const ACL) -> Option<(String, Vec<AccessEntry>)> {
        if owner.is_null() || dacl.is_null() {
            return None;
        }
        let owner = unsafe { sid_string(owner) }?;
        let mut info: ACL_SIZE_INFORMATION = unsafe { std::mem::zeroed() };
        let size = std::mem::size_of::<ACL_SIZE_INFORMATION>() as u32;
        let info_ptr = (&mut info as *mut ACL_SIZE_INFORMATION).cast();
        if unsafe { GetAclInformation(dacl, info_ptr, size, AclSizeInformation) } == 0 {
            return None;
        }
        let mut entries = Vec::new();
        for index in 0..info.AceCount {
            let mut ace: *mut core::ffi::c_void = std::ptr::null_mut();
            if unsafe { GetAce(dacl, index, &mut ace) } == 0 {
                return None;
            }
            let header = unsafe { &*ace.cast::<ACE_HEADER>() };
            let inherit_only = u32::from(header.AceFlags) & INHERIT_ONLY_ACE != 0;
            if header.AceType == ALLOWED || header.AceType == DENIED[0] {
                // Allowed and denied entries share a layout: the header, the rights, then the SID.
                let entry = ace.cast::<ACCESS_ALLOWED_ACE>();
                let sid = unsafe { std::ptr::addr_of_mut!((*entry).SidStart) }.cast();
                entries.push(AccessEntry {
                    sid: unsafe { sid_string(sid) }?,
                    allows: header.AceType == ALLOWED,
                    mask: unsafe { (*entry).Mask },
                    inherit_only,
                });
            } else if !inherit_only && !DENIED.contains(&header.AceType) {
                // Another kind of entry that allows something (object, callback, compound): not used on files.
                return None;
            }
        }
        Some((owner, entries))
    }
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
    let Ok(output) = runner.run(executable, &["--version".into()], Some(&environment), None).await else {
        return false;
    };
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
    let mut folders: Vec<PathBuf> = [
        executable.parent().map(Path::to_path_buf),
        std::fs::canonicalize(executable).ok().and_then(|r| r.parent().map(|p| plain(p.to_path_buf()))),
    ]
    .into_iter()
    .flatten()
    .collect();
    if cfg!(windows) {
        let program_files =
            base.iter().find(|(k, _)| k.eq_ignore_ascii_case("ProgramFiles")).map(|(_, v)| PathBuf::from(v));
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
        "--print",
        "--safe-mode",
        "--output-format",
        "json",
        "--no-session-persistence",
        "--tools",
        "",
        "--system-prompt",
        system,
        "--json-schema",
        &schema.json(),
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    if !model.is_empty() {
        arguments.extend(["--model".to_string(), model.to_string()]);
    }
    // As the API: a brief needs no long thinking, and the user's own (often high) effort would spend their plan and
    // can run past the time limit (AI-25).
    arguments.extend(EFFORT.iter().map(|s| s.to_string()));
    Command { arguments, input: message.as_bytes().to_vec() }
}

const EFFORT: [&str; 2] = ["--effort", "low"];

/// An older `claude` that doesn't know `--effort` says "unknown option '--effort'": it runs again without.
pub fn rejects_effort(message: &str) -> bool {
    let lowered = message.to_lowercase();
    lowered.contains("unknown option") && lowered.contains("--effort")
}

fn without_effort(arguments: &[String]) -> Vec<String> {
    let mut kept = vec![];
    let mut iter = arguments.iter();
    while let Some(argument) = iter.next() {
        if argument == EFFORT[0] {
            iter.next();
        } else {
            kept.push(argument.clone());
        }
    }
    kept
}

/// The brief's command line.
pub fn arguments(items: &[InboxItem], model: &str, now: DateTime<Utc>, language: &str) -> Vec<String> {
    command(&prompt::message(items, now), &prompt::system(language), &Schema::brief(), model).arguments
}

/// What `claude --print --output-format json` prints.
#[derive(Deserialize)]
struct CliResult {
    is_error: bool,
    /// On an error: `error_max_structured_output_retries`, `error_max_turns`, `error_max_budget_usd`,
    /// `error_during_execution`.
    subtype: Option<String>,
    result: Option<String>,
    structured_output: Option<serde_json::Value>,
    /// What it used, as the Messages API words it.
    usage: Option<super::api::ApiUsage>,
}

/// Claude Code's reply: its structured output, or the kind of error it reports.
impl AssistantReply for CliResult {
    fn answer(self) -> Result<serde_json::Value, AssistantFailure> {
        if !self.is_error {
            return self.structured_output.ok_or(AssistantFailure::UnexpectedAnswer);
        }
        Err(match self.subtype.as_deref() {
            Some("error_max_structured_output_retries") => AssistantFailure::NoStructuredAnswer,
            Some("error_max_turns") => AssistantFailure::TooManySteps,
            Some("error_max_budget_usd") => AssistantFailure::SpendingLimit,
            _ => failure(self.result.as_deref().unwrap_or_default()),
        })
    }

    fn usage(&self) -> Option<TokenUsage> {
        self.usage.as_ref().map(super::api::ApiUsage::tokens)
    }
}

/// Output that isn't Claude Code's JSON at all comes from the program, not the model: it keeps its own hint.
fn decode<T: DeserializeOwned>(data: &[u8]) -> Result<T, PluginError> {
    let result: CliResult = serde_json::from_slice(data).map_err(|_| PluginError::Api(UNEXPECTED.into()))?;
    Ok(decode_reply(result)?)
}

/// What Claude Code said went wrong, worded by `explanation`. Signing in is Claude Code's, so it stays here rather
/// than in the provider-neutral `AssistantFailure`, as on macOS.
fn failure(message: &str) -> AssistantFailure {
    AssistantFailure::Failed(explanation(message))
}

/// A brief from `claude`'s output, without the items it made up.
pub fn parse(data: &[u8], known_ids: &HashSet<String>) -> Result<Brief, PluginError> {
    decode::<prompt::Output>(data).map(|o| o.brief(known_ids))
}

/// Turns what `claude` printed on failure into something the user can act on.
pub fn explain(message: &str) -> PluginError {
    PluginError::Api(explanation(message))
}

/// What `claude` printed on failure, worded for the user: Node.js missing, not signed in, or its own message.
fn explanation(message: &str) -> String {
    let lowered = message.to_lowercase();
    // "env: node" (macOS), "'node' is not recognized" (Windows' cmd.exe running npm's claude.cmd).
    if ["env: node", "node: no such file", "node: command not found", "'node' is not recognized"]
        .iter()
        .any(|s| lowered.contains(s))
    {
        return NEEDS_NODE.into();
    }
    if ["not logged in", "/login", "please log in"].iter().any(|s| lowered.contains(s)) {
        return NOT_SIGNED_IN.into();
    }
    FAILED.replace("%@", if message.is_empty() { REQUEST_FAILED } else { message })
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
        let digest_model =
            Some(config.get("digestModel")).filter(|m| !m.is_empty()).unwrap_or_else(|| DEFAULT_DIGEST_MODEL.into());
        ClaudeCodePlugin {
            path: config.get("path"),
            model: config.get("model"),
            digest_model,
            language: language.into(),
            runner,
        }
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
        let mut result =
            self.runner.run(&executable, &command.arguments, Some(&environment), Some(&command.input)).await;
        if matches!(&result, Err(CommandError::Failed(_, message)) if rejects_effort(message)) {
            let arguments = without_effort(&command.arguments);
            result = self.runner.run(&executable, &arguments, Some(&environment), Some(&command.input)).await;
        }
        result.map_err(|error| match error {
            CommandError::Failed(_, message) => explain(&message),
            other => PluginError::Api(other.to_string()),
        })
    }
}

#[async_trait]
impl AssistantPlugin for ClaudeCodePlugin {
    async fn brief(&self, items: &[InboxItem], now: DateTime<Utc>) -> Result<Brief, PluginError> {
        let command =
            command(&prompt::message(items, now), &prompt::system(&self.language), &Schema::brief(), &self.model);
        parse(&self.run(command).await?, &prompt::ids(items))
    }

    async fn digest(&self, items: &[InboxItem], topic: &str, now: DateTime<Utc>) -> Result<String, PluginError> {
        let command = command(
            &prompt::digest_message(items, topic, now),
            &prompt::digest_system(&self.language),
            &Schema::digest(),
            &self.digest_model,
        );
        decode::<prompt::DigestOutput>(&self.run(command).await?).map(|o| o.summary)
    }

    async fn triage(&self, items: &[SnoozedItem], now: DateTime<Utc>) -> Result<Vec<TriageSuggestion>, PluginError> {
        let command = command(
            &prompt::triage_message(items, now),
            &prompt::triage_system(&self.language),
            &Schema::triage(),
            &self.model,
        );
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

    /// LINUX-12: Linux also looks in Linuxbrew, the system's `/usr/bin` and the session's PATH, once each. Unix paths:
    /// on Windows `/nix/profile/bin` isn't absolute.
    #[cfg(unix)]
    #[test]
    fn linux_looks_in_linuxbrew_the_system_and_the_path() {
        let env = SearchEnv {
            home: Some(PathBuf::from("/home/erin")),
            path: Some(std::env::join_paths(["/nix/profile/bin", "relative/bin", "/usr/bin"]).unwrap()),
            ..SearchEnv::default()
        };
        let folders = env.linux_folders();
        let at = |p: &str| folders.iter().position(|f| f == Path::new(p));
        assert!(at("/home/erin/.local/bin") < at("/home/linuxbrew/.linuxbrew/bin"), "the usual places first");
        assert!(at("/home/erin/.linuxbrew/bin").is_some() && at("/nix/profile/bin").is_some());
        assert_eq!(folders.iter().filter(|f| *f == Path::new("/usr/bin")).count(), 1, "once");
        assert!(at("relative/bin").is_none(), "absolute folders only");
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

    #[tokio::test]
    async fn runs_at_low_effort_like_the_api_and_without_it_on_an_older_claude() {
        let answer = r#"{"is_error": false, "structured_output": {"summary": "ok"}}"#;
        let runner = RecordingRunner::new(answer);
        plugin(&fake_claude(), &runner).digest(&[item("a")], "x", now()).await.unwrap();
        assert_eq!(after(&runner.arguments(), "--effort"), "low");
        let older = RecordingRunner::older(answer);
        assert_eq!(plugin(&fake_claude(), &older).digest(&[item("a")], "x", now()).await.unwrap(), "ok");
        assert!(!older.arguments().contains(&"--effort".into()), "run again without it");
        assert!(older.arguments().contains(&"--json-schema".into()));
        assert!(rejects_effort("error: unknown option '--effort'") && !rejects_effort("Not logged in"));
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
        assert_eq!(parse(json.as_bytes(), &HashSet::new()).err(), Some(PluginError::Api(NOT_SIGNED_IN.into())));
        let json = r#"{"type": "result", "is_error": true, "subtype": "error_during_execution", "result": "Boom"}"#;
        assert_eq!(parse(json.as_bytes(), &HashSet::new()).err(), Some(PluginError::Api("Claude Code: Boom".into())));
        assert_eq!(
            parse(br#"{"is_error": true}"#, &HashSet::new()).err(),
            Some(PluginError::Api("Claude Code: the request failed.".into()))
        );
        assert_eq!(parse(b"Segmentation fault", &HashSet::new()).err(), Some(PluginError::Api(UNEXPECTED.into())));
    }

    /// Each kind of error Claude Code reports names its cause, as the other assistants do.
    #[test]
    fn names_the_cause_of_each_error_kind() {
        let cases = [
            (
                r#"{"is_error": true, "subtype": "error_max_structured_output_retries"}"#,
                AssistantFailure::NoStructuredAnswer,
            ),
            (r#"{"is_error": true, "subtype": "error_max_turns"}"#, AssistantFailure::TooManySteps),
            (r#"{"is_error": true, "subtype": "error_max_budget_usd"}"#, AssistantFailure::SpendingLimit),
            (r#"{"is_error": false, "subtype": "success"}"#, AssistantFailure::UnexpectedAnswer),
            (r#"{"is_error": false, "structured_output": {"focus": []}}"#, AssistantFailure::UnexpectedAnswer),
        ];
        for (json, expected) in cases {
            assert_eq!(parse(json.as_bytes(), &HashSet::new()).err(), Some(expected.into()), "{json}");
        }
    }

    #[tokio::test]
    async fn digest_uses_the_lighter_model_and_the_inbox_goes_on_stdin() {
        let runner = RecordingRunner::new(
            r#"{"is_error": false, "structured_output": {"summary": "Two reviews wait; one is blocking."}}"#,
        );
        let summary =
            plugin(&fake_claude(), &runner).digest(&[item("a"), item("a")], "To review", now()).await.unwrap();
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
        let runner = RecordingRunner::new(
            r#"{"is_error": false, "structured_output": {"summary": "s", "focus": [{"id": "a", "reason": "r"}, {"id": "ghost", "reason": "?"}]}}"#,
        );
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
              {{"id": "ghost", "action": "now", "until": "", "reason": "unknown item"}},
              {{"id": "e", "action": "reschedule", "until": "2099-01-01T09:00:00Z", "reason": "far away"}}
            ]}}}}"#
        );
        let runner = RecordingRunner::new(&output);
        let items: Vec<SnoozedItem> = ["a", "b", "c", "d", "e"].iter().map(|id| snoozed(item(id), 3)).collect();
        let suggestions = plugin(&fake_claude(), &runner).triage(&items, now()).await.unwrap();
        assert_eq!(suggestions.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(), ["a", "b", "e"]);
        assert_eq!(suggestions[2].until, Some(now() + chrono::Duration::days(60)));
        assert_eq!(
            (suggestions[0].action, suggestions[0].until),
            (TriageAction::Reschedule, prompt::parse_iso8601(&later))
        );
        assert_eq!((suggestions[1].action, suggestions[1].until), (TriageAction::Done, None));
        let message = runner.input();
        assert!(message.contains(r#""timesSnoozed":3"#) && message.contains(r#""reason":"motivation""#), "{message}");
        assert!(after(&runner.arguments(), "--json-schema").contains(r#""enum":["keep","reschedule","done","now"]"#));
        assert_eq!(items[0].snooze.reason, Some(SnoozeReason::Motivation));
    }

    /// AI-03: Claude Code's `usage`, as the Messages API words it.
    #[tokio::test]
    async fn the_tokens_used_are_reported() {
        let runner = RecordingRunner::new(
            r#"{"is_error": false, "structured_output": {"summary": "ok"}, "usage": {"input_tokens": 7, "cache_read_input_tokens": 1000, "output_tokens": 30}}"#,
        );
        let meter = crate::assistant::AssistantMeter::default();
        meter.measure(plugin(&fake_claude(), &runner).digest(&[item("a")], "x", now())).await.unwrap();
        assert_eq!(meter.tokens(), Some(TokenUsage { input: 1_007, output: 30 }));
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
        let versions: Vec<String> = nvm_folders(&home)
            .iter()
            .map(|p| p.parent().unwrap().file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        std::fs::remove_dir_all(&home).unwrap();
        assert_eq!(versions, ["v22.12.0", "v18.20.4", "v9.11.2"]);
    }

    #[test]
    fn windows_installs_are_searched_installer_first_then_path() {
        // Absolute on every OS: "/U" has no drive letter, so Windows reads it as relative.
        let root = std::env::temp_dir();
        let (home, program_files) = (root.join("U").join("ada"), root.join("PF"));
        let git = program_files.join("Git").join("cmd");
        let env = SearchEnv {
            home: Some(home.clone()),
            app_data: Some(home.join("AppData").join("Roaming")),
            local_app_data: Some(home.join("AppData").join("Local")),
            program_files: Some(program_files.clone()),
            program_files_x86: None,
            nvm_symlink: None,
            path: Some(std::env::join_paths([git.clone(), root.join("tools"), PathBuf::from("relative")]).unwrap()),
        };
        let folders = env.windows_folders();
        assert_eq!(folders[0], home.join(".local").join("bin"), "Anthropic's installer first");
        assert!(folders.contains(&home.join("AppData").join("Roaming").join("npm")));
        assert!(folders.contains(&program_files.join("nodejs")));
        assert_eq!(folders.last(), Some(&git), "PATH last, relative folders left out");
        assert!(!folders.contains(&root.join("tools")), "SEC-30: not outside your profile and Program Files");
        assert!(SearchEnv::default().windows_folders().is_empty());
    }

    #[test]
    fn only_folders_inside_your_profile_or_program_files_are_searched() {
        let env = SearchEnv {
            home: Some(PathBuf::from("/C:/Users/Ada")),
            program_files: Some(PathBuf::from("/C:/Program Files")),
            program_files_x86: Some(PathBuf::from("/C:/Program Files (x86)")),
            ..SearchEnv::default()
        };
        assert!(env.may_search(Path::new("/C:/Users/Ada/.local/bin")));
        assert!(env.may_search(Path::new("/c:/users/ada/scoop/shims")), "case doesn't matter");
        assert!(env.may_search(Path::new("/C:/Program Files (x86)/nodejs")));
        assert!(!env.may_search(Path::new("/C:/tools")));
        assert!(!env.may_search(Path::new("/C:/Users/Adam/bin")), "a whole folder name");
        assert!(!env.may_search(Path::new("/C:/Users/Ada/../Public/bin")), "no way out");
        assert!(!SearchEnv::default().may_search(Path::new("/C:/Users/Ada")));
    }

    #[test]
    fn only_you_or_the_system_may_write_to_claude() {
        const ME: &str = "S-1-5-21-1-2-3-1001";
        let entry = |sid: &str, mask: u32| AccessEntry { sid: sid.into(), allows: true, mask, inherit_only: false };
        let (full, read) = (0x001F_01FF, 0x0012_00A9);
        // Program Files' defaults: TrustedInstaller, SYSTEM and Administrators change it, Users and app packages read.
        let program_files = [
            entry(SYSTEM_SIDS[2], full),
            entry("S-1-5-18", full),
            entry("S-1-5-32-544", full),
            entry("S-1-5-32-545", read),
            entry("S-1-15-2-1", read),
            AccessEntry { inherit_only: true, ..entry("S-1-3-0", 0x1000_0000) },
        ];
        assert!(only_trusted_can_write(SYSTEM_SIDS[2], &program_files, ME));
        assert!(only_trusted_can_write(ME, &[entry(ME, full), entry("s-1-5-18", full)], ME), "your profile");
        let users_write = [entry("S-1-5-32-545", 0x0012_0116)];
        assert!(!only_trusted_can_write(ME, &users_write, ME), "Users may add files, as in C:\\tools");
        assert!(!only_trusted_can_write(ME, &[entry("S-1-1-0", 0x0001_0000)], ME), "Everyone may delete it");
        assert!(!only_trusted_can_write(ME, &[entry("S-1-5-11", 0x0004_0000)], ME), "may change the access list");
        assert!(!only_trusted_can_write("S-1-5-21-1-2-3-1002", &[entry(ME, full)], ME), "someone else owns it");
        let denied = AccessEntry { allows: false, ..entry("S-1-1-0", full) };
        assert!(only_trusted_can_write(ME, &[denied, entry(ME, full)], ME), "a denial refuses nothing more");
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
        assert_eq!(
            result.err(),
            Some(PluginError::Api(NOT_CLAUDE_CODE.replace("%@", &impostor.display().to_string())))
        );
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
        assert_eq!(
            explain("'node' is not recognized as an internal or external command"),
            PluginError::Api(NEEDS_NODE.into())
        );
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
        assert_eq!(
            manifest.fields.iter().map(|f| f.key.as_str()).collect::<Vec<_>>(),
            ["path", "model", "digestModel"]
        );
        assert_eq!(manifest.fields[2].default_value, "claude-haiku-5-5");
        assert!(manifest.fields[0].is_optional && manifest.fields[1].is_optional && !manifest.fields[2].is_optional);
    }
}

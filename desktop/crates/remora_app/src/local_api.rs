//! The `remora` command: `remora count`, `remora list --tab waiting --json`, `remora remind … --at 15:00`
//! ask the running app over a socket only this user can open: a Unix socket (0600) in the runtime folder on Linux, a
//! named pipe whose only access is this user's on Windows. Nothing listens on the network. The rules (what may be
//! asked, what is hidden) are `remora_core::command`'s; this is the endpoint, both ends of it, and the inbox's answer.
//! The macOS app has its own (`LocalSocket.swift`), with the same protocol.

use crate::Inbox;
use chrono::{DateTime, Utc};
use remora_core::{
    agent_read, answer_command, command_invocation, command_output, McpSession, COMMAND_LINE_LIMIT, COMMAND_USAGE,
    MCP_DISABLED,
};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::PathBuf;

/// The first words that make the app's binary a command instead of the tray app.
const COMMANDS: [&str; 6] = ["count", "list", "remind", "mcp", "help", "--help"];

/// Overrides where the app listens and the command asks (tests, a demo run).
pub const ENDPOINT_VARIABLE: &str = "REMORA_SOCKET";

/// Whether the arguments (without the program's name) are a `remora` command.
pub fn is_command(args: &[String]) -> bool {
    args.first().is_some_and(|first| COMMANDS.contains(&first.as_str()))
}

/// The command line's whole run: asks the app, prints its answer. 0 done, 1 refused, 2 a mistake in the command,
/// 3 Remora isn't running.
pub fn run_command(args: &[String]) -> i32 {
    if args.first().is_some_and(|first| first == "help" || first == "--help") {
        println!("{COMMAND_USAGE}");
        return 0;
    }
    let allowed = crate::Managed::read().local_api_allowed();
    if args.first().is_some_and(|first| first == "mcp") {
        return run_mcp(allowed);
    }
    if !allowed {
        eprintln!("{MCP_DISABLED}");
        return 1;
    }
    let invocation = match command_invocation(args) {
        Ok(invocation) => invocation,
        Err(usage) => {
            eprintln!("{usage}");
            return 2;
        }
    };
    match ask(&endpoint(), &invocation.request) {
        Ok(response) => {
            let (out, err, code) = command_output(&response, invocation.json);
            if !out.is_empty() {
                println!("{out}");
            }
            if !err.is_empty() {
                eprintln!("{err}");
            }
            code
        }
        Err(error) => {
            eprintln!("Remora isn't running ({error}).");
            3
        }
    }
}

/// `remora mcp`: an MCP server on standard input and output for the agents on this computer, each tool call
/// relayed to the app as a request line. It runs until the agent closes its end. With `AllowLocalAPI` off it still
/// answers, so the agent can say why, but refuses every call (`McpSession`).
fn run_mcp(allowed: bool) -> i32 {
    let mut session = McpSession::new(env!("CARGO_PKG_VERSION"), allowed);
    let endpoint = endpoint();
    let mut ask = |request: &str| ask(&endpoint, request).map_err(|_| ());
    let stdout = io::stdout();
    for line in io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        if let Some(answer) = session.handle(&line, &mut ask) {
            let mut out = stdout.lock();
            if writeln!(out, "{answer}").and_then(|_| out.flush()).is_err() {
                break;
            }
        }
    }
    0
}

/// What the app answers a request line with: the inbox as it is now, without the titles of the tools whose content
/// is hidden (Settings, or the organization's policy). True when a reminder was saved, for the window to show it.
pub fn answer(inbox: &mut Inbox, line: &str, now: DateTime<Utc>) -> (String, bool) {
    // A policy that changed since the app started listening: refuse rather than answer.
    if !inbox.local_api_allowed() {
        return (serde_json::json!({ "ok": false, "error": MCP_DISABLED }).to_string(), false);
    }
    if let Some(read) = agent_read(line, now) {
        inbox.record_agent_read(read);
    }
    let layout = inbox.layout(now, "");
    let preferences = inbox.effective_preferences();
    let mut saved = false;
    let answer = answer_command(line, &layout, &|plugin| preferences.hides_content(plugin), now, &mut |text, at| {
        let id = inbox.add_reminder(text, at, None, None, now).map_err(|message| message.text)?;
        saved = true;
        Ok(id)
    });
    (answer, saved)
}

/// The endpoint's name: `REMORA_SOCKET` when set; otherwise per user, and apart for a development build.
pub fn endpoint() -> PathBuf {
    if let Some(path) = std::env::var_os(ENDPOINT_VARIABLE).filter(|p| !p.is_empty()) {
        return PathBuf::from(path);
    }
    let name = if crate::paths::INSTALLED { "remora" } else { "remora-dev" };
    if cfg!(windows) {
        let user = std::env::var("USERNAME").unwrap_or_default();
        return PathBuf::from(format!(r"\\.\pipe\{name}-{user}"));
    }
    // The runtime folder is this user's alone (0700) and empty at each login; without one, the data folder.
    let folder = if cfg!(target_os = "linux") {
        std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(crate::paths::data_dir)
    } else {
        std::env::temp_dir()
    };
    folder.join(format!("{name}.sock"))
}

/// Reads one request line, at most `COMMAND_LINE_LIMIT` bytes (and a little: the limit itself is the answer's to
/// enforce), answers it, closes.
fn exchange(stream: impl Read + Write, handler: &dyn Fn(&str) -> String) -> io::Result<()> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.by_ref().take(COMMAND_LINE_LIMIT as u64 + 2).read_line(&mut line)?;
    let mut answer = handler(&line);
    answer.push('\n');
    let stream = reader.get_mut();
    stream.write_all(answer.as_bytes())?;
    stream.flush()
}

/// Sends one request line, returns the answer line.
pub fn ask(endpoint: &std::path::Path, request: &str) -> io::Result<String> {
    let stream = connect(endpoint)?;
    let mut reader = BufReader::new(stream);
    reader.get_mut().write_all(format!("{request}\n").as_bytes())?;
    reader.get_mut().flush()?;
    let mut answer = String::new();
    reader.read_line(&mut answer)?;
    Ok(answer)
}

/// Listens at `endpoint` in the background, `handler` answering each request line. Fails when another app already
/// listens there.
pub fn serve(endpoint: PathBuf, handler: impl Fn(&str) -> String + Send + Sync + 'static) -> io::Result<()> {
    platform::serve(endpoint, std::sync::Arc::new(handler))
}

#[cfg(unix)]
fn connect(endpoint: &std::path::Path) -> io::Result<std::os::unix::net::UnixStream> {
    let stream = std::os::unix::net::UnixStream::connect(endpoint)?;
    stream.set_read_timeout(Some(std::time::Duration::from_secs(5)))?;
    Ok(stream)
}

#[cfg(windows)]
fn connect(endpoint: &std::path::Path) -> io::Result<std::fs::File> {
    // Every instance busy for a moment: wait a little.
    const PIPE_BUSY: i32 = 231;
    let mut tries = 0;
    loop {
        match std::fs::OpenOptions::new().read(true).write(true).open(endpoint) {
            Err(error) if error.raw_os_error() == Some(PIPE_BUSY) && tries < 20 => {
                tries += 1;
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            result => return result,
        }
    }
}

#[cfg(unix)]
mod platform {
    use super::exchange;
    use std::fs;
    use std::io;
    use std::os::unix::fs::{FileTypeExt, PermissionsExt};
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::Duration;

    pub fn serve(path: PathBuf, handler: Arc<dyn Fn(&str) -> String + Send + Sync>) -> io::Result<()> {
        if let Some(folder) = path.parent() {
            fs::create_dir_all(folder)?;
        }
        if UnixStream::connect(&path).is_ok() {
            return Err(io::Error::new(io::ErrorKind::AddrInUse, "another Remora answers there"));
        }
        // A socket left by a run that ended abruptly; never another kind of file.
        if fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_socket()) {
            fs::remove_file(&path)?;
        }
        let listener = UnixListener::bind(&path)?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
        std::thread::Builder::new().name("remora-command".into()).spawn(move || {
            for stream in listener.incoming().flatten() {
                let handler = handler.clone();
                // One at a time is plenty; a client that sends nothing is dropped after a second.
                let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
                let _ = exchange(stream, &*handler);
            }
        })?;
        Ok(())
    }
}

#[cfg(windows)]
mod platform {
    use super::exchange;
    use std::fs::File;
    use std::io;
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use std::path::PathBuf;
    use std::ptr::null_mut;
    use std::sync::Arc;
    use windows_sys::Win32::Foundation::{GetLastError, LocalFree, ERROR_PIPE_CONNECTED, HANDLE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Security::Authorization::{
        ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
    };
    use windows_sys::Win32::Security::{
        GetTokenInformation, TokenUser, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        FlushFileBuffers, FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_DUPLEX,
    };
    use windows_sys::Win32::System::Pipes::{
        ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS,
        PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    /// A security descriptor from `ConvertStringSecurityDescriptorToSecurityDescriptorW`, freed with it.
    struct Descriptor(PSECURITY_DESCRIPTOR);
    // Read-only once made, and only by the thread that creates the pipes.
    unsafe impl Send for Descriptor {}
    impl Drop for Descriptor {
        fn drop(&mut self) {
            unsafe { LocalFree(self.0 as _) };
        }
    }

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(Some(0)).collect()
    }

    /// This user's SID, as text ("S-1-5-21-…").
    fn user_sid() -> io::Result<String> {
        unsafe {
            let mut token: HANDLE = null_mut();
            if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
                return Err(io::Error::last_os_error());
            }
            let token = OwnedHandle::from_raw_handle(token as _);
            let mut size = 0u32;
            GetTokenInformation(token.as_raw_handle() as _, TokenUser, null_mut(), 0, &mut size);
            // u64s: TOKEN_USER starts with a pointer.
            let mut buffer = vec![0u64; (size as usize).div_ceil(8)];
            if GetTokenInformation(token.as_raw_handle() as _, TokenUser, buffer.as_mut_ptr() as _, size, &mut size)
                == 0
            {
                return Err(io::Error::last_os_error());
            }
            let user = &*(buffer.as_ptr() as *const TOKEN_USER);
            let mut text: *mut u16 = null_mut();
            if ConvertSidToStringSidW(user.User.Sid, &mut text) == 0 {
                return Err(io::Error::last_os_error());
            }
            let length = (0..).take_while(|&i| *text.add(i) != 0).count();
            let sid = String::from_utf16_lossy(std::slice::from_raw_parts(text, length));
            LocalFree(text as _);
            Ok(sid)
        }
    }

    /// Full access for this user, none for anyone else (`P`: nothing inherited).
    fn only_this_user() -> io::Result<Descriptor> {
        let sddl = wide(&format!("D:P(A;;GA;;;{})", user_sid()?));
        let mut descriptor: PSECURITY_DESCRIPTOR = null_mut();
        let made = unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                SDDL_REVISION_1,
                &mut descriptor,
                null_mut(),
            )
        };
        if made == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Descriptor(descriptor))
    }

    fn create(name: &[u16], descriptor: &Descriptor, first: bool) -> io::Result<HANDLE> {
        let attributes = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor.0,
            bInheritHandle: 0,
        };
        // The first instance fails when the name is taken, by another Remora or anyone else.
        let open = PIPE_ACCESS_DUPLEX | if first { FILE_FLAG_FIRST_PIPE_INSTANCE } else { 0 };
        let mode = PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS;
        let pipe = unsafe {
            CreateNamedPipeW(name.as_ptr(), open, mode, PIPE_UNLIMITED_INSTANCES, 4096, 4096, 0, &attributes)
        };
        if pipe == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        Ok(pipe)
    }

    /// A connected instance, answered on a thread of its own, then closed.
    struct Instance(HANDLE);
    unsafe impl Send for Instance {}

    pub fn serve(path: PathBuf, handler: Arc<dyn Fn(&str) -> String + Send + Sync>) -> io::Result<()> {
        let name: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let descriptor = only_this_user()?;
        let mut waiting = Instance(create(&name, &descriptor, true)?);
        std::thread::Builder::new().name("remora-command".into()).spawn(move || {
            loop {
                let connected = unsafe { ConnectNamedPipe(waiting.0, null_mut()) } != 0
                    || unsafe { GetLastError() } == ERROR_PIPE_CONNECTED;
                // The next instance is ready before this one is answered: a client never finds none.
                let Ok(next) = create(&name, &descriptor, false) else { return };
                let instance = std::mem::replace(&mut waiting, Instance(next));
                if !connected {
                    drop(unsafe { File::from_raw_handle(instance.0 as _) });
                    continue;
                }
                let handler = handler.clone();
                let _ = std::thread::Builder::new().name("remora-command-client".into()).spawn(move || {
                    let instance = instance;
                    let file = unsafe { File::from_raw_handle(instance.0 as _) };
                    let _ = exchange(&file, &*handler);
                    unsafe {
                        FlushFileBuffers(instance.0);
                        DisconnectNamedPipe(instance.0);
                    }
                });
            }
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Managed, MemoryVault};
    use serde_json::Value;
    use std::sync::Arc;

    fn now() -> DateTime<Utc> {
        "2026-10-15T10:00:00Z".parse().unwrap()
    }

    #[test]
    fn only_the_three_commands_mcp_and_help_are_commands() {
        let args = |list: &[&str]| list.iter().map(|a| a.to_string()).collect::<Vec<_>>();
        assert!(is_command(&args(&["count"])) && is_command(&args(&["remind", "x"])) && is_command(&args(&["help"])));
        assert!(is_command(&args(&["mcp"])));
        assert!(!is_command(&args(&[])) && !is_command(&args(&["--reminder"])) && !is_command(&args(&["--demo"])));
    }

    /// A reminder saved by the command is in the inbox; the policy's hidden content hides its title.
    #[test]
    fn answers_from_the_inbox_and_hides_what_the_policy_hides() {
        let mut inbox = Inbox::in_memory(Arc::new(MemoryVault::default()), Managed::default());
        let (saved, changed) =
            answer(&mut inbox, r#"{"command":"remind","text":"Call Erin","at":"2026-10-15T10:01:00Z"}"#, now());
        assert!(changed, "{saved}");
        inbox.tick(now() + chrono::Duration::minutes(2));
        let later = now() + chrono::Duration::minutes(2);
        let (list, _) = answer(&mut inbox, r#"{"command":"list"}"#, later);
        let list: Value = serde_json::from_str(&list).unwrap();
        assert_eq!(list["items"][0]["title"], "Call Erin");
        let (count, changed) = answer(&mut inbox, r#"{"command":"count"}"#, later);
        assert_eq!(serde_json::from_str::<Value>(&count).unwrap()["count"], 1);
        assert!(!changed);

        inbox.arrange().1.hidden_content_sources = Some(vec!["reminders".into()]);
        let (list, _) = answer(&mut inbox, r#"{"command":"list"}"#, later);
        assert!(!list.contains("Call Erin"), "{list}");
    }

    /// What an agent asks through `remora mcp` is logged under its declared name; the `remora` command isn't.
    /// With `AllowLocalAPI` off, the app refuses even a request that reached it.
    #[test]
    fn agents_reads_are_logged_and_the_policy_can_refuse() {
        let mut inbox = Inbox::in_memory(Arc::new(MemoryVault::default()), Managed::default());
        answer(&mut inbox, r#"{"command":"list","tab":"waiting","agent":"Claude Desktop"}"#, now());
        answer(&mut inbox, r#"{"command":"count"}"#, now());
        let reads = inbox.agent_reads();
        assert_eq!(reads.len(), 1);
        assert_eq!((reads[0].agent.as_str(), reads[0].tab.as_deref()), ("Claude Desktop", Some("waiting")));

        inbox.arrange().1.allow_local_api = Some(false);
        let (refused, _) = answer(&mut inbox, r#"{"command":"count","agent":"Claude Desktop"}"#, now());
        assert!(refused.contains("AllowLocalAPI") && refused.contains(r#""ok":false"#), "{refused}");
        assert_eq!(inbox.agent_reads().len(), 1, "a refused request isn't logged");
    }

    /// Both ends over a real socket, which only this user may open.
    #[cfg(unix)]
    #[test]
    fn asks_and_answers_over_a_private_socket() {
        use std::os::unix::fs::PermissionsExt;
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("remora.sock");
        serve(path.clone(), |line| format!(r#"{{"ok":true,"echo":{}}}"#, line.trim())).unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        assert_eq!(ask(&path, r#"{"command":"count"}"#).unwrap().trim(), r#"{"ok":true,"echo":{"command":"count"}}"#);
        assert!(serve(path.clone(), |_| String::new()).is_err(), "one app per socket");
        assert!(ask(&folder.path().join("none.sock"), "{}").is_err(), "nothing listens: Remora isn't running");
    }
}

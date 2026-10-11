//! Runs a local program, for assistants built on command-line tools. A port of the macOS `CommandRunner.swift`.

use async_trait::async_trait;
use std::collections::HashMap;
use std::fmt;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandError {
    NotFound(String),
    TimedOut,
    /// Exit status (-1 when there is none) and the end of its error output.
    Failed(i32, String),
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommandError::NotFound(name) => write!(f, "{}", NOT_FOUND.replace("%@", name)),
            CommandError::TimedOut => write!(f, "The command took too long and was stopped."),
            CommandError::Failed(status, message) if message.is_empty() => {
                write!(f, "{}", FAILED.replace("%d", &status.to_string()))
            }
            CommandError::Failed(_, message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for CommandError {}

const NOT_FOUND: &str = "Couldn’t find %@.";
const FAILED: &str = "The command failed (status %d).";

#[async_trait]
pub trait CommandRunner: Send + Sync {
    /// `environment` replaces the program's environment; None keeps Remora's. `input` goes to the program's standard
    /// input: what other processes mustn't see goes there, never in `arguments`, which any process of the user can
    /// read.
    async fn run(
        &self,
        program: &Path,
        arguments: &[String],
        environment: Option<&HashMap<String, String>>,
        input: Option<&[u8]>,
    ) -> Result<Vec<u8>, CommandError>;
}

/// The real thing, through `tokio::process`.
pub struct ProcessCommandRunner {
    pub timeout: Duration,
}

impl Default for ProcessCommandRunner {
    /// Three minutes, as on macOS: a brief of a full inbox takes a while.
    fn default() -> Self {
        ProcessCommandRunner { timeout: Duration::from_secs(180) }
    }
}

/// No console window flashing up when the tray app starts a console program.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

async fn read_all(pipe: Option<impl AsyncRead + Unpin>) -> Vec<u8> {
    let mut data = Vec::new();
    if let Some(mut pipe) = pipe {
        let _ = pipe.read_to_end(&mut data).await;
    }
    data
}

#[async_trait]
impl CommandRunner for ProcessCommandRunner {
    async fn run(
        &self,
        program: &Path,
        arguments: &[String],
        environment: Option<&HashMap<String, String>>,
        input: Option<&[u8]>,
    ) -> Result<Vec<u8>, CommandError> {
        let mut command = tokio::process::Command::new(program);
        command
            .args(arguments)
            .current_dir(std::env::temp_dir())
            .stdin(if input.is_some() { Stdio::piped() } else { Stdio::null() })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        if let Some(environment) = environment {
            command.env_clear().envs(environment);
        }
        #[cfg(windows)]
        command.creation_flags(CREATE_NO_WINDOW);
        let mut child = command.spawn().map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => CommandError::NotFound(program.display().to_string()),
            _ => CommandError::Failed(-1, e.to_string()),
        })?;
        if let (Some(mut stdin), Some(input)) = (child.stdin.take(), input) {
            // Written apart and closed after: a prompt bigger than the pipe's buffer would otherwise wait for the
            // program to read it, while we wait for its output.
            let input = input.to_vec();
            tokio::spawn(async move {
                let _ = stdin.write_all(&input).await;
                let _ = stdin.shutdown().await;
            });
        }
        let (stdout, stderr) = (child.stdout.take(), child.stderr.take());
        // Both pipes are read at once: a program that fills one while we wait on the other would block.
        let finished = tokio::time::timeout(self.timeout, async {
            let (data, errors) = tokio::join!(read_all(stdout), read_all(stderr));
            (data, errors, child.wait().await)
        })
        .await;
        let Ok((data, errors, status)) = finished else {
            let _ = child.start_kill();
            return Err(CommandError::TimedOut);
        };
        let status = status.map_err(|e| CommandError::Failed(-1, e.to_string()))?;
        // A failure with nothing on stdout: what the program said on stderr is the only explanation. With output,
        // the output wins (claude prints its errors as JSON and exits non-zero).
        if !status.success() && data.is_empty() {
            return Err(CommandError::Failed(status.code().unwrap_or(-1), tail(&errors)));
        }
        Ok(data)
    }
}

/// The last few lines of a program's error output, short enough for an error message.
pub fn tail(data: &[u8]) -> String {
    let text = String::from_utf8_lossy(data);
    let lines: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    let joined = lines[lines.len().saturating_sub(3)..].join(" ");
    let count = joined.chars().count();
    joined.chars().skip(count.saturating_sub(300)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runner() -> ProcessCommandRunner {
        ProcessCommandRunner { timeout: Duration::from_secs(10) }
    }

    #[test]
    fn the_tail_keeps_the_last_three_lines_and_300_characters() {
        assert_eq!(tail(b"one\n\n  two \nthree\r\nfour\n"), "two three four");
        assert_eq!(tail(&[b'x'; 400]).len(), 300);
        assert_eq!(CommandError::Failed(3, String::new()).to_string(), "The command failed (status 3).");
        assert_eq!(CommandError::NotFound("claude".into()).to_string(), "Couldn’t find claude.");
    }

    #[cfg(unix)]
    mod unix {
        use super::*;
        use std::path::PathBuf;

        fn shell() -> PathBuf {
            PathBuf::from("/bin/sh")
        }

        fn args(script: &str) -> Vec<String> {
            vec!["-c".into(), script.into()]
        }

        #[tokio::test]
        async fn a_failure_carries_the_end_of_its_error_output() {
            let result = runner().run(&shell(), &args("echo first >&2; echo second >&2; exit 3"), None, None).await;
            assert_eq!(result, Err(CommandError::Failed(3, "first second".into())));
        }

        /// Stderr read by nobody blocks a program past the pipe's buffer, until the timeout.
        #[tokio::test]
        async fn lots_of_error_output_does_not_block() {
            let data = runner()
                .run(&shell(), &args("head -c 300000 /dev/zero | tr '\\0' x >&2; echo done"), None, None)
                .await
                .unwrap();
            assert_eq!(data, b"done\n");
        }

        #[tokio::test]
        async fn the_environment_is_passed() {
            let environment = HashMap::from([
                ("REMORA_TEST".to_string(), "yes".to_string()),
                ("PATH".to_string(), "/usr/bin:/bin".to_string()),
            ]);
            let data =
                runner().run(&shell(), &args("printf %s \"$REMORA_TEST\""), Some(&environment), None).await.unwrap();
            assert_eq!(data, b"yes");
        }

        /// What mustn't show in the process list reaches the program on standard input, even past the
        /// pipe's buffer.
        #[tokio::test]
        async fn input_goes_to_standard_input() {
            let secret = b"Fix the CSV export for Alice";
            assert_eq!(runner().run(Path::new("/bin/cat"), &[], None, Some(secret)).await.unwrap(), secret);
            let big = vec![b'x'; 300_000];
            let counted = runner().run(&shell(), &args("wc -c | tr -d ' '"), None, Some(&big)).await.unwrap();
            assert_eq!(String::from_utf8_lossy(&counted).trim(), "300000");
        }

        #[tokio::test]
        async fn output_wins_over_a_failing_status() {
            // claude --output-format json prints its error as JSON and exits non-zero: that JSON is the answer.
            let data = runner().run(&shell(), &args("echo '{\"is_error\": true}'; exit 1"), None, None).await.unwrap();
            assert!(!data.is_empty());
        }

        #[tokio::test]
        async fn a_slow_program_is_stopped() {
            let quick = ProcessCommandRunner { timeout: Duration::from_millis(200) };
            assert_eq!(quick.run(&shell(), &args("sleep 5"), None, None).await, Err(CommandError::TimedOut));
        }

        #[tokio::test]
        async fn a_missing_program_is_not_found() {
            let missing = Path::new("/nowhere/claude");
            assert_eq!(
                runner().run(missing, &[], None, None).await,
                Err(CommandError::NotFound("/nowhere/claude".into()))
            );
        }
    }

    #[cfg(windows)]
    mod windows {
        use super::*;

        #[tokio::test]
        async fn input_goes_to_standard_input() {
            let data = runner()
                .run(
                    Path::new("cmd.exe"),
                    &["/C".into(), "sort".into()],
                    None,
                    Some(b"Fix the CSV export for Alice\r\n"),
                )
                .await
                .unwrap();
            assert_eq!(String::from_utf8_lossy(&data).trim(), "Fix the CSV export for Alice");
        }

        #[tokio::test]
        async fn a_failure_carries_its_status() {
            let result = runner().run(Path::new("cmd.exe"), &["/C".into(), "exit 3".into()], None, None).await;
            assert_eq!(result, Err(CommandError::Failed(3, String::new())));
        }
    }
}

use std::ffi::{OsStr, OsString};
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use crate::error::CoralError;

/// Oldest git we accept. `--porcelain=v2`, `--update-refs` and `merge-file --diff3` all
/// predate this, but 2.40 is the floor the project committed to.
pub const MIN_GIT: GitVersion = GitVersion {
    major: 2,
    minor: 40,
    patch: 0,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct GitVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

impl fmt::Display for GitVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Serializes as `"2.43.0"` rather than a struct; the envelope and the UI both treat a git
/// version as an opaque display string.
impl serde::Serialize for GitVersion {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl GitVersion {
    /// Parses the `git version 2.43.0` line. Trailing vendor suffixes (`.windows.1`,
    /// `(Apple Git-154)`) are ignored rather than rejected.
    fn parse(raw: &str) -> Option<Self> {
        let rest = raw.trim().strip_prefix("git version ")?;
        let mut parts = rest.split(['.', ' ', '-']).map(str::parse::<u16>);
        Some(Self {
            major: parts.next()?.ok()?,
            minor: parts.next()?.ok()?,
            patch: parts.next().and_then(Result::ok).unwrap_or(0),
        })
    }
}

/// What a git invocation is allowed to touch. Drives timeouts and lock flags, and in M1 will
/// drive the engine's read/write scheduling.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GitClass {
    /// Objects and refs only: rev-list, cat-file, for-each-ref, show.
    Read,
    /// Reads *and* refreshes `.git/index`: status, diff, ls-files.
    Status,
    /// Mutates refs, index or worktree.
    Write,
    /// Talks to a remote.
    Network,
}

impl GitClass {
    #[must_use]
    pub const fn timeout(self) -> Option<Duration> {
        match self {
            Self::Read => Some(Duration::from_secs(30)),
            Self::Status => Some(Duration::from_secs(15)),
            // User hooks are arbitrary code; a slow pre-commit is not our failure.
            Self::Write => Some(Duration::from_secs(120)),
            // Bounded by the caller cancelling, not by a clock.
            Self::Network => None,
        }
    }

    /// `Status` deliberately opts out: with `--no-optional-locks` git cannot write the
    /// refreshed index back, so the stat cache stays cold and every later status re-hashes
    /// racily-clean files. On an 80k-file worktree that costs more than the lock contention
    /// it avoids, and the engine serializes writes anyway.
    #[must_use]
    pub const fn no_optional_locks(self) -> bool {
        matches!(self, Self::Read)
    }
}

/// Builder for one git invocation. Every git command in the engine is constructed here; no
/// module assembles an argv by hand.
pub struct GitCommand {
    label: &'static str,
    class: GitClass,
    workdir: Option<PathBuf>,
    args: Vec<OsString>,
    secret_args: Vec<usize>,
    stdin: Option<Vec<u8>>,
    env: Vec<(OsString, OsString)>,
}

impl GitCommand {
    pub fn read(label: &'static str, workdir: impl Into<PathBuf>) -> Self {
        Self::new(label, GitClass::Read, Some(workdir.into()))
    }

    pub fn status(label: &'static str, workdir: impl Into<PathBuf>) -> Self {
        Self::new(label, GitClass::Status, Some(workdir.into()))
    }

    pub fn write(label: &'static str, workdir: impl Into<PathBuf>) -> Self {
        Self::new(label, GitClass::Write, Some(workdir.into()))
    }

    pub fn network(label: &'static str, workdir: impl Into<PathBuf>) -> Self {
        Self::new(label, GitClass::Network, Some(workdir.into()))
    }

    /// No `-C`; for `git --version` and `git rev-parse` run against an arbitrary cwd.
    #[must_use]
    pub fn bare(label: &'static str, class: GitClass) -> Self {
        Self::new(label, class, None)
    }

    fn new(label: &'static str, class: GitClass, workdir: Option<PathBuf>) -> Self {
        Self {
            label,
            class,
            workdir,
            args: Vec::new(),
            secret_args: Vec::new(),
            stdin: None,
            env: Vec::new(),
        }
    }

    #[must_use]
    pub fn arg(mut self, a: impl AsRef<OsStr>) -> Self {
        self.args.push(a.as_ref().to_os_string());
        self
    }

    /// Overrides one environment variable for this command alone.
    ///
    /// Applied after the hermetic defaults, so it can replace them. There is one reason to:
    /// the defaults pin `GIT_EDITOR` and `GIT_SEQUENCE_EDITOR` to a no-op so nothing can ever
    /// hang waiting for an editor, and an environment variable beats `-c` config, so an
    /// interactive rebase cannot install its todo any other way.
    #[must_use]
    pub fn env(mut self, key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> Self {
        self.env
            .push((key.as_ref().to_os_string(), value.as_ref().to_os_string()));
        self
    }

    #[must_use]
    pub fn args<I, S>(mut self, it: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.args
            .extend(it.into_iter().map(|s| s.as_ref().to_os_string()));
        self
    }

    /// Registers the argument as sensitive: it is replaced by `<redacted>` in every log line,
    /// error and span field.
    #[must_use]
    pub fn secret_arg(mut self, a: impl AsRef<OsStr>) -> Self {
        self.secret_args.push(self.args.len());
        self.args.push(a.as_ref().to_os_string());
        self
    }

    #[must_use]
    pub fn stdin_bytes(mut self, b: Vec<u8>) -> Self {
        self.stdin = Some(b);
        self
    }

    #[must_use]
    pub const fn class(&self) -> GitClass {
        self.class
    }

    /// The full argv with secrets and URL userinfo removed. This is the only form that is
    /// ever logged or serialized.
    #[must_use]
    pub fn redacted_argv(&self, git: &Path) -> Vec<String> {
        let mut out = vec![git.display().to_string()];
        for a in self.base_args() {
            out.push(a.to_string_lossy().into_owned());
        }
        for (i, a) in self.args.iter().enumerate() {
            out.push(if self.secret_args.contains(&i) {
                "<redacted>".to_owned()
            } else {
                redact_url_userinfo(&a.to_string_lossy())
            });
        }
        out
    }

    /// The `-C` and `-c` prefix every invocation shares.
    fn base_args(&self) -> Vec<OsString> {
        let mut v: Vec<OsString> = Vec::new();
        if let Some(dir) = &self.workdir {
            v.push("-C".into());
            v.push(dir.clone().into_os_string());
        }
        if self.class.no_optional_locks() {
            v.push("--no-optional-locks".into());
        }
        for kv in [
            "core.quotepath=off",
            "color.ui=never",
            "core.pager=cat",
            "advice.detachedHead=false",
            // A read must never trigger a repack on the user's repo.
            "gc.auto=0",
        ] {
            v.push("-c".into());
            v.push(kv.into());
        }
        // Only network commands can need a credential, and only they should be able to reach
        // the helper: a read that somehow asked for one would be answered by a helper the user
        // never saw a prompt from.
        if matches!(self.class, GitClass::Network)
            && let Some((args, _)) = crate::credential::helper_config()
        {
            v.extend(args.into_iter().map(OsString::from));
        }
        v
    }
}

/// Replaces the password in `scheme://user:password@host/…` with `<redacted>`.
fn redact_url_userinfo(s: &str) -> String {
    let Some(scheme_end) = s.find("://") else {
        return s.to_owned();
    };
    let rest = &s[scheme_end + 3..];
    let Some(at) = rest.find('@') else {
        return s.to_owned();
    };
    let userinfo = &rest[..at];
    let Some(colon) = userinfo.find(':') else {
        return s.to_owned();
    };
    if userinfo.contains('/') {
        return s.to_owned();
    }
    format!(
        "{}{}:<redacted>{}",
        &s[..scheme_end + 3],
        &userinfo[..colon],
        &rest[at..]
    )
}

pub struct GitOutput {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

/// Returned by a streaming sink to end the walk early. The runner then kills the child rather
/// than draining output nobody will read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sink {
    Continue,
    Stop,
}

pub struct GitStream {
    pub records: u64,
    pub stopped_early: bool,
}

/// Locates `git` once at startup, checks its version, and runs commands through it.
#[derive(Clone, Debug)]
pub struct GitRunner {
    git: PathBuf,
    version: GitVersion,
}

impl GitRunner {
    /// Resolves git from `PATH` and enforces [`MIN_GIT`].
    ///
    /// # Errors
    /// [`CoralError::GitMissing`] if git cannot be spawned, [`CoralError::GitTooOld`] if it is
    /// older than [`MIN_GIT`].
    pub async fn discover() -> Result<Self, CoralError> {
        Self::at(PathBuf::from("git")).await
    }

    /// Same as [`GitRunner::discover`] but for an explicitly configured git.
    ///
    /// # Errors
    /// As [`GitRunner::discover`].
    pub async fn at(git: PathBuf) -> Result<Self, CoralError> {
        let probe = Self {
            git,
            version: GitVersion {
                major: 0,
                minor: 0,
                patch: 0,
            },
        };
        let out = probe
            .output(GitCommand::bare("version", GitClass::Read).arg("--version"))
            .await
            .map_err(|e| match e {
                CoralError::GitSpawn { .. } => CoralError::GitMissing,
                other => other,
            })?;
        let raw = String::from_utf8_lossy(&out.stdout).into_owned();
        let version = GitVersion::parse(&raw).ok_or(CoralError::GitVersionUnparsable {
            raw: raw.trim().to_owned(),
        })?;
        if version < MIN_GIT {
            return Err(CoralError::GitTooOld {
                found: version,
                required: MIN_GIT,
            });
        }
        Ok(Self { version, ..probe })
    }

    #[must_use]
    pub const fn version(&self) -> GitVersion {
        self.version
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.git
    }

    /// Runs a command to completion, buffering stdout. Use only where output is bounded;
    /// M1 adds the streaming variants for rev-list and status.
    ///
    /// # Errors
    /// [`CoralError::GitSpawn`] if the child cannot start, [`CoralError::GitExit`] on a
    /// non-zero exit, [`CoralError::GitSignal`] if it was killed.
    pub async fn output(&self, cmd: GitCommand) -> Result<GitOutput, CoralError> {
        let argv = cmd.redacted_argv(&self.git);
        let mut child = self.spawn(&cmd, &argv)?;

        // stdin is written on its own task rather than before reading stdout. Writing it all
        // first deadlocks as soon as the child's output fills its pipe: git stops reading our
        // input, and we are still blocked writing it. `cat-file --batch` over a screenful of
        // object ids hits this immediately.
        let writer = cmd
            .stdin
            .clone()
            .zip(child.stdin.take())
            .map(|(bytes, mut sink)| {
                tokio::spawn(async move {
                    use tokio::io::AsyncWriteExt;
                    let _ = sink.write_all(&bytes).await;
                    let _ = sink.shutdown().await;
                })
            });

        let out = child.wait_with_output().await?;
        if let Some(handle) = writer {
            let _ = handle.await;
        }
        if out.status.success() {
            return Ok(GitOutput {
                stdout: out.stdout,
                stderr: out.stderr,
            });
        }
        Err(Self::exit_error(cmd.label, argv, &out))
    }

    /// Streams stdout to `sink` one record at a time, splitting on `delim`. Never holds more
    /// than one record plus the read buffer, so it is safe on a walk of the whole kernel.
    ///
    /// # Errors
    /// As [`GitRunner::output`]. A sink returning [`Sink::Stop`] is not an error.
    pub async fn stream<F>(
        &self,
        cmd: GitCommand,
        delim: u8,
        mut sink: F,
    ) -> Result<GitStream, CoralError>
    where
        F: FnMut(&[u8]) -> Result<Sink, CoralError>,
    {
        use tokio::io::AsyncReadExt;

        let argv = cmd.redacted_argv(&self.git);
        let mut child = self.spawn(&cmd, &argv)?;
        let Some(mut out) = child.stdout.take() else {
            return Err(CoralError::Protocol {
                label: cmd.label,
                detail: "stdout was not piped".to_owned(),
            });
        };

        let mut buf = vec![0_u8; 64 * 1024];
        let mut pending: Vec<u8> = Vec::with_capacity(256);
        let mut records = 0_u64;
        let mut stopped = false;

        'read: loop {
            let n = out.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            // A record can straddle any number of reads, so anything left over is carried.
            let mut rest = &buf[..n];
            while let Some(i) = rest.iter().position(|b| *b == delim) {
                let (head, tail) = rest.split_at(i);
                rest = &tail[1..];
                let record: &[u8] = if pending.is_empty() {
                    head
                } else {
                    pending.extend_from_slice(head);
                    &pending
                };
                records += 1;
                let control = sink(record)?;
                pending.clear();
                if control == Sink::Stop {
                    stopped = true;
                    break 'read;
                }
            }
            pending.extend_from_slice(rest);
        }

        if stopped {
            // Nobody will read the rest, so do not make git finish writing it.
            let _ = child.start_kill();
            let _ = child.wait().await;
            return Ok(GitStream {
                records,
                stopped_early: true,
            });
        }

        // A trailing record with no delimiter still counts.
        if !pending.is_empty() {
            records += 1;
            sink(&pending)?;
        }

        let status = child.wait().await?;
        if status.success() {
            Ok(GitStream {
                records,
                stopped_early: false,
            })
        } else {
            let stderr = read_stderr(&mut child).await;
            Err(match status.code() {
                Some(code) => CoralError::GitExit {
                    label: cmd.label,
                    code,
                    argv,
                    stderr,
                },
                None => CoralError::GitSignal {
                    label: cmd.label,
                    signal: signal_of(status),
                    argv,
                },
            })
        }
    }
    /// Builds the fully configured child process. `tokio::process::Command` wraps this, so the
    /// async and blocking paths cannot drift in their arguments or environment.
    /// Streams stderr, splitting on carriage returns as well as newlines.
    ///
    /// `--progress` rewrites a single line with CR rather than emitting one line per update,
    /// so splitting only on newlines would deliver one enormous record at the end instead of
    /// live progress.
    ///
    /// # Errors
    /// As [`GitRunner::output`].
    pub async fn stream_err<F>(&self, cmd: GitCommand, sink: F) -> Result<(), CoralError>
    where
        F: FnMut(&[u8]) -> Result<Sink, CoralError>,
    {
        self.stream_both(cmd, |_| Ok(Sink::Continue), sink).await
    }

    /// Streams stdout by line and stderr by progress record at the same time.
    ///
    /// Both must be drained concurrently: a child that fills one pipe's buffer blocks, so
    /// reading stdout to completion before touching stderr deadlocks on any command that
    /// produces a lot of both — which is exactly what `push --porcelain --progress` does.
    ///
    /// # Errors
    /// As [`GitRunner::output`].
    pub async fn stream_both<O, E>(
        &self,
        cmd: GitCommand,
        mut on_stdout: O,
        mut on_stderr: E,
    ) -> Result<(), CoralError>
    where
        O: FnMut(&[u8]) -> Result<Sink, CoralError>,
        E: FnMut(&[u8]) -> Result<Sink, CoralError>,
    {
        use tokio::io::AsyncReadExt;

        let argv = cmd.redacted_argv(&self.git);
        let mut child = self.spawn(&cmd, &argv)?;
        let (Some(mut out), Some(mut err)) = (child.stdout.take(), child.stderr.take()) else {
            return Err(CoralError::Protocol {
                label: cmd.label,
                detail: "stdout or stderr was not piped".to_owned(),
            });
        };

        let mut out_buf = vec![0_u8; 32 * 1024];
        let mut err_buf = vec![0_u8; 32 * 1024];
        let mut out_pending: Vec<u8> = Vec::new();
        let mut err_pending: Vec<u8> = Vec::new();
        let (mut out_open, mut err_open) = (true, true);
        let mut tail = String::new();

        while out_open || err_open {
            tokio::select! {
                r = out.read(&mut out_buf), if out_open => match r? {
                    0 => out_open = false,
                    n => feed(&out_buf[..n], &mut out_pending, b"\n", &mut on_stdout)?,
                },
                r = err.read(&mut err_buf), if err_open => match r? {
                    0 => err_open = false,
                    n => {
                        // Keep the last of stderr for the error message if this fails.
                        tail.push_str(&String::from_utf8_lossy(&err_buf[..n]));
                        let keep = tail.len().saturating_sub(4096);
                        tail.drain(..keep);
                        feed(&err_buf[..n], &mut err_pending, b"\n\r", &mut on_stderr)?;
                    }
                },
            }
        }
        if !out_pending.is_empty() {
            on_stdout(&out_pending)?;
        }
        if !err_pending.is_empty() {
            on_stderr(&err_pending)?;
        }

        let status = child.wait().await?;
        if status.success() {
            return Ok(());
        }
        Err(match status.code() {
            Some(code) => CoralError::GitExit {
                label: cmd.label,
                code,
                argv,
                stderr: tail.trim().to_owned(),
            },
            None => CoralError::GitSignal {
                label: cmd.label,
                signal: signal_of(status),
                argv,
            },
        })
    }
    fn std_command(&self, cmd: &GitCommand) -> std::process::Command {
        let mut c = std::process::Command::new(&self.git);
        c.args(cmd.base_args())
            .args(&cmd.args)
            .stdin(if cmd.stdin.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        hide_console(&mut c);
        apply_env(&mut c);
        if matches!(cmd.class, GitClass::Network)
            && let Some((_, session)) = crate::credential::helper_config()
        {
            c.env(crate::credential::SESSION_VAR, session);
        }
        for (key, value) in &cmd.env {
            c.env(key, value);
        }
        c
    }

    /// Synchronous [`GitRunner::stream`], for callers that are already on a blocking thread.
    ///
    /// [`crate::graph::CommitStream`] is synchronous by design, so the subprocess walk uses
    /// this rather than borrowing a runtime handle: `block_on` panics when the calling thread
    /// is already driving the runtime.
    ///
    /// # Errors
    /// As [`GitRunner::stream`].
    pub fn stream_blocking<F>(
        &self,
        cmd: &GitCommand,
        delim: u8,
        mut sink: F,
    ) -> Result<GitStream, CoralError>
    where
        F: FnMut(&[u8]) -> Result<Sink, CoralError>,
    {
        use std::io::Read;

        let argv = cmd.redacted_argv(&self.git);
        let mut child = self
            .std_command(cmd)
            .spawn()
            .map_err(|source| CoralError::GitSpawn {
                label: cmd.label,
                argv: argv.clone(),
                source,
            })?;
        let Some(mut out) = child.stdout.take() else {
            return Err(CoralError::Protocol {
                label: cmd.label,
                detail: "stdout was not piped".to_owned(),
            });
        };

        let mut buf = vec![0_u8; 64 * 1024];
        let mut pending: Vec<u8> = Vec::with_capacity(256);
        let mut records = 0_u64;
        let mut stopped = false;

        'read: loop {
            let n = out.read(&mut buf)?;
            if n == 0 {
                break;
            }
            let mut rest = &buf[..n];
            while let Some(i) = rest.iter().position(|b| *b == delim) {
                let (head, tail) = rest.split_at(i);
                rest = &tail[1..];
                let record: &[u8] = if pending.is_empty() {
                    head
                } else {
                    pending.extend_from_slice(head);
                    &pending
                };
                records += 1;
                let control = sink(record)?;
                pending.clear();
                if control == Sink::Stop {
                    stopped = true;
                    break 'read;
                }
            }
            pending.extend_from_slice(rest);
        }

        if stopped {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(GitStream {
                records,
                stopped_early: true,
            });
        }
        if !pending.is_empty() {
            records += 1;
            sink(&pending)?;
        }

        let status = child.wait()?;
        if status.success() {
            return Ok(GitStream {
                records,
                stopped_early: false,
            });
        }
        let mut stderr = String::new();
        if let Some(mut e) = child.stderr.take() {
            let _ = e.read_to_string(&mut stderr);
        }
        Err(match status.code() {
            Some(code) => CoralError::GitExit {
                label: cmd.label,
                code,
                argv,
                stderr: stderr.trim().to_owned(),
            },
            None => CoralError::GitSignal {
                label: cmd.label,
                signal: signal_of(status),
                argv,
            },
        })
    }

    fn spawn(
        &self,
        cmd: &GitCommand,
        argv: &[String],
    ) -> Result<tokio::process::Child, CoralError> {
        let mut c = tokio::process::Command::from(self.std_command(cmd));
        c.kill_on_drop(true);
        c.spawn().map_err(|source| CoralError::GitSpawn {
            label: cmd.label,
            argv: argv.to_vec(),
            source,
        })
    }

    fn exit_error(
        label: &'static str,
        argv: Vec<String>,
        out: &std::process::Output,
    ) -> CoralError {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_owned();
        match out.status.code() {
            Some(code) => CoralError::GitExit {
                label,
                code,
                argv,
                stderr,
            },
            None => CoralError::GitSignal {
                label,
                signal: signal_of(out.status),
                argv,
            },
        }
    }
}

/// Splits `chunk` on any of `delims`, carrying an incomplete trailing record in `pending`.
fn feed<F>(
    chunk: &[u8],
    pending: &mut Vec<u8>,
    delims: &[u8],
    sink: &mut F,
) -> Result<(), CoralError>
where
    F: FnMut(&[u8]) -> Result<Sink, CoralError>,
{
    let mut rest = chunk;
    while let Some(i) = rest.iter().position(|b| delims.contains(b)) {
        let (head, tail) = rest.split_at(i);
        rest = &tail[1..];
        if pending.is_empty() {
            if !head.is_empty() {
                sink(head)?;
            }
        } else {
            pending.extend_from_slice(head);
            sink(pending)?;
            pending.clear();
        }
    }
    pending.extend_from_slice(rest);
    Ok(())
}

async fn read_stderr(child: &mut tokio::process::Child) -> String {
    use tokio::io::AsyncReadExt;
    let mut s = String::new();
    if let Some(mut e) = child.stderr.take() {
        let _ = e.read_to_string(&mut s).await;
    }
    s.trim().to_owned()
}

#[cfg(unix)]
fn signal_of(status: std::process::ExitStatus) -> i32 {
    use std::os::unix::process::ExitStatusExt;
    status.signal().unwrap_or(0)
}

#[cfg(not(unix))]
fn signal_of(_status: std::process::ExitStatus) -> i32 {
    0
}

/// Stops a child opening a console window of its own.
///
/// On Windows a process with a console subsystem gets one whether or not anything is written to
/// it, and Coral runs git for everything it shows: opening a repository alone is a status, a
/// ref listing and a graph walk, and the file watcher runs them again on every change. Without
/// this the window is a stream of black rectangles appearing and vanishing.
///
/// `CREATE_NO_WINDOW` rather than `DETACHED_PROCESS`: the child keeps its standard handles,
/// which are the pipes every one of these reads its answer from.
pub fn hide_console(c: &mut std::process::Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        c.creation_flags(CREATE_NO_WINDOW);
    }
    // Nothing to do anywhere else; the parameter is used only on Windows.
    let _ = c;
}

/// Pins the locale so stderr fingerprinting is sound, and clears every git environment
/// variable that would silently redirect the command somewhere else.
fn apply_env(c: &mut std::process::Command) {
    for (k, v) in [
        ("LC_ALL", "C"),
        ("LANG", "C"),
        ("GIT_TERMINAL_PROMPT", "0"),
        ("GIT_PAGER", "cat"),
        ("GIT_EDITOR", ":"),
        ("GIT_SEQUENCE_EDITOR", ":"),
        ("SSH_ASKPASS_REQUIRE", "never"),
    ] {
        c.env(k, v);
    }
    for k in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_COMMON_DIR",
        "GIT_INDEX_FILE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_CONFIG",
        "GIT_NAMESPACE",
        "GIT_PREFIX",
        "GIT_ASKPASS",
        "GIT_LITERAL_PATHSPECS",
        "GIT_GLOB_PATHSPECS",
        "GIT_NOGLOB_PATHSPECS",
        "GIT_ICASE_PATHSPECS",
    ] {
        c.env_remove(k);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_and_vendor_versions() {
        assert_eq!(
            GitVersion::parse("git version 2.43.0\n"),
            Some(GitVersion {
                major: 2,
                minor: 43,
                patch: 0
            })
        );
        assert_eq!(
            GitVersion::parse("git version 2.39.3 (Apple Git-146)"),
            Some(GitVersion {
                major: 2,
                minor: 39,
                patch: 3
            })
        );
        assert_eq!(
            GitVersion::parse("git version 2.45.1.windows.1"),
            Some(GitVersion {
                major: 2,
                minor: 45,
                patch: 1
            })
        );
        assert_eq!(GitVersion::parse("not git at all"), None);
    }

    #[test]
    fn version_ordering_matches_min_git() {
        assert!(
            GitVersion {
                major: 2,
                minor: 39,
                patch: 9
            } < MIN_GIT
        );
        assert!(
            GitVersion {
                major: 2,
                minor: 40,
                patch: 0
            } >= MIN_GIT
        );
        assert!(
            GitVersion {
                major: 3,
                minor: 0,
                patch: 0
            } >= MIN_GIT
        );
    }

    #[test]
    fn redacts_only_the_password_in_a_remote_url() {
        assert_eq!(
            redact_url_userinfo("https://user:ghp_secret@github.com/o/r.git"),
            "https://user:<redacted>@github.com/o/r.git"
        );
        assert_eq!(
            redact_url_userinfo("https://github.com/o/r.git"),
            "https://github.com/o/r.git"
        );
        // An `@` in the path is not userinfo.
        assert_eq!(
            redact_url_userinfo("https://github.com/o/r@v1.git"),
            "https://github.com/o/r@v1.git"
        );
    }

    #[test]
    fn redacted_argv_hides_secret_args_and_keeps_base_flags() {
        let cmd = GitCommand::read("test", "/repo")
            .arg("ls-remote")
            .secret_arg("token123");
        let argv = cmd.redacted_argv(Path::new("git"));
        assert!(argv.contains(&"<redacted>".to_owned()));
        assert!(!argv.iter().any(|a| a.contains("token123")));
        assert!(argv.contains(&"--no-optional-locks".to_owned()));
    }

    #[test]
    fn status_class_keeps_the_index_writable() {
        assert!(!GitClass::Status.no_optional_locks());
        assert!(GitClass::Read.no_optional_locks());
    }
}

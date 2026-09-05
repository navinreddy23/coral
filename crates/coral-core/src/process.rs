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

/// Where a git installation is, when it is not simply `git` on `PATH`.
///
/// Not one file. git dispatches to helper programs in `libexec/git-core` and seeds a new
/// repository from `share/git-core/templates`, and a copy carried somewhere unusual finds
/// neither unless it is told where they are.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitInstall {
    /// The `git` binary to run.
    pub program: PathBuf,
    /// `libexec/git-core`, passed as `GIT_EXEC_PATH`.
    pub exec_path: Option<PathBuf>,
    /// `share/git-core/templates`, passed as `GIT_TEMPLATE_DIR`.
    pub templates: Option<PathBuf>,
}

/// Names a git to use instead of the one on `PATH`: either the binary itself, or the prefix a
/// full installation sits under.
pub const GIT_VAR: &str = "CORAL_GIT";

impl GitInstall {
    /// Whatever `git` resolves to on `PATH`, with nothing else configured.
    #[must_use]
    pub fn on_path() -> Self {
        Self {
            program: PathBuf::from("git"),
            exec_path: None,
            templates: None,
        }
    }

    /// The installation under `prefix`, or `None` when there is no git there.
    ///
    /// The layout is the one `make prefix=… install` produces, which is what the bundle is.
    #[must_use]
    pub fn at_prefix(prefix: &Path) -> Option<Self> {
        let program = prefix.join("bin").join(git_binary());
        if !program.is_file() {
            return None;
        }
        Some(Self {
            program,
            exec_path: directory(prefix.join("libexec").join("git-core")),
            templates: directory(prefix.join("share").join("git-core").join("templates")),
        })
    }

    /// What `CORAL_GIT` names, if anything: either a git binary or the prefix of a full
    /// installation.
    #[must_use]
    pub fn from_env() -> Option<Self> {
        let named = PathBuf::from(std::env::var_os(GIT_VAR)?);
        if named.is_dir() {
            return Self::at_prefix(&named);
        }
        if !named.is_file() {
            return None;
        }
        Some(Self {
            program: named,
            exec_path: None,
            templates: None,
        })
    }

    /// The git this build carries with it, if it carries one.
    ///
    /// Found but never preferred on its own. It is offered as a choice, and running it because
    /// it happens to be there would mean the application quietly used a different git from the
    /// command line beside it.
    #[must_use]
    pub fn bundled() -> Option<Self> {
        bundle_prefixes(
            std::env::current_exe().ok().as_deref(),
            std::env::var_os("APPDIR").map(PathBuf::from).as_deref(),
        )
        .iter()
        .find_map(|prefix| Self::at_prefix(prefix))
    }
}

/// The git every later [`GitRunner::discover`] should use, once something has chosen one.
///
/// Process-wide, because that is what it is: which git the application runs is not a property
/// of a repository or of a command, and threading it through every call site would put the same
/// value in a hundred signatures for the sake of one screen that sets it. `None` puts the
/// search back to where it starts.
pub fn use_git(install: Option<GitInstall>) {
    if let Ok(mut chosen) = chosen().write() {
        *chosen = install;
    }
}

/// What [`use_git`] was last given.
#[must_use]
pub fn chosen_git() -> Option<GitInstall> {
    chosen().read().ok()?.clone()
}

fn chosen() -> &'static std::sync::RwLock<Option<GitInstall>> {
    static CHOSEN: std::sync::OnceLock<std::sync::RwLock<Option<GitInstall>>> =
        std::sync::OnceLock::new();
    CHOSEN.get_or_init(|| std::sync::RwLock::new(None))
}

/// Where a bundled git could be, given where this binary is and what packaged it.
///
/// Two layouts and no search. An `AppImage` says where it was mounted, and everything inside
/// it is at a fixed place under that; anything unpacked from an archive keeps git beside the
/// binary. Guessing more widely would mean a Coral that silently ran some other git.
fn bundle_prefixes(exe: Option<&Path>, appdir: Option<&Path>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(appdir) = appdir {
        out.push(appdir.join("usr").join("lib").join("coral").join("git"));
    }
    if let Some(dir) = exe.and_then(Path::parent) {
        out.push(dir.join("git"));
    }
    out
}

const fn git_binary() -> &'static str {
    if cfg!(windows) { "git.exe" } else { "git" }
}

fn directory(path: PathBuf) -> Option<PathBuf> {
    path.is_dir().then_some(path)
}

/// Locates `git` once at startup, checks its version, and runs commands through it.
#[derive(Clone, Debug)]
pub struct GitRunner {
    git: GitInstall,
    version: GitVersion,
}

impl GitRunner {
    /// Resolves git and enforces [`MIN_GIT`].
    ///
    /// What [`use_git`] chose, else what `CORAL_GIT` names, else `PATH`. A git that was chosen
    /// and then cannot be run falls back rather than leaving the user with an application that
    /// does nothing, and says so: the settings screen reports which git is actually in use, so
    /// a fallback is visible rather than silent.
    ///
    /// # Errors
    /// [`CoralError::GitMissing`] if git cannot be spawned, [`CoralError::GitTooOld`] if it is
    /// older than [`MIN_GIT`].
    pub async fn discover() -> Result<Self, CoralError> {
        let Some(asked) = chosen_git().or_else(GitInstall::from_env) else {
            return Self::install(GitInstall::on_path()).await;
        };
        match Self::install(asked.clone()).await {
            Ok(runner) => Ok(runner),
            Err(e) => {
                tracing::warn!(
                    git = %asked.program.display(),
                    error = %e,
                    "the chosen git could not be used; falling back to the one on PATH"
                );
                Self::install(GitInstall::on_path()).await
            }
        }
    }

    /// Same as [`GitRunner::discover`] but for an explicitly named git binary.
    ///
    /// # Errors
    /// As [`GitRunner::discover`].
    pub async fn at(git: PathBuf) -> Result<Self, CoralError> {
        Self::install(GitInstall {
            program: git,
            exec_path: None,
            templates: None,
        })
        .await
    }

    /// Same as [`GitRunner::discover`] but for an installation already located.
    ///
    /// # Errors
    /// As [`GitRunner::discover`].
    pub async fn install(git: GitInstall) -> Result<Self, CoralError> {
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
        &self.git.program
    }

    /// The whole installation, for anything that has to describe where git came from.
    #[must_use]
    pub const fn located(&self) -> &GitInstall {
        &self.git
    }

    /// Runs a command to completion, buffering stdout. Use only where output is bounded;
    /// M1 adds the streaming variants for rev-list and status.
    ///
    /// # Errors
    /// [`CoralError::GitSpawn`] if the child cannot start, [`CoralError::GitExit`] on a
    /// non-zero exit, [`CoralError::GitSignal`] if it was killed.
    pub async fn output(&self, cmd: GitCommand) -> Result<GitOutput, CoralError> {
        self.output_allowing(cmd, &[]).await
    }

    /// Runs a command whose non-zero exit is an answer rather than a failure.
    ///
    /// `diff --no-index` exits 1 to say the two paths differ, which is the whole reason for
    /// running it. Anything outside `ok` is still an error.
    ///
    /// # Errors
    /// As [`GitRunner::output`], for every code but those in `ok`.
    pub async fn output_allowing(
        &self,
        cmd: GitCommand,
        ok: &[i32],
    ) -> Result<GitOutput, CoralError> {
        let argv = cmd.redacted_argv(&self.git.program);
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
        if out.status.success() || out.status.code().is_some_and(|code| ok.contains(&code)) {
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

        let argv = cmd.redacted_argv(&self.git.program);
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

        let argv = cmd.redacted_argv(&self.git.program);
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
        let mut c = std::process::Command::new(&self.git.program);
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
        // A git that is not the system's has to be told where its own helpers and templates
        // are; one that is must not inherit someone else's answer to the same question.
        match &self.git.exec_path {
            Some(path) => c.env("GIT_EXEC_PATH", path),
            None => c.env_remove("GIT_EXEC_PATH"),
        };
        match &self.git.templates {
            Some(path) => c.env("GIT_TEMPLATE_DIR", path),
            None => c.env_remove("GIT_TEMPLATE_DIR"),
        };
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

        let argv = cmd.redacted_argv(&self.git.program);
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
        let stderr = why_it_failed(&String::from_utf8_lossy(&out.stderr));
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

    /// A prefix laid out the way `make prefix=… install` lays one out.
    fn fake_install(root: &Path, with_extras: bool) -> PathBuf {
        let prefix = root.join("git");
        std::fs::create_dir_all(prefix.join("bin")).unwrap();
        std::fs::write(prefix.join("bin").join(git_binary()), b"#!/bin/sh\n").unwrap();
        if with_extras {
            std::fs::create_dir_all(prefix.join("libexec").join("git-core")).unwrap();
            std::fs::create_dir_all(prefix.join("share").join("git-core").join("templates"))
                .unwrap();
        }
        prefix
    }

    #[test]
    fn reads_a_prefix_as_a_whole_installation() {
        // The helpers and the templates matter as much as the binary: without `GIT_EXEC_PATH`
        // a git carried somewhere unusual cannot find `git-remote-https`, and every fetch over
        // https fails with a message about an unsupported protocol.
        let root = tempfile::tempdir().unwrap();
        let prefix = fake_install(root.path(), true);

        let found = GitInstall::at_prefix(&prefix).expect("a prefix with a git in it");
        assert_eq!(found.program, prefix.join("bin").join(git_binary()));
        assert_eq!(
            found.exec_path,
            Some(prefix.join("libexec").join("git-core"))
        );
        assert_eq!(
            found.templates,
            Some(prefix.join("share").join("git-core").join("templates"))
        );
    }

    #[test]
    fn a_prefix_without_the_extras_still_names_the_binary() {
        let root = tempfile::tempdir().unwrap();
        let prefix = fake_install(root.path(), false);

        let found = GitInstall::at_prefix(&prefix).expect("a prefix with a git in it");
        assert_eq!(found.exec_path, None);
        assert_eq!(found.templates, None);
    }

    #[test]
    fn a_directory_with_no_git_in_it_is_not_an_installation() {
        let root = tempfile::tempdir().unwrap();
        assert_eq!(GitInstall::at_prefix(root.path()), None);
        assert_eq!(GitInstall::at_prefix(&root.path().join("nowhere")), None);
    }

    #[test]
    fn looks_where_the_two_packagings_put_it_and_nowhere_else() {
        // Two fixed places, not a search. A Coral that hunted for a git would eventually find
        // one that is not the one it shipped, and the user would have no way to tell.
        let exe = PathBuf::from("/opt/coral/bin/coral-app");
        let appdir = PathBuf::from("/tmp/.mount_Coral");

        let both = bundle_prefixes(Some(&exe), Some(&appdir));
        assert_eq!(
            both,
            vec![
                PathBuf::from("/tmp/.mount_Coral/usr/lib/coral/git"),
                PathBuf::from("/opt/coral/bin/git"),
            ]
        );
        // The AppImage's answer comes first, because inside one both are true and only that
        // one is the copy this build shipped.
        assert_eq!(bundle_prefixes(None, Some(&appdir)).len(), 1);
        assert_eq!(bundle_prefixes(Some(&exe), None).len(), 1);
        assert!(bundle_prefixes(None, None).is_empty());
    }

    #[test]
    fn on_path_asks_for_nothing_in_particular() {
        // The system's git knows where its own helpers are, and telling it would only be a way
        // of getting it wrong.
        let plain = GitInstall::on_path();
        assert_eq!(plain.program, PathBuf::from("git"));
        assert_eq!(plain.exec_path, None);
        assert_eq!(plain.templates, None);
    }

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

/// What git said went wrong, without what it said while it was working.
///
/// A network command writes its progress to stderr and its reason at the end, so a failed pull
/// was reported as "remote: Enumerating objects: 5, done. remote: Counting objects: 20% (1/5)…"
/// — two hundred characters of counting where "Not possible to fast-forward" was the answer.
///
/// Where git names the failure outright, that is the whole message: its `hint:` lines advise
/// running `git merge --no-ff` or `git rebase`, which is advice for a terminal and not for a
/// window whose Pull button offers both. Otherwise everything that is not a counter, and
/// failing that the last few lines, because a message nobody wrote a rule for beats none.
fn why_it_failed(stderr: &str) -> String {
    // Progress is redrawn with carriage returns, so "Rebasing (1/1)\rerror: could not apply" is
    // one line to `lines()` and hides the `error:` that names the failure. Only what survives
    // the last redraw of each line counts, which is also all a terminal would have shown.
    let shown: Vec<String> = stderr
        .lines()
        .map(|line| {
            line.rsplit('\r')
                .next()
                .unwrap_or(line)
                .trim_end()
                .to_owned()
        })
        .collect();
    let lines: Vec<&str> = shown.iter().map(String::as_str).collect();
    let named: Vec<&str> = lines
        .iter()
        .copied()
        .filter(|l| names_a_failure(l))
        .collect();
    if !named.is_empty() {
        return named.join("\n");
    }

    let kept: Vec<&str> = lines
        .iter()
        .copied()
        .filter(|line| !line.trim().is_empty() && !is_progress(line))
        .collect();
    if kept.is_empty() {
        let tail = lines.len().saturating_sub(3);
        return lines[tail..].join("\n").trim().to_owned();
    }
    kept.join("\n")
}

/// A line that says what went wrong rather than what to do about it.
fn names_a_failure(line: &str) -> bool {
    let bare = line.trim().strip_prefix("remote:").unwrap_or(line).trim();
    bare.starts_with("fatal:") || bare.starts_with("error:") || bare.starts_with("ERROR:")
}

/// A line git writes to say it is still going.
fn is_progress(line: &str) -> bool {
    const COUNTERS: [&str; 6] = [
        "Enumerating objects:",
        "Counting objects:",
        "Compressing objects:",
        "Writing objects:",
        "Receiving objects:",
        "Resolving deltas:",
    ];
    let bare = line.trim().strip_prefix("remote:").unwrap_or(line).trim();
    COUNTERS.iter().any(|c| bare.starts_with(c))
        || bare.starts_with("Total ")
        || bare.starts_with("Unpacking objects:")
        || bare.starts_with("Updating files:")
}

#[cfg(test)]
mod failure_tests {
    use super::why_it_failed;

    #[test]
    fn keeps_the_reason_and_drops_the_counting() {
        // What a diverged `git pull --ff-only` actually writes, progress first.
        let stderr = "remote: Enumerating objects: 5, done.\n\
                      remote: Counting objects: 100% (5/5), done.\n\
                      remote: Compressing objects: 100% (2/2), done.\n\
                      remote: Total 3 (delta 1), reused 0 (delta 0)\n\
                      hint: Diverging branches can't be fast-forwarded\n\
                      fatal: Not possible to fast-forward, aborting.";
        // The reason alone: git's hints say to run `git merge --no-ff` or `git rebase`, which
        // is advice for a terminal, not for a window whose Pull button offers both.
        assert_eq!(
            why_it_failed(stderr),
            "fatal: Not possible to fast-forward, aborting."
        );
    }

    #[test]
    fn sees_past_a_line_git_redrew() {
        // What a stopped `git pull --rebase` writes: the progress counter and the failure share
        // one line, separated by a carriage return.
        let stderr = "From gitlab.com:someone/repo\n\
                      Rebasing (1/1)\rerror: could not apply 051e21f… we changed hello";
        assert_eq!(
            why_it_failed(stderr),
            "error: could not apply 051e21f… we changed hello"
        );
    }

    #[test]
    fn keeps_the_advice_when_nothing_names_the_failure() {
        let stderr = "remote: Counting objects: 100% (5/5), done.\nhint: something to try";
        assert_eq!(why_it_failed(stderr), "hint: something to try");
    }

    #[test]
    fn keeps_what_the_host_says_even_though_it_starts_with_remote() {
        let stderr = "remote: Enumerating objects: 1, done.\n\
                      remote: ERROR: The project could not be found";
        assert_eq!(
            why_it_failed(stderr),
            "remote: ERROR: The project could not be found"
        );
    }

    #[test]
    fn falls_back_to_the_last_of_it_when_every_line_is_progress() {
        let stderr = "remote: Counting objects: 50% (1/2)\nremote: Counting objects: 100% (2/2)";
        assert_eq!(
            why_it_failed(stderr),
            "remote: Counting objects: 50% (1/2)\nremote: Counting objects: 100% (2/2)"
        );
    }
}

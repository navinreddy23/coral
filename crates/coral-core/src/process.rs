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
        }
    }

    #[must_use]
    pub fn arg(mut self, a: impl AsRef<OsStr>) -> Self {
        self.args.push(a.as_ref().to_os_string());
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

        if let (Some(bytes), Some(mut sink)) = (&cmd.stdin, child.stdin.take()) {
            use tokio::io::AsyncWriteExt;
            sink.write_all(bytes).await?;
            sink.shutdown().await?;
        }

        let out = child.wait_with_output().await?;
        if out.status.success() {
            return Ok(GitOutput {
                stdout: out.stdout,
                stderr: out.stderr,
            });
        }
        Err(Self::exit_error(cmd.label, argv, &out))
    }

    fn spawn(
        &self,
        cmd: &GitCommand,
        argv: &[String],
    ) -> Result<tokio::process::Child, CoralError> {
        let mut c = tokio::process::Command::new(&self.git);
        c.args(cmd.base_args())
            .args(&cmd.args)
            .stdin(if cmd.stdin.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        apply_env(&mut c);
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

#[cfg(unix)]
fn signal_of(status: std::process::ExitStatus) -> i32 {
    use std::os::unix::process::ExitStatusExt;
    status.signal().unwrap_or(0)
}

#[cfg(not(unix))]
fn signal_of(_status: std::process::ExitStatus) -> i32 {
    0
}

/// Pins the locale so stderr fingerprinting is sound, and clears every git environment
/// variable that would silently redirect the command somewhere else.
fn apply_env(c: &mut tokio::process::Command) {
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

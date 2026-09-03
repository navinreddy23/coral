use std::path::{Path, PathBuf};
use std::process::Command;

/// A throwaway repository built by shelling out to git, with every source of machine-specific
/// variation pinned so snapshots are byte-stable across developers and CI.
pub struct TestRepo {
    dir: tempfile::TempDir,
}

const EPOCH: &str = "2005-04-07T15:13:13-07:00";

impl TestRepo {
    /// Creates an empty repository whose initial branch is `main`.
    ///
    /// # Panics
    /// If git is missing or `git init` fails — a broken fixture is a broken test run, not a
    /// condition callers can handle.
    #[must_use]
    pub fn new() -> Self {
        let repo = Self {
            dir: tempfile::tempdir().expect("temp dir"),
        };
        repo.git(["init", "--initial-branch=main", "--quiet"]);
        repo.git(["config", "user.name", "Coral Fixture"]);
        repo.git(["config", "user.email", "fixture@coral.test"]);
        repo.git(["config", "core.autocrlf", "false"]);
        repo.git(["config", "commit.gpgsign", "false"]);
        repo
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    /// Writes a file relative to the worktree root, creating parent directories.
    ///
    /// # Panics
    /// If the write fails.
    #[must_use]
    pub fn write(self, rel: &str, contents: &str) -> Self {
        let full: PathBuf = self.dir.path().join(rel);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).expect("create parent");
        }
        std::fs::write(full, contents).expect("write fixture file");
        self
    }

    /// Stages everything and commits with fixed author and committer timestamps.
    ///
    /// # Panics
    /// If git fails.
    #[must_use]
    pub fn commit(self, message: &str) -> Self {
        self.git(["add", "--all"]);
        self.git(["commit", "--quiet", "--allow-empty", "-m", message]);
        self
    }

    /// Runs git and returns trimmed stdout.
    ///
    /// # Panics
    /// If the command cannot be spawned or exits non-zero.
    pub fn git<I, S>(&self, args: I) -> String
    where
        I: IntoIterator<Item = S>,
        S: AsRef<std::ffi::OsStr>,
    {
        String::from_utf8_lossy(&self.git_bytes(args))
            .trim()
            .to_owned()
    }

    /// Raw stdout, for NUL-delimited output that must not be trimmed or lossily decoded.
    ///
    /// # Panics
    /// If the command cannot be spawned or exits non-zero.
    pub fn git_bytes<I, S>(&self, args: I) -> Vec<u8>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<std::ffi::OsStr>,
    {
        let out = self.command(args).output().expect("spawn git");
        assert!(
            out.status.success(),
            "git failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        out.stdout
    }

    /// One hermetic git invocation. Unlike the engine's runner, a fixture ignores the
    /// developer's global and system config entirely and pins both identities and dates.
    fn command<I, S>(&self, args: I) -> Command
    where
        I: IntoIterator<Item = S>,
        S: AsRef<std::ffi::OsStr>,
    {
        let mut c = Command::new("git");
        c.current_dir(self.dir.path())
            .args(args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_AUTHOR_NAME", "Coral Fixture")
            .env("GIT_AUTHOR_EMAIL", "fixture@coral.test")
            .env("GIT_AUTHOR_DATE", EPOCH)
            .env("GIT_COMMITTER_NAME", "Coral Fixture")
            .env("GIT_COMMITTER_EMAIL", "fixture@coral.test")
            .env("GIT_COMMITTER_DATE", EPOCH)
            .env("LC_ALL", "C");
        c
    }
}

impl Default for TestRepo {
    fn default() -> Self {
        Self::new()
    }
}

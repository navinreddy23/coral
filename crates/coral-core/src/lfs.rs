//! Git LFS beyond reading files: putting a file type in it, and locking files on its server.
//!
//! All of it is git-lfs's own commands. Tracking edits `.gitattributes`; locking asks the LFS
//! server, which is the only place a lock exists (see `git lfs help lock`), so those calls go
//! over the network and are given a time limit here rather than left to hang a panel.

use std::time::Duration;

use crate::error::CoralError;
use crate::process::{GitCommand, GitRunner};
use crate::repo::RepoLocation;

/// How long the LFS server gets to answer about locks.
pub const SERVER_LIMIT: Duration = Duration::from_secs(15);

/// A file locked on the LFS server.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub struct LfsLock {
    pub id: String,
    pub path: String,
    /// Who holds it, as the server names them; empty when it does not say.
    pub owner: String,
    /// Whether it is held by whoever this repository authenticates as.
    pub ours: bool,
}

#[derive(serde::Deserialize)]
struct RawLock {
    id: String,
    path: String,
    #[serde(default)]
    owner: Option<RawOwner>,
}

#[derive(serde::Deserialize)]
struct RawOwner {
    #[serde(default)]
    name: String,
}

#[derive(serde::Deserialize)]
struct Verified {
    #[serde(default)]
    ours: Vec<RawLock>,
    #[serde(default)]
    theirs: Vec<RawLock>,
}

impl RawLock {
    fn held(self, ours: bool) -> LfsLock {
        LfsLock {
            id: self.id,
            path: self.path,
            owner: self.owner.map(|o| o.name).unwrap_or_default(),
            ours,
        }
    }
}

/// Reads `git lfs locks --verify --json`: the locks held as this repository's user, and the
/// rest. The split is the server's, which is the only thing that knows who a lock belongs to.
///
/// # Errors
/// [`CoralError::Protocol`] when the output is not that.
pub fn parse_locks(out: &[u8]) -> Result<Vec<LfsLock>, CoralError> {
    let text = String::from_utf8_lossy(out);
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    let verified: Verified = serde_json::from_str(&text).map_err(|e| CoralError::Protocol {
        label: "lfs locks",
        detail: e.to_string(),
    })?;
    let mut locks: Vec<LfsLock> = verified.ours.into_iter().map(|l| l.held(true)).collect();
    locks.extend(verified.theirs.into_iter().map(|l| l.held(false)));
    Ok(locks)
}

impl RepoLocation {
    /// Whether the top-level `.gitattributes` puts anything in LFS.
    ///
    /// The cheap question asked before the expensive one: listing locks goes to a server, and a
    /// repository that keeps nothing in LFS has no locks and no reason to make that trip.
    #[must_use]
    pub fn uses_lfs(&self) -> bool {
        self.workdir
            .as_ref()
            .and_then(|root| std::fs::read_to_string(root.join(".gitattributes")).ok())
            .is_some_and(|text| text.contains("filter=lfs"))
    }

    /// Puts files matching `pattern` in LFS from now on, setting LFS up here first if needed.
    ///
    /// Files already committed are left as they are: tracking says how the next version of
    /// each is stored, and rewriting history is `git lfs migrate`, which is not this.
    ///
    /// # Errors
    /// Propagates git-lfs failures, including its absence.
    pub async fn lfs_track(&self, runner: &GitRunner, pattern: &str) -> Result<(), CoralError> {
        if !self.lfs_installed(runner).await? {
            // `--local`, as when a repository is created with LFS: this repository, not the
            // user's whole configuration.
            runner
                .output(
                    GitCommand::write("lfs-install", self.display_path())
                        .args(["lfs", "install", "--local"]),
                )
                .await?;
        }
        runner
            .output(
                GitCommand::write("lfs-track", self.display_path()).args(["lfs", "track", pattern]),
            )
            .await
            .map(|_| ())
    }

    /// Whether git-lfs's filter is configured for this repository, at any level.
    async fn lfs_installed(&self, runner: &GitRunner) -> Result<bool, CoralError> {
        let out = runner
            .output_allowing(
                GitCommand::read("config", self.display_path()).args([
                    "config",
                    "--get",
                    "filter.lfs.process",
                ]),
                &[1],
            )
            .await?;
        Ok(out.code == 0 && !out.stdout.trim_ascii().is_empty())
    }

    /// The locks the LFS server holds for this repository.
    ///
    /// # Errors
    /// The server's refusal, or [`CoralError::Refused`] when it does not answer in time.
    pub async fn lfs_locks(&self, runner: &GitRunner) -> Result<Vec<LfsLock>, CoralError> {
        let out = self
            .lfs_server(runner, "lfs-locks", &["lfs", "locks", "--verify", "--json"])
            .await?;
        parse_locks(&out)
    }

    /// Locks `path` on the LFS server, so others are told it is being edited.
    ///
    /// # Errors
    /// The server's refusal, such as a lock someone else holds.
    pub async fn lfs_lock(&self, runner: &GitRunner, path: &str) -> Result<(), CoralError> {
        self.lfs_server(runner, "lfs-lock", &["lfs", "lock", "--", path])
            .await
            .map(|_| ())
    }

    /// Releases the lock on `path`. `force` releases one held by somebody else, which the
    /// server allows only to those it trusts with it.
    ///
    /// # Errors
    /// The server's refusal.
    pub async fn lfs_unlock(
        &self,
        runner: &GitRunner,
        path: &str,
        force: bool,
    ) -> Result<(), CoralError> {
        let mut args = vec!["lfs", "unlock"];
        if force {
            args.push("--force");
        }
        args.extend(["--", path]);
        self.lfs_server(runner, "lfs-unlock", &args)
            .await
            .map(|_| ())
    }

    async fn lfs_server(
        &self,
        runner: &GitRunner,
        label: &'static str,
        args: &[&str],
    ) -> Result<Vec<u8>, CoralError> {
        let cmd = GitCommand::network(label, self.display_path()).args(args);
        match tokio::time::timeout(SERVER_LIMIT, runner.output(cmd)).await {
            Ok(out) => Ok(out?.stdout),
            Err(_) => Err(CoralError::Refused {
                label,
                detail: format!(
                    "the LFS server did not answer within {} seconds",
                    SERVER_LIMIT.as_secs()
                ),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{LfsLock, parse_locks};

    #[test]
    fn reads_whose_each_lock_is() {
        let out = br#"{"ours":[{"id":"1","path":"a.psd","owner":{"name":"Ada"},"locked_at":"x"}],"theirs":[{"id":"2","path":"art/b.psd","locked_at":"x"}]}"#;
        assert_eq!(
            parse_locks(out).unwrap(),
            [
                LfsLock {
                    id: "1".into(),
                    path: "a.psd".into(),
                    owner: "Ada".into(),
                    ours: true
                },
                LfsLock {
                    id: "2".into(),
                    path: "art/b.psd".into(),
                    owner: String::new(),
                    ours: false
                },
            ]
        );
    }

    #[test]
    fn no_output_is_no_locks() {
        assert_eq!(parse_locks(b"").unwrap(), []);
        assert_eq!(parse_locks(b"{\"ours\":[],\"theirs\":[]}\n").unwrap(), []);
    }
}

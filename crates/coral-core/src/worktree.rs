//! Linked working trees.
//!
//! A second checkout of the same repository, sharing its object store. Creating one from a
//! commit is how a commit is examined without disturbing the checkout the user is working in,
//! which is what the reference offers from the commit menu.

use std::path::{Path, PathBuf};

use bstr::ByteSlice;

use crate::error::CoralError;
use crate::process::{GitCommand, GitRunner};
use crate::repo::RepoLocation;

/// One working tree attached to the repository, the main one included.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub struct Worktree {
    pub path: String,
    /// Empty for a worktree that has never been checked out.
    pub head: String,
    /// The branch checked out there, short form. None when the head is detached.
    pub branch: Option<String>,
    /// True while another process holds it, which is what stops a concurrent checkout.
    pub locked: bool,
    /// True when the main worktree of a bare repository, which has no files of its own.
    pub bare: bool,
    /// True for the repository's own working tree, which is the one that cannot be removed.
    ///
    /// git lists it first and marks it no other way. Telling it apart by comparing its path
    /// with the path the repository was opened at does not work: inside a submodule git reports
    /// the gitdir under `.git/modules/…` rather than the checkout, so the submodule listed
    /// itself as a linked tree and offered to remove it.
    pub main: bool,
}

/// Parses `git worktree list --porcelain`.
///
/// Records are separated by a blank line and each attribute is its own line, which is the only
/// listing format that survives a path containing spaces.
#[must_use]
pub fn parse_list(stdout: &[u8]) -> Vec<Worktree> {
    let mut out = Vec::new();
    let mut current: Option<Worktree> = None;

    for line in stdout.split(|b| *b == b'\n') {
        let line = line.trim();
        if line.is_empty() {
            out.extend(current.take());
            continue;
        }
        let (key, value) = match line.iter().position(|b| *b == b' ') {
            Some(at) => (&line[..at], line[at + 1..].to_str_lossy().into_owned()),
            None => (line, String::new()),
        };
        match key {
            b"worktree" => {
                out.extend(current.take());
                // git lists the main working tree first, always.
                let first = out.is_empty();
                current = Some(Worktree {
                    path: value,
                    head: String::new(),
                    branch: None,
                    locked: false,
                    bare: false,
                    main: first,
                });
            }
            b"HEAD" => {
                if let Some(w) = current.as_mut() {
                    w.head = value;
                }
            }
            b"branch" => {
                if let Some(w) = current.as_mut() {
                    w.branch = Some(value.trim_start_matches("refs/heads/").to_owned());
                }
            }
            b"locked" => {
                if let Some(w) = current.as_mut() {
                    w.locked = true;
                }
            }
            b"bare" => {
                if let Some(w) = current.as_mut() {
                    w.bare = true;
                }
            }
            // `detached` and `prunable` carry no field this needs; a detached worktree is
            // already the one with no branch.
            _ => {}
        }
    }
    out.extend(current);
    out
}

impl RepoLocation {
    /// Lists the repository's working trees.
    ///
    /// # Errors
    /// Propagates git failures.
    pub async fn worktrees(&self, runner: &GitRunner) -> Result<Vec<Worktree>, CoralError> {
        let out = runner
            .output(GitCommand::read("worktree", self.display_path()).args([
                "worktree",
                "list",
                "--porcelain",
            ]))
            .await?;
        Ok(parse_list(&out.stdout))
    }

    /// Checks a revision out into a new working tree.
    ///
    /// With a branch name it creates that branch there; without one the new tree is detached,
    /// because two working trees of the same repository may not have the same branch checked
    /// out and silently borrowing an existing one would move it under the user.
    ///
    /// # Errors
    /// Propagates git failures: a path that already exists, or a branch already checked out
    /// somewhere else.
    pub async fn worktree_add(
        &self,
        runner: &GitRunner,
        path: &Path,
        rev: &str,
        branch: Option<&str>,
    ) -> Result<PathBuf, CoralError> {
        let mut cmd =
            GitCommand::write("worktree", self.display_path()).args(["worktree", "add", "--quiet"]);
        match branch {
            Some(name) => cmd = cmd.args(["-b", name]),
            None => cmd = cmd.arg("--detach"),
        }
        cmd = cmd.arg(path).arg(rev);
        runner.output(cmd).await?;
        Ok(path.to_path_buf())
    }

    /// Removes a working tree, leaving the repository alone.
    ///
    /// # Errors
    /// Propagates git failures, including uncommitted changes without `force`.
    pub async fn worktree_remove(
        &self,
        runner: &GitRunner,
        path: &Path,
        force: bool,
    ) -> Result<(), CoralError> {
        let mut cmd =
            GitCommand::write("worktree", self.display_path()).args(["worktree", "remove"]);
        if force {
            cmd = cmd.arg("--force");
        }
        runner.output(cmd.arg(path)).await.map(|_| ())
    }
}

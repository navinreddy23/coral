use std::path::{Path, PathBuf};

use crate::error::CoralError;
use crate::process::{GitCommand, GitRunner, GitVersion};

/// Where HEAD points.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Head {
    Branch {
        name: String,
    },
    Detached {
        oid: String,
    },
    /// A branch that exists symbolically but has no commit yet — a fresh `git init`.
    Unborn {
        name: String,
    },
}

/// The multi-step operation the repository is in the middle of, detected from the files git
/// leaves in the git dir rather than from porcelain.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "snake_case")]
pub enum OpState {
    Clean,
    Merge,
    Rebase,
    CherryPick,
    Revert,
    Bisect,
}

/// The paths that identify a repository. Cheap to clone and carried by every engine task.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepoLocation {
    /// Worktree root. `None` for a bare repository.
    pub workdir: Option<PathBuf>,
    /// This worktree's git dir — `.git/worktrees/<name>` for a linked worktree.
    pub git_dir: PathBuf,
    /// The shared git dir. Differs from `git_dir` only in linked worktrees; refs and objects
    /// live here.
    pub common_dir: PathBuf,
    pub is_bare: bool,
}

/// What `coral open` reports.
#[derive(Clone, Debug, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub struct RepoInfo {
    pub path: PathBuf,
    pub git_dir: PathBuf,
    pub common_dir: PathBuf,
    pub is_bare: bool,
    #[cfg_attr(feature = "ts", ts(type = "string"))]
    pub git_version: GitVersion,
    pub head: Head,
    pub state: OpState,
    /// Whether a commit-graph is present. Its absence is the single biggest predictor of a
    /// slow first paint on a large repository.
    pub commit_graph: bool,
}

impl RepoLocation {
    /// Resolves `path` to a repository.
    ///
    /// # Errors
    /// [`CoralError::NotARepository`] if `path` is not inside a git repository.
    pub async fn discover(runner: &GitRunner, path: &Path) -> Result<Self, CoralError> {
        let out = runner
            .output(
                GitCommand::read("rev-parse", path)
                    .args(["rev-parse", "--path-format=absolute"])
                    .args(["--git-dir", "--git-common-dir", "--is-bare-repository"]),
            )
            .await
            .map_err(|e| match e {
                // git exits 128 with "not a git repository"; anything else is a real failure.
                CoralError::GitExit { code: 128, .. } => {
                    CoralError::NotARepository(path.to_path_buf())
                }
                other => other,
            })?;

        let text = String::from_utf8_lossy(&out.stdout);
        let mut lines = text.lines();
        let git_dir = PathBuf::from(next_line(&mut lines, "git-dir")?);
        let common_dir = PathBuf::from(next_line(&mut lines, "git-common-dir")?);
        let is_bare = next_line(&mut lines, "is-bare-repository")? == "true";

        let workdir = if is_bare {
            None
        } else {
            Some(Self::toplevel(runner, path).await?)
        };
        Ok(Self {
            workdir,
            git_dir,
            common_dir,
            is_bare,
        })
    }

    async fn toplevel(runner: &GitRunner, path: &Path) -> Result<PathBuf, CoralError> {
        let out = runner
            .output(GitCommand::read("rev-parse", path).args([
                "rev-parse",
                "--path-format=absolute",
                "--show-toplevel",
            ]))
            .await?;
        let text = String::from_utf8_lossy(&out.stdout);
        Ok(PathBuf::from(text.trim_end_matches(['\n', '\r'])))
    }

    /// The directory the UI shows for this repository.
    #[must_use]
    pub fn display_path(&self) -> &Path {
        self.workdir.as_deref().unwrap_or(&self.git_dir)
    }

    /// Resolves a path inside this worktree's git dir.
    #[must_use]
    pub fn git_path(&self, name: &str) -> PathBuf {
        self.git_dir.join(name)
    }

    /// True when the shared object store carries a commit-graph.
    #[must_use]
    pub fn has_commit_graph(&self) -> bool {
        let info = self.common_dir.join("objects").join("info");
        info.join("commit-graph").exists() || info.join("commit-graphs").is_dir()
    }

    /// Reads the in-progress operation from the git dir. Faster and more reliable than
    /// porcelain, which only tells you that files are unmerged.
    #[must_use]
    pub fn op_state(&self) -> OpState {
        // rebase-merge covers interactive and merge-backend rebases; rebase-apply covers the
        // am-backend and `git am` itself.
        if self.git_path("rebase-merge").is_dir() || self.git_path("rebase-apply").is_dir() {
            OpState::Rebase
        } else if self.git_path("MERGE_HEAD").exists() {
            OpState::Merge
        } else if self.git_path("CHERRY_PICK_HEAD").exists() {
            OpState::CherryPick
        } else if self.git_path("REVERT_HEAD").exists() {
            OpState::Revert
        } else if self.git_path("BISECT_LOG").exists() {
            OpState::Bisect
        } else {
            OpState::Clean
        }
    }

    /// Reads HEAD, distinguishing an unborn branch from a detached one.
    ///
    /// # Errors
    /// Propagates unexpected git failures.
    pub async fn head(&self, runner: &GitRunner) -> Result<Head, CoralError> {
        let dir = self.display_path();
        let symbolic = runner
            .output(GitCommand::read("symbolic-ref", dir).args([
                "symbolic-ref",
                "--quiet",
                "--short",
                "HEAD",
            ]))
            .await;

        match symbolic {
            Ok(out) => {
                let name = String::from_utf8_lossy(&out.stdout).trim().to_owned();
                if self.resolve(runner, "HEAD").await?.is_some() {
                    Ok(Head::Branch { name })
                } else {
                    Ok(Head::Unborn { name })
                }
            }
            // Exit 1 from symbolic-ref means HEAD is not symbolic, i.e. detached.
            Err(CoralError::GitExit { code: 1, .. }) => {
                let oid =
                    self.resolve(runner, "HEAD")
                        .await?
                        .ok_or_else(|| CoralError::Protocol {
                            label: "symbolic-ref",
                            detail: "HEAD is neither symbolic nor resolvable".to_owned(),
                        })?;
                Ok(Head::Detached { oid })
            }
            Err(other) => Err(other),
        }
    }

    /// Resolves a revision to a full oid, or `None` when it does not exist.
    async fn resolve(&self, runner: &GitRunner, rev: &str) -> Result<Option<String>, CoralError> {
        let out = runner
            .output(GitCommand::read("rev-parse", self.display_path()).args([
                "rev-parse",
                "--verify",
                "--quiet",
                &format!("{rev}^{{commit}}"),
            ]))
            .await;
        match out {
            Ok(o) => Ok(Some(String::from_utf8_lossy(&o.stdout).trim().to_owned())),
            // `--quiet` turns "no such revision" into a silent exit 1.
            Err(CoralError::GitExit { code: 1, .. }) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Reads the working tree state.
    ///
    /// # Errors
    /// Propagates git failures and [`CoralError::Protocol`] if the output does not parse.
    pub async fn status(&self, runner: &GitRunner) -> Result<crate::status::Status, CoralError> {
        let out = runner
            .output(GitCommand::status("status", self.display_path()).args([
                "status",
                "--porcelain=v2",
                "-z",
                "--branch",
                "--show-stash",
            ]))
            .await?;
        crate::status::Status::parse(&out.stdout)
    }

    /// Lists every ref, with upstream tracking for local branches.
    ///
    /// # Errors
    /// Propagates git failures and [`CoralError::Protocol`] if the output does not parse.
    pub async fn refs(&self, runner: &GitRunner) -> Result<Vec<crate::refs::GitRef>, CoralError> {
        let out = runner
            .output(
                GitCommand::read("for-each-ref", self.display_path())
                    .arg("for-each-ref")
                    .arg(format!("--format={}", crate::refs::FORMAT)),
            )
            .await?;
        crate::refs::parse(&out.stdout)
    }

    /// Gathers everything `coral open` reports.
    ///
    /// # Errors
    /// Propagates git failures from the HEAD lookup.
    pub async fn info(&self, runner: &GitRunner) -> Result<RepoInfo, CoralError> {
        Ok(RepoInfo {
            path: self.display_path().to_path_buf(),
            git_dir: self.git_dir.clone(),
            common_dir: self.common_dir.clone(),
            is_bare: self.is_bare,
            git_version: runner.version(),
            head: self.head(runner).await?,
            state: self.op_state(),
            commit_graph: self.has_commit_graph(),
        })
    }
}

fn next_line<'a>(
    lines: &mut std::str::Lines<'a>,
    field: &'static str,
) -> Result<&'a str, CoralError> {
    lines.next().ok_or(CoralError::Protocol {
        label: "rev-parse",
        detail: format!("missing {field} in output"),
    })
}

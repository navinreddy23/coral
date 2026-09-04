//! Mutating operations. Every git command string lives here rather than being assembled by
//! callers, so the arguments for a given operation exist in exactly one place.

use crate::error::CoralError;
use crate::process::{GitCommand, GitRunner};
use crate::repo::RepoLocation;

/// How a commit is created.
#[derive(Clone, Debug, Default)]
pub struct CommitOpts {
    pub message: String,
    /// Replace the previous commit rather than adding one.
    pub amend: bool,
    /// Append a `Signed-off-by` trailer.
    pub signoff: bool,
    /// Override the author, as `Name <email>`.
    pub author: Option<String>,
    /// Commit even with nothing staged, which a merge or an empty marker commit needs.
    pub allow_empty: bool,
}

/// Where a reset leaves the index and worktree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResetMode {
    /// Move the branch only.
    Soft,
    /// Move the branch and reset the index.
    Mixed,
    /// Move the branch and discard index and worktree changes.
    Hard,
}

impl ResetMode {
    const fn flag(self) -> &'static str {
        match self {
            Self::Soft => "--soft",
            Self::Mixed => "--mixed",
            Self::Hard => "--hard",
        }
    }
}

/// How a merge should be recorded.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum MergeMode {
    /// Fast-forward when possible, otherwise create a merge commit.
    #[default]
    Auto,
    /// Always create a merge commit.
    NoFf,
    /// Refuse unless it can fast-forward.
    FfOnly,
    /// Apply the changes but do not record a merge.
    Squash,
}

impl RepoLocation {
    /// Records a commit.
    ///
    /// # Errors
    /// Propagates git failures; a failing hook surfaces with its own output in the error.
    pub async fn commit(&self, runner: &GitRunner, o: &CommitOpts) -> Result<String, CoralError> {
        let mut cmd = GitCommand::write("commit", self.display_path())
            .args(["commit", "--quiet", "-m", &o.message]);
        if o.amend {
            cmd = cmd.arg("--amend");
        }
        if o.signoff {
            cmd = cmd.arg("--signoff");
        }
        if o.allow_empty {
            cmd = cmd.arg("--allow-empty");
        }
        if let Some(a) = &o.author {
            cmd = cmd.arg(format!("--author={a}"));
        }
        runner.output(cmd).await?;
        self.rev_parse(runner, "HEAD").await
    }

    /// Resolves a revision to a full object id.
    ///
    /// # Errors
    /// Propagates git failures.
    pub async fn rev_parse(&self, runner: &GitRunner, rev: &str) -> Result<String, CoralError> {
        let out = runner
            .output(
                GitCommand::read("rev-parse", self.display_path())
                    .args(["rev-parse", "--verify"])
                    .arg(rev),
            )
            .await?;
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
    }

    /// Creates a branch, optionally checking it out.
    ///
    /// # Errors
    /// Propagates git failures, including an existing branch of the same name.
    pub async fn branch_create(
        &self,
        runner: &GitRunner,
        name: &str,
        at: Option<&str>,
        checkout: bool,
    ) -> Result<(), CoralError> {
        let mut cmd = if checkout {
            GitCommand::write("checkout", self.display_path()).args(["checkout", "-b", name])
        } else {
            GitCommand::write("branch", self.display_path()).args(["branch", name])
        };
        if let Some(rev) = at {
            cmd = cmd.arg(rev);
        }
        runner.output(cmd).await.map(|_| ())
    }

    /// Deletes a branch. Without `force`, git refuses if it is not merged.
    ///
    /// # Errors
    /// Propagates git failures.
    pub async fn branch_delete(
        &self,
        runner: &GitRunner,
        name: &str,
        force: bool,
    ) -> Result<(), CoralError> {
        let flag = if force { "-D" } else { "-d" };
        runner
            .output(GitCommand::write("branch", self.display_path()).args(["branch", flag, name]))
            .await
            .map(|_| ())
    }

    /// Renames a branch.
    ///
    /// # Errors
    /// Propagates git failures.
    pub async fn branch_rename(
        &self,
        runner: &GitRunner,
        from: &str,
        to: &str,
    ) -> Result<(), CoralError> {
        runner
            .output(
                GitCommand::write("branch", self.display_path()).args(["branch", "-m", from, to]),
            )
            .await
            .map(|_| ())
    }

    /// Points a local branch at an upstream.
    ///
    /// # Errors
    /// Propagates git failures.
    pub async fn set_upstream(
        &self,
        runner: &GitRunner,
        branch: &str,
        upstream: &str,
    ) -> Result<(), CoralError> {
        runner
            .output(GitCommand::write("branch", self.display_path()).args([
                "branch",
                &format!("--set-upstream-to={upstream}"),
                branch,
            ]))
            .await
            .map(|_| ())
    }

    /// Checks out a revision.
    ///
    /// # Errors
    /// Propagates git failures, including a dirty worktree that would be overwritten.
    pub async fn checkout(&self, runner: &GitRunner, rev: &str) -> Result<(), CoralError> {
        runner
            .output(
                GitCommand::write("checkout", self.display_path())
                    .args(["checkout", "--quiet"])
                    .arg(rev),
            )
            .await
            .map(|_| ())
    }

    /// Creates a tag. A message makes it annotated.
    ///
    /// # Errors
    /// Propagates git failures.
    pub async fn tag_create(
        &self,
        runner: &GitRunner,
        name: &str,
        at: Option<&str>,
        message: Option<&str>,
    ) -> Result<(), CoralError> {
        let mut cmd = GitCommand::write("tag", self.display_path()).arg("tag");
        if let Some(m) = message {
            cmd = cmd.args(["-a", "-m", m]);
        }
        cmd = cmd.arg(name);
        if let Some(rev) = at {
            cmd = cmd.arg(rev);
        }
        runner.output(cmd).await.map(|_| ())
    }

    /// Deletes a tag.
    ///
    /// # Errors
    /// Propagates git failures.
    pub async fn tag_delete(&self, runner: &GitRunner, name: &str) -> Result<(), CoralError> {
        runner
            .output(GitCommand::write("tag", self.display_path()).args(["tag", "-d", name]))
            .await
            .map(|_| ())
    }

    /// Stashes the worktree.
    ///
    /// # Errors
    /// Propagates git failures.
    pub async fn stash_push(
        &self,
        runner: &GitRunner,
        message: Option<&str>,
        include_untracked: bool,
    ) -> Result<(), CoralError> {
        let mut cmd =
            GitCommand::write("stash", self.display_path()).args(["stash", "push", "--quiet"]);
        if include_untracked {
            cmd = cmd.arg("--include-untracked");
        }
        if let Some(m) = message {
            cmd = cmd.args(["-m", m]);
        }
        runner.output(cmd).await.map(|_| ())
    }

    /// Applies a stash entry, optionally dropping it.
    ///
    /// # Errors
    /// Propagates git failures, including conflicts raised by the application.
    pub async fn stash_apply(
        &self,
        runner: &GitRunner,
        index: usize,
        pop: bool,
    ) -> Result<(), CoralError> {
        let verb = if pop { "pop" } else { "apply" };
        runner
            .output(GitCommand::write("stash", self.display_path()).args([
                "stash",
                verb,
                "--quiet",
                &format!("stash@{{{index}}}"),
            ]))
            .await
            .map(|_| ())
    }

    /// Drops a stash entry.
    ///
    /// # Errors
    /// Propagates git failures.
    pub async fn stash_drop(&self, runner: &GitRunner, index: usize) -> Result<(), CoralError> {
        runner
            .output(GitCommand::write("stash", self.display_path()).args([
                "stash",
                "drop",
                "--quiet",
                &format!("stash@{{{index}}}"),
            ]))
            .await
            .map(|_| ())
    }

    /// Moves the current branch, and optionally the index and worktree.
    ///
    /// # Errors
    /// Propagates git failures.
    pub async fn reset(
        &self,
        runner: &GitRunner,
        rev: &str,
        mode: ResetMode,
    ) -> Result<(), CoralError> {
        runner
            .output(
                GitCommand::write("reset", self.display_path())
                    .args(["reset", "--quiet", mode.flag()])
                    .arg(rev),
            )
            .await
            .map(|_| ())
    }
}

/// How a stopped operation should proceed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpAction {
    Continue,
    Abort,
    Skip,
}

/// The result of an operation that may stop for conflicts.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub struct OpOutcome {
    /// True when the operation finished; false when it stopped and needs resolution.
    pub completed: bool,
    pub state: crate::repo::OpState,
    /// Paths left unmerged, when it stopped.
    pub conflicts: Vec<String>,
    /// git's own explanation, when it stopped.
    pub message: String,
}

impl RepoLocation {
    /// Builds the outcome of an operation that may have stopped, by asking the repository
    /// rather than by interpreting git's exit code.
    ///
    /// # Errors
    /// Propagates git failures from the status read.
    pub async fn op_outcome(
        &self,
        runner: &GitRunner,
        message: String,
    ) -> Result<OpOutcome, CoralError> {
        let status = self.status(runner).await?;
        let conflicts: Vec<String> = status.conflicted().map(|e| e.path.to_string()).collect();
        let state = self.op_state();
        Ok(OpOutcome {
            completed: conflicts.is_empty() && state == crate::repo::OpState::Clean,
            state,
            conflicts,
            message,
        })
    }

    /// Merges `rev` into the current branch.
    ///
    /// A merge that stops on conflicts is not an error: it is an outcome the user acts on. The
    /// distinction is made from the repository's own state, not from the exit code, because
    /// git exits non-zero for both a conflict and a genuine failure.
    ///
    /// # Errors
    /// Propagates git failures that left no conflict behind.
    pub async fn merge(
        &self,
        runner: &GitRunner,
        rev: &str,
        mode: MergeMode,
        message: Option<&str>,
    ) -> Result<OpOutcome, CoralError> {
        let mut cmd = GitCommand::write("merge", self.display_path()).arg("merge");
        match mode {
            MergeMode::Auto => {}
            MergeMode::NoFf => cmd = cmd.arg("--no-ff"),
            MergeMode::FfOnly => cmd = cmd.arg("--ff-only"),
            MergeMode::Squash => cmd = cmd.arg("--squash"),
        }
        if let Some(m) = message {
            cmd = cmd.args(["-m", m]);
        }
        // Without this git opens an editor for the merge message and hangs.
        cmd = cmd.arg("--no-edit").arg(rev);
        self.run_stoppable(runner, cmd).await
    }

    /// Replays the current branch onto `onto`.
    ///
    /// # Errors
    /// Propagates git failures that left no conflict behind.
    pub async fn rebase(
        &self,
        runner: &GitRunner,
        onto: &str,
        update_refs: bool,
    ) -> Result<OpOutcome, CoralError> {
        let mut cmd = GitCommand::write("rebase", self.display_path()).arg("rebase");
        if update_refs {
            cmd = cmd.arg("--update-refs");
        }
        self.run_stoppable(runner, cmd.arg(onto)).await
    }

    /// Applies commits onto the current branch.
    ///
    /// # Errors
    /// Propagates git failures that left no conflict behind.
    pub async fn cherry_pick(
        &self,
        runner: &GitRunner,
        revs: &[&str],
        commit: bool,
    ) -> Result<OpOutcome, CoralError> {
        let mut cmd = GitCommand::write("cherry-pick", self.display_path())
            .args(["cherry-pick", "--no-edit"]);
        if !commit {
            // The changes land in the index and the working tree and stop there, so they can be
            // amended, split, or added to something else before anything is recorded. git still
            // treats this as a cherry-pick in progress, which is what lets a conflict be
            // resolved and continued the same way.
            cmd = cmd.arg("--no-commit");
        }
        self.run_stoppable(runner, cmd.args(revs)).await
    }

    /// Records commits that undo `revs`.
    ///
    /// # Errors
    /// Propagates git failures that left no conflict behind.
    pub async fn revert(&self, runner: &GitRunner, revs: &[&str]) -> Result<OpOutcome, CoralError> {
        let cmd = GitCommand::write("revert", self.display_path())
            .args(["revert", "--no-edit"])
            .args(revs);
        self.run_stoppable(runner, cmd).await
    }

    /// Continues, aborts, or skips whatever operation is in progress.
    ///
    /// # Errors
    /// [`CoralError::Protocol`] when nothing is in progress.
    pub async fn op(&self, runner: &GitRunner, action: OpAction) -> Result<OpOutcome, CoralError> {
        use crate::repo::OpState;

        let verb = match action {
            OpAction::Continue => "--continue",
            OpAction::Abort => "--abort",
            OpAction::Skip => "--skip",
        };
        let subcommand = match self.op_state() {
            OpState::Merge => "merge",
            OpState::Rebase => "rebase",
            OpState::CherryPick => "cherry-pick",
            OpState::Revert => "revert",
            OpState::Bisect | OpState::Clean => {
                return Err(CoralError::Refused {
                    label: "continue",
                    detail: "no merge, rebase, cherry-pick or revert is in progress".to_owned(),
                });
            }
        };

        // `--continue` must not open an editor for the message git has already written. The
        // runner sets GIT_EDITOR=":" on every command, so it accepts the existing message.
        let cmd = GitCommand::write("op", self.display_path()).args([subcommand, verb]);
        self.run_stoppable(runner, cmd).await
    }

    /// Runs a command that may legitimately stop for conflicts.
    async fn run_stoppable(
        &self,
        runner: &GitRunner,
        cmd: GitCommand,
    ) -> Result<OpOutcome, CoralError> {
        match runner.output(cmd).await {
            Ok(out) => {
                let msg = String::from_utf8_lossy(&out.stdout).trim().to_owned();
                self.op_outcome(runner, msg).await
            }
            Err(e) => {
                // Conflicts and a real failure both exit non-zero. Only the repository's own
                // state tells them apart.
                let outcome = self
                    .op_outcome(runner, e.stderr().unwrap_or_default().to_owned())
                    .await?;
                if outcome.completed {
                    Err(e)
                } else {
                    Ok(outcome)
                }
            }
        }
    }
}

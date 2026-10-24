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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
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

/// Which changes a stash takes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StashScope {
    /// Every change to a tracked file, and new files too when asked.
    Everything { include_untracked: bool },
    /// These files only, new or not, whatever else has changed.
    Paths(Vec<String>),
    /// What is staged, leaving unstaged changes and new files behind. `git stash --staged`,
    /// which needs git 2.35.
    Staged,
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

    /// Creates a tag. A message makes it annotated, and `tag.gpgSign` then signs it.
    ///
    /// Without one it is lightweight, which is a ref and nothing else: there is no object for
    /// a signature to sit on, so `--no-sign` says so. `tag.gpgSign` otherwise turns every tag
    /// into a signed one needing a message, and the window, which pins the editor to a no-op
    /// so nothing can hang waiting for one, got "fatal: no tag message?" from a menu entry
    /// that had just offered to make a tag without a message.
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
        match message {
            Some(m) => cmd = cmd.args(["-a", "-m", m]),
            None => cmd = cmd.arg("--no-sign"),
        }
        cmd = cmd.arg(name);
        if let Some(rev) = at {
            cmd = cmd.arg(rev);
        }
        runner.output(cmd).await.map(|_| ())
    }

    /// Moves an existing tag to another commit.
    ///
    /// Separate from [`RepoLocation::tag_create`] because it is a different act: creating a
    /// tag fails if one of that name exists, and this replaces it. Anyone who has already
    /// fetched the old one keeps it, which is why the window asks first.
    ///
    /// # Errors
    /// Propagates git failures, including an unknown revision.
    pub async fn tag_move(
        &self,
        runner: &GitRunner,
        name: &str,
        at: &str,
    ) -> Result<(), CoralError> {
        runner
            .output(GitCommand::write("tag", self.display_path()).args(["tag", "-f", name, at]))
            .await
            .map(|_| ())
    }

    /// Moves a branch that is not checked out up to `at`, refusing anything but a
    /// fast-forward.
    ///
    /// `fetch .` rather than `branch -f`: fetching a ref without a leading `+` is how git is
    /// asked to move it only if nothing would be lost, and it says so itself when the answer
    /// is no. `branch -f` would move it either way.
    ///
    /// # Errors
    /// Propagates git failures, including a move that is not a fast-forward.
    pub async fn branch_fast_forward(
        &self,
        runner: &GitRunner,
        name: &str,
        at: &str,
    ) -> Result<(), CoralError> {
        runner
            .output(GitCommand::write("fetch", self.display_path()).args([
                "fetch",
                ".",
                &format!("{at}:refs/heads/{name}"),
            ]))
            .await
            .map(|_| ())
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

    /// Stashes the changes `scope` names, and leaves the rest where they are.
    ///
    /// # Errors
    /// Propagates git failures, including git's own refusal when there is nothing in scope
    /// to stash, and [`CoralError::Refused`] before the first commit.
    pub async fn stash_push(
        &self,
        runner: &GitRunner,
        message: Option<&str>,
        scope: &StashScope,
    ) -> Result<(), CoralError> {
        // Asked here rather than left to git: `--quiet` silences the one line git gives for it,
        // "You do not have the initial commit yet", and the window said only that git had
        // exited with 1.
        if matches!(self.head(runner).await?, crate::repo::Head::Unborn { .. }) {
            return Err(CoralError::Refused {
                label: "stash",
                detail: "there is no commit yet to stash against. Make the first commit, then \
                         stash"
                    .to_owned(),
            });
        }
        let mut cmd =
            GitCommand::write("stash", self.display_path()).args(["stash", "push", "--quiet"]);
        if let Some(m) = message {
            cmd = cmd.args(["-m", m]);
        }
        match scope {
            StashScope::Everything { include_untracked } => {
                if *include_untracked {
                    cmd = cmd.arg("--include-untracked");
                }
            }
            // With -u an untracked file named here goes too, and one not named stays; without
            // it, picking a new file would stash nothing and say nothing about why.
            StashScope::Paths(paths) => {
                cmd = cmd
                    .args(["--include-untracked", "--"])
                    .args(paths.iter().map(|p| format!(":(literal){p}")));
            }
            StashScope::Staged => cmd = cmd.arg("--staged"),
        }
        runner.output(cmd).await.map(|_| ())
    }

    /// Applies a stash entry, optionally dropping it.
    ///
    /// A stash that lands on conflicts has not failed, any more than a merge that does: the
    /// worktree holds both sides and git keeps the entry. Reported as an outcome for the same
    /// reason, and told apart from a real failure the same way — by asking the repository
    /// rather than by reading the exit code, which is 1 for both.
    ///
    /// `--quiet` is kept: git still prints the conflicting paths, and without it a successful
    /// pop answers with a whole `git status` nobody asked for.
    ///
    /// # Errors
    /// Propagates git failures that left no conflict behind.
    pub async fn stash_apply(
        &self,
        runner: &GitRunner,
        index: usize,
        pop: bool,
    ) -> Result<OpOutcome, CoralError> {
        let verb = if pop { "pop" } else { "apply" };
        self.run_stoppable(
            runner,
            GitCommand::write("stash", self.display_path()).args([
                "stash",
                verb,
                "--quiet",
                &format!("stash@{{{index}}}"),
            ]),
        )
        .await
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

/// Adds `-m <n>` when a mainline was named.
///
/// A merge has two sides and undoing or replaying it means keeping one of them, so git refuses
/// both `revert` and `cherry-pick` on a merge unless told which — and the refusal, "is a merge
/// but no -m option was given", is a sentence about git's command line rather than about the
/// repository. Naming the side is the caller's decision; passing it on is this.
fn with_mainline(cmd: GitCommand, mainline: Option<u32>) -> GitCommand {
    match mainline {
        Some(n) => cmd.args(["-m", &n.to_string()]),
        None => cmd,
    }
}

/// git's output as a terminal would have shown it.
///
/// Progress is redrawn with carriage returns rather than newlines: rebase writes
/// "Rebasing (1/2)\rerror: could not apply …", which a terminal shows as the error alone and a
/// window shows as "Rebasing (1/2)error: could not apply …" — one run-on line that reads like a
/// fault in the message rather than in the rebase. Only what survives the last redraw of each
/// line is kept.
fn as_shown(text: &str) -> String {
    text.lines()
        .map(crate::process::last_record)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_owned()
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
        mainline: Option<u32>,
    ) -> Result<OpOutcome, CoralError> {
        let mut cmd = GitCommand::write("cherry-pick", self.display_path())
            .args(["cherry-pick", "--no-edit"]);
        cmd = with_mainline(cmd, mainline);
        if !commit {
            // The changes land in the index and the working tree and stop there, so they can be
            // amended, split, or added to something else before anything is recorded. git still
            // treats this as a cherry-pick in progress, which is what lets a conflict be
            // resolved and continued the same way.
            cmd = cmd.arg("--no-commit");
        }
        let out = self.run_stoppable(runner, cmd.args(revs)).await?;
        if !commit && revs.len() > 1 && out.completed {
            // git leaves only the last commit's message in MERGE_MSG, which is where the
            // commit box starts; folded together, the changes are every commit's.
            if let Some(text) = self.messages_of(runner, revs).await {
                self.settle_message(&text)?;
            }
        }
        Ok(out)
    }

    /// Each commit's whole message, in the order given, a blank line between them.
    async fn messages_of(&self, runner: &GitRunner, revs: &[&str]) -> Option<String> {
        let out = runner
            .output(
                GitCommand::read("log", self.display_path())
                    .args(["log", "--no-walk=unsorted", "--format=%B%x00"])
                    .args(revs),
            )
            .await
            .ok()?;
        let text = String::from_utf8_lossy(&out.stdout);
        let messages: Vec<&str> = text
            .split('\0')
            .map(str::trim)
            .filter(|m| !m.is_empty())
            .collect();
        (!messages.is_empty()).then(|| messages.join("\n\n"))
    }

    /// Records commits that undo `revs`.
    ///
    /// `mainline` names which parent's line of development to keep, counting from one. git
    /// requires one for a merge commit and refuses a number the commit has no parent for.
    ///
    /// # Errors
    /// Propagates git failures that left no conflict behind.
    pub async fn revert(
        &self,
        runner: &GitRunner,
        revs: &[&str],
        mainline: Option<u32>,
    ) -> Result<OpOutcome, CoralError> {
        let cmd = GitCommand::write("revert", self.display_path()).args(["revert", "--no-edit"]);
        let cmd = with_mainline(cmd, mainline).args(revs);
        self.run_stoppable(runner, cmd).await
    }

    /// Makes `text` the message the stopped operation commits with when it continues.
    ///
    /// For a message edited in the window while a merge, cherry-pick, revert or rebase waits.
    /// Each reads its own file when it continues, and none reads what is in the commit box: a
    /// rebase reads `rebase-merge/message`, the rest `MERGE_MSG` (see `git help rebase`,
    /// "--continue"). Both are written when a rebase is stopped, since a squash also leaves
    /// its combined message in `MERGE_MSG`.
    ///
    /// # Errors
    /// I/O failures writing either file.
    pub fn settle_message(&self, text: &str) -> Result<(), CoralError> {
        let mut body = text.trim_end().to_owned();
        body.push('\n');
        std::fs::write(self.git_dir.join("MERGE_MSG"), &body)?;
        let rebase = self.git_dir.join("rebase-merge");
        if rebase.is_dir() {
            std::fs::write(rebase.join("message"), &body)?;
        }
        Ok(())
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
            // `rebase-apply` is left by `git am` as well, and the two are settled by different
            // subcommands however alike they look on screen.
            OpState::Rebase if self.applying_patches() => "am",
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

        // An interactive rebase can owe a new message to the commit it is stopped on. Written
        // in before continuing, because continuing is what records that commit: a reword whose
        // commit also conflicts hands the rebase to the merge tool, and what continues it from
        // there is this. Held only inside the call that started the rebase, the message
        // somebody typed into the picker was dropped and the commit kept its old one.
        if matches!(action, OpAction::Continue) && subcommand == "rebase" {
            self.settle_reword(runner).await?;
        }

        // `--continue` must not open an editor for the message git has already written. The
        // runner sets GIT_EDITOR=":" on every command, so it accepts the existing message.
        let cmd = GitCommand::write("op", self.display_path()).args([subcommand, verb]);
        let outcome = self.run_stoppable(runner, cmd).await?;

        // An interactive rebase can be carrying rewords that have not been applied yet: a
        // commit set to reword is replayed as an `edit`, and the new message is written on when
        // the rebase stops on it. A conflict on that very commit hands the rebase to the merge
        // tool, and what continues it from there is this — so without this the message somebody
        // typed into the picker was silently dropped and the commit kept its old one.
        if matches!(action, OpAction::Abort) {
            self.forget_rewords();
            return Ok(outcome);
        }
        if subcommand != "rebase" {
            return Ok(outcome);
        }
        if outcome.completed {
            self.forget_rewords();
        }
        Ok(outcome)
    }

    /// Runs a command that may legitimately stop for conflicts.
    /// The same, reporting git's progress as it goes.
    ///
    /// Separate from [`Self::run_stoppable`] because streaming has to hold stdout itself: the
    /// buffered form gets it back at the end, and an operation that reads it for the message
    /// still needs one.
    pub(crate) async fn run_stoppable_watching<F>(
        &self,
        runner: &GitRunner,
        cmd: GitCommand,
        mut on_progress: F,
    ) -> Result<OpOutcome, CoralError>
    where
        F: FnMut(&crate::remote::Progress),
    {
        let mut said: Vec<String> = Vec::new();
        let streamed = runner
            .stream_both(
                cmd,
                |line| {
                    said.push(String::from_utf8_lossy(line).into_owned());
                    Ok(crate::process::Sink::Continue)
                },
                |line| {
                    if let Some(p) = crate::remote::parse_progress(line) {
                        on_progress(&p);
                    }
                    Ok(crate::process::Sink::Continue)
                },
            )
            .await;
        match streamed {
            Ok(()) => self.op_outcome(runner, as_shown(&said.join("\n"))).await,
            Err(e) => self.stopped_or_failed(runner, e).await,
        }
    }

    pub(crate) async fn run_stoppable(
        &self,
        runner: &GitRunner,
        cmd: GitCommand,
    ) -> Result<OpOutcome, CoralError> {
        match runner.output(cmd).await {
            Ok(out) => {
                let msg = as_shown(&String::from_utf8_lossy(&out.stdout));
                self.op_outcome(runner, msg).await
            }
            Err(e) => self.stopped_or_failed(runner, e).await,
        }
    }

    /// Conflicts and a real failure both exit non-zero. Only the repository's own state tells
    /// them apart, which is why the exit code is never read for this.
    async fn stopped_or_failed(
        &self,
        runner: &GitRunner,
        e: CoralError,
    ) -> Result<OpOutcome, CoralError> {
        let outcome = self
            .op_outcome(runner, as_shown(e.stderr().unwrap_or_default()))
            .await?;
        if outcome.completed {
            Err(e)
        } else {
            Ok(outcome)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::as_shown;

    #[test]
    fn keeps_only_what_the_last_redraw_left() {
        // What git actually writes while a rebase stops. Kept whole, the window showed
        // "Rebasing (1/2)error: could not apply …" as one line.
        let raw = "Rebasing (1/2)\rerror: could not apply b210f45… feature: rewrite\nhint: Resolve all conflicts manually\n";
        assert_eq!(
            as_shown(raw),
            "error: could not apply b210f45… feature: rewrite\nhint: Resolve all conflicts manually"
        );
    }

    #[test]
    fn leaves_a_message_without_progress_alone() {
        assert_eq!(as_shown("Successfully rebased.\n"), "Successfully rebased.");
        assert_eq!(as_shown("first\r\nsecond\r\n"), "first\nsecond");
    }
}

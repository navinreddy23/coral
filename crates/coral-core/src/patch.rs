//! Patch files, as `git format-patch` writes them.
//!
//! A commit exported as a mailbox-format file, which is what gets attached to a message or fed
//! to `git am` on another machine.

use std::path::{Path, PathBuf};

use bstr::ByteSlice;

use crate::error::CoralError;
use crate::ops::OpOutcome;
use crate::process::{GitCommand, GitRunner};
use crate::repo::RepoLocation;

/// What applying a patch file should leave behind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PatchLanding {
    /// A commit per patch, keeping the author and date the file carries. `git am`.
    Commit,
    /// The changes in the worktree, for review before anything is recorded. `git apply`.
    WorkingTree,
}

impl RepoLocation {
    /// Writes one commit as a patch file into `directory`, returning what was written.
    ///
    /// `--no-numbered` and `--zero-commit` are deliberately not passed: the file name git
    /// chooses carries the commit's own summary, which is what makes a directory of patches
    /// readable, and the numbering is what keeps a series in order.
    ///
    /// # Errors
    /// Propagates git failures, including a directory that cannot be written and a revision
    /// with no parent to diff against.
    pub async fn format_patch(
        &self,
        runner: &GitRunner,
        rev: &str,
        directory: &Path,
    ) -> Result<Vec<PathBuf>, CoralError> {
        self.write_patches(runner, &["-1".to_owned(), rev.to_owned()], directory)
            .await
    }

    /// Writes every commit after `from` up to and including `to`, as a numbered series.
    ///
    /// The range is exclusive at the older end, which is what `from..to` means everywhere else
    /// in git and what somebody picking two commits is asking for: the changes *between* them,
    /// not the older one over again.
    ///
    /// # Errors
    /// Propagates git failures, including a range whose ends are unrelated.
    pub async fn format_patch_range(
        &self,
        runner: &GitRunner,
        from: &str,
        to: &str,
        directory: &Path,
    ) -> Result<Vec<PathBuf>, CoralError> {
        self.write_patches(runner, &[format!("{from}..{to}")], directory)
            .await
    }

    /// How many patch files `from..to` would write.
    ///
    /// Asked before writing, because the answer can be enormous and the window has no way to
    /// take it back: two commits picked far apart on the kernel is nearly a million and a half
    /// files, which fills a disk and hangs the window that asked for it. Cheap enough to ask
    /// every time — `rev-list --count` on that range is milliseconds against the walk itself.
    ///
    /// # Errors
    /// Propagates git failures, including ends that name nothing.
    pub async fn count_range(
        &self,
        runner: &GitRunner,
        from: &str,
        to: &str,
    ) -> Result<u64, CoralError> {
        let out = runner
            .output(
                GitCommand::read("rev-list", self.display_path())
                    .args(["rev-list", "--count"])
                    .arg(format!("{from}..{to}")),
            )
            .await?;
        Ok(String::from_utf8_lossy(&out.stdout)
            .trim()
            .parse()
            .unwrap_or(0))
    }

    async fn write_patches(
        &self,
        runner: &GitRunner,
        revs: &[String],
        directory: &Path,
    ) -> Result<Vec<PathBuf>, CoralError> {
        let out = runner
            .output(
                GitCommand::write("format-patch", self.display_path())
                    .args(["format-patch", "--output-directory"])
                    .arg(directory)
                    .args(revs.iter().map(String::as_str)),
            )
            .await?;

        // One path per line on stdout, which is the only reliable way to learn the name git
        // built from the summary.
        Ok(out
            .stdout
            .split(|b| *b == b'\n')
            .map(<[u8]>::trim)
            .filter(|l| !l.is_empty())
            .map(|l| PathBuf::from(l.to_str_lossy().into_owned()))
            .collect())
    }
}

impl RepoLocation {
    /// Applies patch files to the current branch.
    ///
    /// `--3way` in both landings, and that is the whole reason this is usable: without it a
    /// patch whose context has moved by a line is refused outright, with a message about a
    /// hunk failing rather than anything the reader can act on. With it git falls back to a
    /// real merge against the blobs the patch names, so the ordinary case of a slightly stale
    /// patch lands, and the case that genuinely conflicts stops in the conflict tool like any
    /// other operation instead of failing.
    ///
    /// A stop is an outcome, not an error, exactly as it is for merge and rebase.
    ///
    /// # Errors
    /// Propagates git failures: a file that is not a patch, or one that cannot be read.
    pub async fn apply_patches(
        &self,
        runner: &GitRunner,
        files: &[PathBuf],
        landing: PatchLanding,
    ) -> Result<OpOutcome, CoralError> {
        if files.is_empty() {
            return Err(CoralError::Refused {
                label: "apply-patch",
                detail: "no patch file was named".to_owned(),
            });
        }

        let cmd = match landing {
            // `--empty=drop`: a patch that adds nothing is a patch that has already landed,
            // which is the ordinary result of applying a series twice. git otherwise stops and
            // asks, in the middle of a series, with no way for the window to answer.
            PatchLanding::Commit => GitCommand::write("am", self.display_path())
                .args(["am", "--3way", "--empty=drop"])
                .args(files.iter().map(|f| f.as_os_str())),
            PatchLanding::WorkingTree => GitCommand::write("apply", self.display_path())
                .args(["apply", "--3way"])
                .args(files.iter().map(|f| f.as_os_str())),
        };
        let before = self.landing_print(runner).await;
        let mut outcome = self.run_stoppable(runner, cmd).await?;
        // Both landings exit 0 having done nothing when the patch is already there: `am` says
        // so on a line among others and `apply` says nothing at all. Comparing the repository
        // before and after is the one answer that holds for both.
        if outcome.completed && before.is_some() && before == self.landing_print(runner).await {
            ALREADY_APPLIED.clone_into(&mut outcome.message);
        }
        Ok(outcome)
    }

    /// Where HEAD is and what the worktree holds against it, which is everything applying a
    /// patch can change. `None` when either cannot be read, so an unborn branch is never
    /// reported as unchanged.
    async fn landing_print(&self, runner: &GitRunner) -> Option<(String, Vec<u8>)> {
        let head = self.rev_parse(runner, "HEAD").await.ok()?;
        let diff = runner
            .output(GitCommand::read("diff", self.display_path()).args([
                "diff",
                "HEAD",
                "--binary",
                "--no-ext-diff",
                "--no-color",
            ]))
            .await
            .ok()?;
        Some((head, diff.stdout))
    }
}

/// What an apply that changed nothing reports, worded so the window can recognise it.
pub const ALREADY_APPLIED: &str = "Already applied: nothing changed.";

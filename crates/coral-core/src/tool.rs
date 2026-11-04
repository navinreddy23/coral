//! The diff and merge tools git is configured with, started from the window.
//!
//! `git difftool` and `git mergetool` rather than running the tool directly: git knows how to
//! invoke each of the dozens it supports, reads `diff.guitool` and `merge.guitool` before the
//! plain keys, and for a merge writes the three versions out and stages the result. Coral only
//! decides which file and which revisions.
//! <https://git-scm.com/docs/git-difftool> <https://git-scm.com/docs/git-mergetool>

use crate::config::{AppConfig, Scope};
use crate::error::CoralError;
use crate::process::{GitCommand, GitRunner};
use crate::repo::RepoLocation;

/// The tools git would start, by the names git has for them, or `None` where none is set.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub struct ExternalTools {
    pub diff: Option<String>,
    pub merge: Option<String>,
}

/// What a file is compared across.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Compared {
    /// The working tree against the index.
    Unstaged,
    /// The index against HEAD.
    Staged,
    /// What one commit did: its first parent against it.
    Commit(String),
}

impl RepoLocation {
    /// The diff and merge tools configured here, preferring the graphical ones, as `--gui`
    /// does.
    ///
    /// # Errors
    /// Propagates git failures.
    pub async fn external_tools(&self, runner: &GitRunner) -> Result<ExternalTools, CoralError> {
        let effective = Scope::Effective(AppConfig(None));
        let mut out = ExternalTools::default();
        for (slot, keys) in [
            (&mut out.diff, ["diff.guitool", "diff.tool"]),
            (&mut out.merge, ["merge.guitool", "merge.tool"]),
        ] {
            for key in keys {
                if let Some(name) = self.config_get(runner, effective, key).await? {
                    *slot = Some(name);
                    break;
                }
            }
        }
        Ok(out)
    }

    /// Opens one file's change in the configured diff tool and returns once it has started.
    ///
    /// # Errors
    /// [`CoralError::Refused`] when no diff tool is configured, since git would then fall back
    /// to asking which one on a terminal nobody is at; and a failure to start git.
    pub async fn open_difftool(
        &self,
        runner: &GitRunner,
        file: &str,
        compared: &Compared,
    ) -> Result<(), CoralError> {
        if self.external_tools(runner).await?.diff.is_none() {
            return Err(CoralError::Refused {
                label: "open a diff tool",
                detail: "git has no diff tool set here; set diff.tool, or diff.guitool".to_owned(),
            });
        }
        let mut cmd = GitCommand::read("difftool", self.display_path()).args([
            "difftool",
            "--no-prompt",
            "--gui",
        ]);
        cmd = match compared {
            Compared::Unstaged => cmd,
            Compared::Staged => cmd.arg("--cached"),
            Compared::Commit(rev) => cmd.arg(format!("{rev}^")).arg(rev),
        };
        runner.launch(&cmd.arg("--").arg(file))
    }

    /// Opens a conflicted file in the configured merge tool, which stages it when the tool
    /// says it was resolved.
    ///
    /// # Errors
    /// [`CoralError::Refused`] when no merge tool is configured, and a failure to start git.
    pub async fn open_mergetool(&self, runner: &GitRunner, file: &str) -> Result<(), CoralError> {
        if self.external_tools(runner).await?.merge.is_none() {
            return Err(CoralError::Refused {
                label: "open a merge tool",
                detail: "git has no merge tool set here; set merge.tool, or merge.guitool"
                    .to_owned(),
            });
        }
        let cmd = GitCommand::write("mergetool", self.display_path())
            .args(["mergetool", "--no-prompt", "--gui", "--"])
            .arg(file);
        runner.launch(&cmd)
    }
}

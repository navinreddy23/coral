//! rerere: git remembering how a conflict was resolved and resolving it the same way when the
//! same conflict comes back, as it does on every rebase of a long-lived branch.
//! <https://git-scm.com/docs/git-rerere>

use std::collections::HashSet;

use crate::config::Scope;
use crate::error::CoralError;
use crate::process::{GitCommand, GitRunner};
use crate::repo::RepoLocation;

impl RepoLocation {
    /// Whether git will record resolutions here and reuse them.
    ///
    /// Not only `rerere.enabled`: git treats an unset key as on when `rr-cache` exists, which is
    /// what an earlier `git rerere` left behind.
    ///
    /// # Errors
    /// Propagates git failures.
    pub async fn rerere_enabled(&self, runner: &GitRunner) -> Result<bool, CoralError> {
        let set = self
            .config_get(
                runner,
                Scope::Effective(crate::config::AppConfig(None)),
                "rerere.enabled",
            )
            .await?;
        Ok(match set.as_deref() {
            Some(value) => crate::config::truthy(value),
            None => self.common_dir.join("rr-cache").is_dir(),
        })
    }

    /// Turns recording and reuse on or off for this repository.
    ///
    /// Off is written as `false`, never by unsetting the key: unset, an existing `rr-cache`
    /// keeps rerere running, so "off" would not be.
    ///
    /// # Errors
    /// Propagates git failures.
    pub async fn set_rerere(&self, runner: &GitRunner, on: bool) -> Result<(), CoralError> {
        self.config_put(
            runner,
            Scope::Repository,
            "rerere.enabled",
            Some(if on { "true" } else { "false" }),
        )
        .await
    }

    /// The conflicted paths rerere has not resolved, or `None` when rerere is off and so says
    /// nothing either way.
    pub(crate) async fn rerere_remaining(
        &self,
        runner: &GitRunner,
    ) -> Result<Option<HashSet<String>>, CoralError> {
        if !self.rerere_enabled(runner).await? {
            return Ok(None);
        }
        let out = runner
            .output(GitCommand::read("rerere", self.display_path()).args(["rerere", "remaining"]))
            .await?;
        Ok(Some(
            String::from_utf8_lossy(&out.stdout)
                .lines()
                .map(str::to_owned)
                .collect(),
        ))
    }
}

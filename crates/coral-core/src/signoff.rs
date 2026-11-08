//! Whether commits made in a repository carry a `Signed-off-by` line unless asked otherwise.
//!
//! A project that takes contributions under a Developer Certificate of Origin wants one on
//! every commit, and a box that has to be ticked each time is a box that gets forgotten. git
//! has no setting of its own for this on `commit` (`format.signOff` is for `format-patch`), so it
//! is Coral's, kept in the repository's config where git in a terminal can see it too.

use crate::config::{AppConfig, Scope, truthy};
use crate::error::CoralError;
use crate::process::GitRunner;
use crate::repo::RepoLocation;

const KEY: &str = "coral.signOff";

impl RepoLocation {
    /// Whether this repository signs off its commits by default.
    ///
    /// # Errors
    /// Propagates git failures.
    pub async fn signs_off(&self, runner: &GitRunner) -> Result<bool, CoralError> {
        Ok(self
            .config_get(runner, Scope::Effective(AppConfig(None)), KEY)
            .await?
            .is_some_and(|v| truthy(&v)))
    }

    /// Sets whether this repository signs off its commits by default.
    ///
    /// # Errors
    /// Propagates git failures.
    pub async fn set_signs_off(&self, runner: &GitRunner, on: bool) -> Result<(), CoralError> {
        let value = on.then_some("true");
        self.config_put(runner, Scope::Repository, KEY, value).await
    }
}

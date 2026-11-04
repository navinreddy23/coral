//! Where issues that are not the host's own live: a Jira, a Linear, anything a commit message
//! names as `KEY-123`. Kept in the repository's own config as a URL with `{key}` in it, so
//! everyone who clones a repository with it set in a shared config sees the same links.

use crate::config::{AppConfig, Scope};
use crate::error::CoralError;
use crate::process::GitRunner;
use crate::repo::RepoLocation;

/// The config key, under Coral's own section.
const KEY: &str = "coral.issueUrl";

impl RepoLocation {
    /// The issue tracker's URL, with `{key}` where `ABC-123` goes, if one is set.
    ///
    /// # Errors
    /// Propagates git failures.
    pub async fn tracker(&self, runner: &GitRunner) -> Result<Option<String>, CoralError> {
        Ok(self
            .config_get(runner, Scope::Effective(AppConfig(None)), KEY)
            .await?
            .filter(|url| url.contains("{key}")))
    }

    /// Sets the tracker's URL for this repository, or unsets it with `None`.
    ///
    /// # Errors
    /// [`CoralError::Refused`] for a URL with no `{key}` in it, which could only ever link
    /// every reference to the same page; otherwise git failures.
    pub async fn set_tracker(
        &self,
        runner: &GitRunner,
        url: Option<&str>,
    ) -> Result<(), CoralError> {
        let url = url.map(str::trim).filter(|u| !u.is_empty());
        if let Some(url) = url
            && !url.contains("{key}")
        {
            return Err(CoralError::Refused {
                label: "link issues",
                detail: "the address needs {key} where ABC-123 goes".to_owned(),
            });
        }
        self.config_put(runner, Scope::Repository, KEY, url).await
    }
}

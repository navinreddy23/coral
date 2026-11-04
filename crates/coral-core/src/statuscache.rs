//! The two caches git can keep in the index so a status does not have to look at everything:
//! the untracked cache, which remembers which directories held nothing new, and the
//! filesystem monitor, a daemon that tells git which paths changed since it last asked.
//!
//! Both are git's own, switched on in the repository's config, so a terminal beside Coral is
//! as fast as Coral is. Neither changes what a status says, only how long it takes to say it.
//! <https://git-scm.com/docs/git-update-index#_untracked_cache>
//! <https://git-scm.com/docs/git-fsmonitor--daemon>

use crate::config::{AppConfig, Scope};
use crate::error::CoralError;
use crate::process::{GitCommand, GitRunner};
use crate::repo::RepoLocation;

/// Whether each cache is on here, and whether this git can run the monitor at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub struct StatusCaches {
    pub untracked_cache: bool,
    pub fsmonitor: bool,
    /// Whether this git was built with its own monitor daemon. Builds for macOS and Windows
    /// are; the usual Linux ones are not, and there `core.fsmonitor = true` makes every
    /// status fail rather than run slowly.
    pub fsmonitor_supported: bool,
}

/// Whether `git version --build-options` names the built-in monitor.
///
/// # Errors
/// Propagates git failures.
pub async fn fsmonitor_supported(runner: &GitRunner) -> Result<bool, CoralError> {
    let out = runner
        .output(
            GitCommand::read("version", std::env::temp_dir()).args(["version", "--build-options"]),
        )
        .await?;
    Ok(String::from_utf8_lossy(&out.stdout).contains("fsmonitor--daemon"))
}

impl RepoLocation {
    /// The caches as git will use them here, every level of config considered.
    ///
    /// # Errors
    /// Propagates git failures.
    pub async fn status_caches(&self, runner: &GitRunner) -> Result<StatusCaches, CoralError> {
        let effective = Scope::Effective(AppConfig(None));
        let untracked = self
            .config_get(runner, effective, "core.untrackedCache")
            .await?;
        let monitor = self.config_get(runner, effective, "core.fsmonitor").await?;
        Ok(StatusCaches {
            untracked_cache: untracked.as_deref().is_some_and(crate::config::truthy),
            fsmonitor: monitor.as_deref().is_some_and(crate::config::truthy),
            fsmonitor_supported: fsmonitor_supported(runner).await?,
        })
    }

    /// Turns the untracked cache on or off for this repository. The index takes it on, or
    /// drops it, at the next status.
    ///
    /// # Errors
    /// Propagates git failures.
    pub async fn set_untracked_cache(
        &self,
        runner: &GitRunner,
        on: bool,
    ) -> Result<(), CoralError> {
        self.config_put(
            runner,
            Scope::Repository,
            "core.untrackedCache",
            Some(if on { "true" } else { "false" }),
        )
        .await
    }

    /// Turns git's own filesystem monitor on or off for this repository.
    ///
    /// # Errors
    /// [`CoralError::Refused`] when turning it on with a git that has no monitor to run, which
    /// would make every status here fail. Otherwise propagates git failures.
    pub async fn set_fsmonitor(&self, runner: &GitRunner, on: bool) -> Result<(), CoralError> {
        if on && !fsmonitor_supported(runner).await? {
            return Err(CoralError::Refused {
                label: "turn on the filesystem monitor",
                detail: "this git was built without one; git for macOS and Windows has it, \
                         the usual Linux builds do not"
                    .to_owned(),
            });
        }
        self.config_put(
            runner,
            Scope::Repository,
            "core.fsmonitor",
            Some(if on { "true" } else { "false" }),
        )
        .await?;
        if !on {
            // A daemon already watching keeps running until told; stopping it is best-effort,
            // since one that was never started has nothing to stop.
            let _ = runner
                .output(
                    GitCommand::write("fsmonitor--daemon", self.display_path())
                        .args(["fsmonitor--daemon", "stop"]),
                )
                .await;
        }
        Ok(())
    }
}

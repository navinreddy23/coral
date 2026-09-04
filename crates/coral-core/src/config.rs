//! Reading and writing git configuration, at the level that owns it.
//!
//! Both the signing settings and the ssh settings are layered the same way: an app-level
//! default that every repository inherits, and a per-repository override. That layering is
//! git's own, which is what makes the command line agree with Coral, and it lives here rather
//! than in either of the modules that use it.

use std::path::Path;

use crate::error::CoralError;
use crate::process::{GitCommand, GitRunner};
use crate::repo::RepoLocation;

/// Where the app-level defaults live.
///
/// `None` is the user's own git configuration, which is what the application uses: putting
/// them there means the command line sees the same settings rather than a private store git
/// would ignore. A path is how the tests stay off the machine they run on.
#[derive(Clone, Copy, Debug, Default)]
pub struct AppConfig<'a>(pub Option<&'a Path>);

/// Which configuration a read or write applies to.
#[derive(Clone, Copy, Debug)]
pub enum Scope<'a> {
    /// Every level merged, which is what git will actually do.
    Effective(AppConfig<'a>),
    /// The app-level defaults alone.
    App(AppConfig<'a>),
    /// This repository alone.
    Repository,
}

impl Scope<'_> {
    pub(crate) fn flags(self) -> Vec<String> {
        match self {
            // Merged, which is what `git config --get` reports on its own.
            Self::Effective(_) => Vec::new(),
            Self::App(_) => vec!["--global".to_owned()],
            Self::Repository => vec!["--local".to_owned()],
        }
    }

    /// Substitutes the app-level file for the user's own.
    ///
    /// `GIT_CONFIG_GLOBAL` rather than `--file`, because the file has to sit at the level it
    /// stands for: `--file` reads that file alone, which would report an app-level value as
    /// the effective one even where the repository overrides it. Substituting the global keeps
    /// the layering git already has. The system file goes with it, or the machine's own
    /// settings would leak into what is meant to be an isolated one.
    pub(crate) fn env(self) -> Option<[(&'static str, String); 2]> {
        let path = match self {
            Self::Effective(AppConfig(path)) | Self::App(AppConfig(path)) => path?,
            Self::Repository => return None,
        };
        Some([
            ("GIT_CONFIG_GLOBAL", path.display().to_string()),
            ("GIT_CONFIG_NOSYSTEM", "1".to_owned()),
        ])
    }
}

impl RepoLocation {
    /// Reads one key, or `None` when it is not set at that level.
    pub(crate) async fn config_get(
        &self,
        runner: &GitRunner,
        scope: Scope<'_>,
        key: &str,
    ) -> Result<Option<String>, CoralError> {
        let mut cmd = GitCommand::read("config", self.display_path())
            .arg("config")
            .args(scope.flags());
        for (name, value) in scope.env().into_iter().flatten() {
            cmd = cmd.env(name, value);
        }
        match runner.output(cmd.args(["--get", key])).await {
            Ok(out) => Ok(Some(String::from_utf8_lossy(&out.stdout).trim().to_owned())),
            // 1 is "no such key". 128 is a missing file, which a global config that has never
            // been written is; neither is a failure.
            Err(CoralError::GitExit { code: 1 | 128, .. }) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Writes one key, or unsets it when the value is `None`.
    pub(crate) async fn config_put(
        &self,
        runner: &GitRunner,
        scope: Scope<'_>,
        key: &str,
        value: Option<&str>,
    ) -> Result<(), CoralError> {
        let mut base = GitCommand::write("config", self.display_path())
            .arg("config")
            .args(scope.flags());
        for (name, value) in scope.env().into_iter().flatten() {
            base = base.env(name, value);
        }
        let cmd = match value {
            Some(value) => base.arg(key).arg(value),
            None => base.args(["--unset-all", key]),
        };
        match runner.output(cmd).await {
            // 5 is "nothing was set", which for an unset is the state that was asked for.
            Ok(_) | Err(CoralError::GitExit { code: 5, .. }) => Ok(()),
            Err(e) => Err(e),
        }
    }
}

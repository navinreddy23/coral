use coral_core::CoralError;
use coral_core::process::{GitRunner, GitVersion, MIN_GIT};

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionInfo {
    pub coral: &'static str,
    pub git: GitVersion,
    pub git_path: String,
    pub git_minimum: GitVersion,
}

/// Reports the git binary coral resolved and the floor it enforces.
///
/// # Errors
/// [`CoralError::GitMissing`] or [`CoralError::GitTooOld`].
pub async fn run() -> Result<VersionInfo, CoralError> {
    let runner = GitRunner::discover().await?;
    Ok(VersionInfo {
        coral: env!("CARGO_PKG_VERSION"),
        git: runner.version(),
        git_path: runner.path().display().to_string(),
        git_minimum: MIN_GIT,
    })
}

impl crate::output::Human for VersionInfo {
    fn human(&self) -> String {
        format!(
            "coral {}\n  git {} at {} (minimum {})",
            self.coral, self.git, self.git_path, self.git_minimum
        )
    }
}

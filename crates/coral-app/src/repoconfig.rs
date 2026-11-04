//! The per-repository switches that are git configuration rather than Coral's own: how this
//! repository merges and how fast its status is.

use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;

use crate::commands::IpcError;

/// What the Repository pane shows.
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoConfig {
    /// git records each resolved conflict and resolves the same one the same way next time.
    pub rerere: bool,
}

async fn read(runner: &GitRunner, loc: &RepoLocation) -> Result<RepoConfig, IpcError> {
    Ok(RepoConfig {
        rerere: loc.rerere_enabled(runner).await?,
    })
}

/// The repository's switches as they stand.
///
/// # Errors
/// Propagates git failures.
#[tauri::command]
pub async fn repo_config(path: String) -> Result<RepoConfig, IpcError> {
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, std::path::Path::new(&path)).await?;
    read(&runner, &loc).await
}

/// Turns reuse of recorded resolutions on or off for this repository.
///
/// # Errors
/// Propagates git failures.
#[tauri::command]
pub async fn repo_set_rerere(path: String, on: bool) -> Result<RepoConfig, IpcError> {
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, std::path::Path::new(&path)).await?;
    loc.set_rerere(&runner, on).await?;
    read(&runner, &loc).await
}

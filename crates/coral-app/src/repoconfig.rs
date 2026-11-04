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
    /// The two caches git keeps in the index to answer a status quickly.
    pub caches: coral_core::statuscache::StatusCaches,
}

async fn read(runner: &GitRunner, loc: &RepoLocation) -> Result<RepoConfig, IpcError> {
    Ok(RepoConfig {
        rerere: loc.rerere_enabled(runner).await?,
        caches: loc.status_caches(runner).await?,
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

/// Turns the untracked cache on or off for this repository.
///
/// # Errors
/// Propagates git failures.
#[tauri::command]
pub async fn repo_set_untracked_cache(path: String, on: bool) -> Result<RepoConfig, IpcError> {
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, std::path::Path::new(&path)).await?;
    loc.set_untracked_cache(&runner, on).await?;
    read(&runner, &loc).await
}

/// Turns git's own filesystem monitor on or off for this repository.
///
/// # Errors
/// Refuses to turn it on with a git that has none; otherwise propagates git failures.
#[tauri::command]
pub async fn repo_set_fsmonitor(path: String, on: bool) -> Result<RepoConfig, IpcError> {
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, std::path::Path::new(&path)).await?;
    loc.set_fsmonitor(&runner, on).await?;
    read(&runner, &loc).await
}

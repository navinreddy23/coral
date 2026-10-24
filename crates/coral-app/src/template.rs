//! The commit message template, for the message box to start from.

use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;

use crate::commands::IpcError;

/// The repository's commit message template, without its comment lines, or `None`.
///
/// # Errors
/// Propagates git failures.
#[tauri::command]
pub async fn commit_template(path: String) -> Result<Option<String>, IpcError> {
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, std::path::Path::new(&path)).await?;
    Ok(loc.commit_template(&runner).await?)
}

//! A commit's note, for the details panel to show and edit.

use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;

use crate::commands::IpcError;

/// The note on `rev`, or `None`.
///
/// # Errors
/// Propagates git failures, such as an unknown revision.
#[tauri::command]
pub async fn commit_note(path: String, rev: String) -> Result<Option<String>, IpcError> {
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, std::path::Path::new(&path)).await?;
    Ok(loc.note(&runner, &rev).await?)
}

/// Sets the note on `rev`; empty text removes it.
///
/// # Errors
/// Propagates git failures, such as an unknown revision.
#[tauri::command]
pub async fn set_note(path: String, rev: String, text: String) -> Result<(), IpcError> {
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, std::path::Path::new(&path)).await?;
    Ok(loc.set_note(&runner, &rev, &text).await?)
}

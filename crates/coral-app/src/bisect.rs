//! Reading a bisection for the window. Marking and ending one are actions, so they report
//! and refresh the way every other operation does.

use coral_core::bisect::Bisection;

use crate::commands::IpcError;

/// The bisection in progress, or nothing when there is none.
///
/// # Errors
/// Propagates git failures.
#[tauri::command]
pub async fn repo_bisection(path: String) -> Result<Option<Bisection>, IpcError> {
    let runner = coral_core::process::GitRunner::discover().await?;
    let loc =
        coral_core::repo::RepoLocation::discover(&runner, std::path::Path::new(&path)).await?;
    Ok(loc.bisection(&runner).await?)
}

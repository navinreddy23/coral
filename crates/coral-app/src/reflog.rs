//! Reading where HEAD has been, for the window's recovery sheet. Bringing a commit back is a
//! branch or a checkout, which are actions like any other.

use coral_core::reflog::ReflogEntry;

use crate::commands::IpcError;

/// How far back the sheet reads. A busy repository moves HEAD a few hundred times a month,
/// and the work people come looking for was lost recently.
const DEPTH: usize = 1000;

/// HEAD's recent moves, newest first, with the commits nothing else reaches marked.
///
/// # Errors
/// Propagates git failures.
#[tauri::command]
pub async fn repo_reflog(path: String) -> Result<Vec<ReflogEntry>, IpcError> {
    let runner = coral_core::process::GitRunner::discover().await?;
    let loc =
        coral_core::repo::RepoLocation::discover(&runner, std::path::Path::new(&path)).await?;
    Ok(loc.reflog(&runner, DEPTH).await?)
}

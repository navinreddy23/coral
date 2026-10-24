//! Ignoring untracked files from the staging panel.

use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;

use crate::commands::IpcError;

/// What to ignore.
#[derive(Debug, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Ignored {
    /// One path, and nothing that merely shares its name.
    File { file: String },
    /// A pattern as written, such as `*.log` or `/build/`.
    Pattern { pattern: String },
}

/// Adds a line for `what` to the repository's top-level `.gitignore`.
///
/// # Errors
/// Refused for a bare repository; otherwise I/O failures on the file.
#[tauri::command]
pub async fn ignore(path: String, what: Ignored) -> Result<(), IpcError> {
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, std::path::Path::new(&path)).await?;
    let pattern = match what {
        Ignored::File { file } => coral_core::gitignore::exact(&file),
        Ignored::Pattern { pattern } => pattern,
    };
    Ok(loc.ignore(&pattern)?)
}

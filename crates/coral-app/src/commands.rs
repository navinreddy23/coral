use coral_core::process::GitRunner;
use coral_core::repo::{RepoInfo, RepoLocation};

/// Errors cross the IPC boundary as the same stable `code` the CLI envelope uses, so the UI
/// has one error vocabulary regardless of which front end it is talking to.
#[derive(serde::Serialize)]
pub struct IpcError {
    code: &'static str,
    message: String,
}

impl From<coral_core::CoralError> for IpcError {
    fn from(e: coral_core::CoralError) -> Self {
        Self {
            code: e.code(),
            message: e.to_string(),
        }
    }
}

/// Which repository the window should open with.
///
/// `CORAL_REPO` exists so the app can be pointed at a repository without a file dialog, which
/// is what makes benchmarking against a large clone possible. M5 replaces this with the tab
/// session.
#[tauri::command]
pub fn initial_repo() -> String {
    std::env::var("CORAL_REPO").unwrap_or_else(|_| ".".to_owned())
}

#[tauri::command]
pub async fn open_repo(path: String) -> Result<RepoInfo, IpcError> {
    tracing::info!(path, "open_repo");
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, std::path::Path::new(&path)).await?;
    Ok(loc.info(&runner).await?)
}

use coral_core::process::GitRunner;
use coral_core::repo::{RepoInfo, RepoLocation};

/// Errors cross the IPC boundary as the same stable `code` the CLI envelope uses, so the UI
/// has one error vocabulary regardless of which front end it is talking to.
#[derive(Debug, serde::Serialize)]
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
#[must_use]
pub fn initial_repo() -> String {
    std::env::var("CORAL_REPO").unwrap_or_else(|_| ".".to_owned())
}

/// # Errors
/// [`coral_core::CoralError::NotARepository`], or any git failure.
#[tauri::command]
pub async fn open_repo(path: String) -> Result<RepoInfo, IpcError> {
    tracing::info!(path, "open_repo");
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, std::path::Path::new(&path)).await?;
    Ok(loc.info(&runner).await?)
}

/// The working tree, for the WIP row and the staging panel.
/// # Errors
/// Propagates git failures.
#[tauri::command]
pub async fn repo_status(path: String) -> Result<coral_core::Status, IpcError> {
    let runner = coral_core::process::GitRunner::discover().await?;
    let loc =
        coral_core::repo::RepoLocation::discover(&runner, std::path::Path::new(&path)).await?;
    Ok(loc.status(&runner).await?)
}

/// Stages or unstages whole paths, then reports the resulting status.
///
/// Returning the new status rather than nothing means the panel cannot drift from the
/// repository: there is no separate refresh to miss.
/// # Errors
/// Propagates git failures, including a path that does not exist.
#[tauri::command]
pub async fn stage_paths(
    path: String,
    paths: Vec<String>,
    stage: bool,
) -> Result<coral_core::Status, IpcError> {
    let runner = coral_core::process::GitRunner::discover().await?;
    let loc =
        coral_core::repo::RepoLocation::discover(&runner, std::path::Path::new(&path)).await?;
    let refs: Vec<&str> = paths.iter().map(String::as_str).collect();

    if stage {
        loc.stage(&runner, &refs).await?;
    } else {
        loc.unstage(&runner, &refs).await?;
    }
    Ok(loc.status(&runner).await?)
}

/// Records a commit from what is staged, then reports the resulting status.
/// # Errors
/// Propagates git failures, including a rejecting hook.
#[tauri::command]
pub async fn commit_staged(
    path: String,
    message: String,
    amend: bool,
) -> Result<coral_core::Status, IpcError> {
    let runner = coral_core::process::GitRunner::discover().await?;
    let loc =
        coral_core::repo::RepoLocation::discover(&runner, std::path::Path::new(&path)).await?;
    let opts = coral_core::ops::CommitOpts {
        message,
        amend,
        ..coral_core::ops::CommitOpts::default()
    };

    loc.commit(&runner, &opts).await?;
    Ok(loc.status(&runner).await?)
}

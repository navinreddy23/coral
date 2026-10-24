//! Git LFS from the staging panel and a commit's files: tracking a type, and locks.

use coral_core::lfs::LfsLock;
use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;

use crate::commands::IpcError;

async fn located(path: &str) -> Result<(GitRunner, RepoLocation), coral_core::CoralError> {
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, std::path::Path::new(path)).await?;
    Ok((runner, loc))
}

/// Puts files matching `pattern` in LFS from now on.
///
/// # Errors
/// git-lfs's failures, including its absence.
#[tauri::command]
pub async fn lfs_track(path: String, pattern: String) -> Result<(), IpcError> {
    let (runner, loc) = located(&path).await?;
    Ok(loc.lfs_track(&runner, &pattern).await?)
}

/// Which of `files` LFS already keeps, so a menu need not offer to put them there.
///
/// # Errors
/// Propagates git failures.
#[tauri::command]
pub async fn lfs_kept(path: String, files: Vec<String>) -> Result<Vec<String>, IpcError> {
    let (runner, loc) = located(&path).await?;
    let mut kept: Vec<String> = loc.in_lfs(&runner, &files).await?.into_iter().collect();
    kept.sort();
    Ok(kept)
}

/// The locks on the LFS server, or `None` for a repository that keeps nothing in LFS.
///
/// # Errors
/// The server's refusal, or no answer in time.
#[tauri::command]
pub async fn lfs_locks(path: String) -> Result<Option<Vec<LfsLock>>, IpcError> {
    let (runner, loc) = located(&path).await?;
    if !loc.uses_lfs() {
        return Ok(None);
    }
    Ok(Some(loc.lfs_locks(&runner).await?))
}

/// Locks `file` on the LFS server.
///
/// # Errors
/// The server's refusal, such as a lock somebody else holds.
#[tauri::command]
pub async fn lfs_lock(path: String, file: String) -> Result<(), IpcError> {
    let (runner, loc) = located(&path).await?;
    Ok(loc.lfs_lock(&runner, &file).await?)
}

/// Releases the lock on `file`; `force` releases somebody else's.
///
/// # Errors
/// The server's refusal.
#[tauri::command]
pub async fn lfs_unlock(path: String, file: String, force: bool) -> Result<(), IpcError> {
    let (runner, loc) = located(&path).await?;
    Ok(loc.lfs_unlock(&runner, &file, force).await?)
}

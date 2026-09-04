//! Telling the window when the repository changed underneath it.
//!
//! Everything a user can do in Coral they can also do in the terminal beside it, and a client
//! that only notices its own writes shows the wrong branch until something else happens to
//! reload. The engine already classifies and debounces the file events; this carries them
//! across the IPC boundary and makes sure only one repository is watched at a time.

// Tauri requires `State` by value in a command signature; it is a handle, not the data.
#![allow(clippy::needless_pass_by_value)]

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;
use coral_core::watch::RepoWatcher;
use tauri::Emitter as _;

use crate::commands::IpcError;

/// The event the window listens on.
pub const EVENT: &str = "repo://changed";

/// Which watch is current.
///
/// A counter rather than a handle: the watcher lives inside the task that reads from it, so
/// stopping one means telling that task it is no longer the current one. Storing the watcher
/// here instead would need the receiver taken out of it, and dropping the watcher while its
/// task still held the receiver is what leaves a thread blocked forever.
#[derive(Default)]
pub struct Watchers {
    generation: Arc<AtomicU64>,
}

/// What starting a watch reports back.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Watching {
    /// False when the worktree could not be watched, so the window knows to refresh on focus.
    pub complete: bool,
    /// Why, when it could not.
    pub detail: Option<String>,
}

/// Watches one repository, replacing whatever was being watched before.
///
/// # Errors
/// Propagates git failures from opening the repository, and [`coral_core::CoralError::Io`] when
/// even the git directory cannot be watched.
#[tauri::command]
pub async fn watch_repo(
    app: tauri::AppHandle,
    watchers: tauri::State<'_, Watchers>,
    path: String,
) -> Result<Watching, IpcError> {
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, std::path::Path::new(&path)).await?;

    let generation = Arc::clone(&watchers.generation);
    let me = generation.fetch_add(1, Ordering::SeqCst) + 1;

    let mut watcher = RepoWatcher::start(&loc)?;
    let degraded = watcher.degraded();

    tauri::async_runtime::spawn(async move {
        // Owning the watcher here is what keeps it alive; the loop ends when a newer watch has
        // taken over, when the sender is dropped, or when the window has gone.
        while let Some(change) = watcher.recv().await {
            if generation.load(Ordering::SeqCst) != me {
                return;
            }
            if app.emit(EVENT, change).is_err() {
                return;
            }
        }
    });

    Ok(Watching {
        complete: degraded.is_none(),
        detail: degraded,
    })
}

/// Stops watching. Closing a repository should not leave a thread reading its directory.
///
/// # Errors
/// Never; the signature is a `Result` because every command in this layer is one.
#[tauri::command]
#[allow(clippy::unused_async)]
pub async fn unwatch_repo(watchers: tauri::State<'_, Watchers>) -> Result<(), IpcError> {
    watchers.generation.fetch_add(1, Ordering::SeqCst);
    Ok(())
}

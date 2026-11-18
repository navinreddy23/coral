//! Searching the files of a revision for the window's code search, one search per repository.
//!
//! Kept apart from the find box's registry: the two are open together, and a search typed into
//! one must not stop the other. Each new code search of a repository stops the one still running
//! there, so typing with pauses costs one `git grep` rather than one per pause.

// Tauri requires `State` by value in a command signature; it is a handle, not the data.
#![allow(clippy::needless_pass_by_value)]

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};

use coral_core::grep::{GrepQuery, GrepResult};
use tokio::sync::oneshot;

/// The code search running in each repository, by path, and how to stop it.
#[derive(Default)]
pub struct Greps {
    running: Mutex<HashMap<String, (u64, oneshot::Sender<()>)>>,
    next: AtomicU64,
}

impl Greps {
    /// Registers a search of `path`, stopping the one before it by dropping its sender.
    fn start(&self, path: &str) -> (u64, oneshot::Receiver<()>) {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.held().insert(path.to_owned(), (id, tx));
        (id, rx)
    }

    /// Forgets a search that finished, unless a newer one has already taken its place.
    fn finish(&self, path: &str, id: u64) {
        let mut held = self.held();
        if held.get(path).is_some_and(|(running, _)| *running == id) {
            held.remove(path);
        }
    }

    fn stop(&self, path: &str) -> bool {
        self.held().remove(path).is_some()
    }

    fn held(&self) -> std::sync::MutexGuard<'_, HashMap<String, (u64, oneshot::Sender<()>)>> {
        self.running.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// The lines of the files at `rev` that match `query`.
///
/// # Errors
/// Propagates git failures, which include a pattern git cannot read, and is
/// [`coral_core::CoralError::Cancelled`] when a newer search or [`stop_grep`] stopped it.
#[tauri::command]
pub async fn grep_code(
    greps: tauri::State<'_, Greps>,
    path: String,
    rev: String,
    query: GrepQuery,
) -> Result<GrepResult, crate::commands::IpcError> {
    let (id, stopped) = greps.start(&path);
    let found = tokio::select! {
        found = grep(&path, &rev, &query) => found,
        _ = stopped => Err(coral_core::CoralError::Cancelled { label: "code search" }),
    };
    greps.finish(&path, id);
    let found = found?;
    tracing::info!(
        rev,
        matched = found.matches.len(),
        more = found.more,
        "grep_code"
    );
    Ok(found)
}

async fn grep(
    path: &str,
    rev: &str,
    query: &GrepQuery,
) -> Result<GrepResult, coral_core::CoralError> {
    let runner = coral_core::process::GitRunner::discover().await?;
    let loc = coral_core::repo::RepoLocation::discover(&runner, std::path::Path::new(path)).await?;
    loc.grep(&runner, rev, query).await
}

/// Stops the code search running in `path`, answering whether there was one.
#[tauri::command]
#[must_use]
pub fn stop_grep(greps: tauri::State<'_, Greps>, path: String) -> bool {
    greps.stop(&path)
}

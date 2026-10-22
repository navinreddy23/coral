//! Finding commits from the find box, one search per repository at a time.
//!
//! A search is a walk of the whole history, and a `code:` search reads every diff on the way:
//! minutes on the kernel. Each new search of a repository stops the one still running there,
//! and closing the box stops it too, so a phrase typed with pauses in it costs one walk rather
//! than one per pause. Stopping drops the work, and the runner kills git with it.

// Tauri requires `State` by value in a command signature; it is a handle, not the data.
#![allow(clippy::needless_pass_by_value)]

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};

use tokio::sync::oneshot;

use crate::graph::GraphCache;
use crate::scope::Scopes;

/// The search running in each repository, by path, and how to stop it.
#[derive(Default)]
pub struct Searches {
    running: Mutex<HashMap<String, (u64, oneshot::Sender<()>)>>,
    next: AtomicU64,
}

impl Searches {
    /// Registers a search of `path`, stopping the one before it. Dropping the sender of the
    /// one it replaces is what stops it.
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

/// A commit a search matched, placed on the row it sits on.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FoundCommit {
    pub oid: String,
    pub row: u32,
}

/// Commits whose message, author, path or changes match `query`, in graph order.
///
/// Sorted by row rather than by date, because the answer is stepped through with a next and a
/// previous button against the list on screen, and a search that jumped about would be no help
/// at all. Matches on commits the loaded walk does not hold are dropped: there is nowhere to
/// send the reader.
///
/// # Errors
/// Propagates git failures and an unreadable qualifier, and is
/// [`coral_core::CoralError::Cancelled`] when a newer search or a closed box stopped it.
#[tauri::command]
pub async fn search_commits(
    cache: tauri::State<'_, GraphCache>,
    scopes: tauri::State<'_, Scopes>,
    searches: tauri::State<'_, Searches>,
    path: String,
    query: String,
    limit: u64,
) -> Result<Vec<FoundCommit>, crate::commands::IpcError> {
    let (id, stopped) = searches.start(&path);
    let found = tokio::select! {
        found = find(&path, &query, limit) => found,
        _ = stopped => Err(coral_core::CoralError::Cancelled { label: "search" }),
    };
    searches.finish(&path, id);
    let found = found?;
    tracing::info!(query, matched = found.len(), "search_commits");
    if found.is_empty() {
        return Ok(Vec::new());
    }

    // Whatever walk is in hand: a search that has already taken ten seconds must not then
    // wait on a walk, and the rows it places matches at are the rows on screen.
    let store = cache.best_store(&path, &scopes.read(&path)).await?;
    let mut out: Vec<FoundCommit> = found
        .into_iter()
        .filter_map(|oid| {
            let row = gix::ObjectId::from_hex(oid.as_bytes())
                .ok()
                .and_then(|id| store.row_of(&id))?;
            Some(FoundCommit { oid, row })
        })
        .collect();
    out.sort_by_key(|f| f.row);
    Ok(out)
}

async fn find(path: &str, query: &str, limit: u64) -> Result<Vec<String>, coral_core::CoralError> {
    let runner = coral_core::process::GitRunner::discover().await?;
    let loc = coral_core::repo::RepoLocation::discover(&runner, std::path::Path::new(path)).await?;
    loc.search_commits(&runner, query, limit).await
}

/// Stops the search running in `path`, answering whether there was one.
#[tauri::command]
#[must_use]
pub fn stop_search(searches: tauri::State<'_, Searches>, path: String) -> bool {
    searches.stop(&path)
}

#[cfg(test)]
mod tests {
    use super::Searches;

    #[tokio::test]
    async fn a_new_search_stops_the_one_before_it() {
        let searches = Searches::default();
        let (first, stopped) = searches.start("/repo");
        let (second, _running) = searches.start("/repo");
        assert!(stopped.await.is_err(), "the first was told to stop");
        // The first finishing late must not forget the second.
        searches.finish("/repo", first);
        assert!(searches.stop("/repo"));
        searches.finish("/repo", second);
    }

    #[tokio::test]
    async fn another_repository_is_left_alone() {
        let searches = Searches::default();
        let (_, mut kernel) = searches.start("/kernel");
        let _ = searches.start("/coral");
        assert!(
            kernel
                .try_recv()
                .is_err_and(|e| e == tokio::sync::oneshot::error::TryRecvError::Empty)
        );
    }

    #[test]
    fn stopping_with_nothing_running_is_not_an_error() {
        let searches = Searches::default();
        assert!(!searches.stop("/repo"));
        let (id, _rx) = searches.start("/repo");
        searches.finish("/repo", id);
        assert!(!searches.stop("/repo"), "a finished search is gone");
    }
}

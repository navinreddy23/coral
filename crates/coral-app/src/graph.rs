use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, PoisonError};

use coral_core::CoralError;
use coral_core::graph::{GixCommitStream, RowStore, StreamOpts, build, wire};
use tokio::sync::Mutex;

/// How many repositories keep their walk. A `RowStore` for the kernel is about eighty
/// megabytes, so this cannot be unbounded; it is comfortably more tabs than anyone switches
/// between, and the one that goes is the one nobody has looked at for longest.
const KEEP: usize = 6;

/// The row stores for the repositories that are open, one slot each.
///
/// Rebuilding the graph per frame would cost seconds, so the store is built once and sliced.
/// One slot for the whole application was worse than no cache at all past the first tab: every
/// switch evicted the walk it was switching away from, so switching back walked 1.8M commits
/// again, and — because the slot's lock was held for the whole walk — a fifteen-commit
/// repository could not be drawn until the kernel had finished being walked.
#[derive(Default)]
pub struct GraphCache {
    slots: std::sync::Mutex<HashMap<String, Arc<Slot>>>,
    /// Counts uses, so the least recently used slot can be named without a clock.
    tick: AtomicU64,
}

/// One repository's walk, and its own lock.
///
/// Per repository rather than one for all of them: the lock is held across the walk on purpose,
/// so two requests for the same repository share one walk instead of racing to do it twice.
/// Sharing it between repositories is what made them wait for each other.
#[derive(Default)]
struct Slot {
    held: Mutex<Option<Cached>>,
    /// The commit-time walk, kept where it can be read while the topological one is running.
    ///
    /// The point of painting in sixteen milliseconds is undone if the rows painted have no
    /// author and no message. They had none: asking for a row's metadata asked for the
    /// topological store, and waited the six seconds that store took to build, so a large
    /// repository showed a column of dots and nothing else for as long as it walked. This is a
    /// plain lock that is never held across an await, so reading it cannot wait for a walk.
    quick: std::sync::Mutex<Option<Arc<RowStore>>>,
    used: AtomicU64,
    /// What the refs hashed to when the held walk was asked for, so a tab switch back to a
    /// repository nobody has touched is free rather than another walk of it.
    refs: AtomicU64,
}

impl Slot {
    fn quick_store(&self) -> Option<Arc<RowStore>> {
        self.quick
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn set_quick(&self, store: Option<Arc<RowStore>>) {
        *self.quick.lock().unwrap_or_else(PoisonError::into_inner) = store;
    }
}

struct Cached {
    /// True when this store came from a commit-time walk and is not topologically sound.
    provisional: bool,
    store: Arc<RowStore>,
}

impl GraphCache {
    /// The slot for `path`, making one if this repository has not been walked yet.
    fn slot(&self, path: &str) -> Arc<Slot> {
        let now = self.tick.fetch_add(1, Ordering::Relaxed);
        let mut slots = self.slots.lock().unwrap_or_else(PoisonError::into_inner);

        if let Some(slot) = slots.get(path) {
            slot.used.store(now, Ordering::Relaxed);
            return Arc::clone(slot);
        }
        if slots.len() >= KEEP
            && let Some(oldest) = slots
                .iter()
                .min_by_key(|(_, slot)| slot.used.load(Ordering::Relaxed))
                .map(|(held, _)| held.clone())
        {
            slots.remove(&oldest);
        }

        let slot = Arc::new(Slot {
            held: Mutex::new(None),
            quick: std::sync::Mutex::new(None),
            used: AtomicU64::new(now),
            refs: AtomicU64::new(0),
        });
        slots.insert(path.to_owned(), Arc::clone(&slot));
        slot
    }

    /// Drops the walk held for `path` when `refs` says the repository has moved since.
    ///
    /// Dropping it unconditionally is what made every tab switch cost a full walk: coming back
    /// to a repository nobody had touched threw away a perfectly good six seconds of work and
    /// did it again. Returns whether the walk was dropped.
    async fn forget_if_moved(&self, path: &str, refs: u64) -> bool {
        let slot = self.slot(path);
        let mut held = slot.held.lock().await;
        if slot.refs.swap(refs, Ordering::Relaxed) == refs && held.is_some() {
            return false;
        }
        *held = None;
        slot.set_quick(None);
        true
    }

    /// Returns the store for `path`, building it if it is not held.
    async fn store(&self, path: &str, first_paint: bool) -> Result<Arc<RowStore>, CoralError> {
        let slot = self.slot(path);
        // Answered without the lock, which the topological walk holds for as long as it runs.
        if first_paint && let Some(quick) = slot.quick_store() {
            return Ok(quick);
        }

        let mut held = slot.held.lock().await;
        // Serving a provisional store to the request that wants topological rows would mean the
        // real walk never ran and the user stayed on commit-time order for good.
        if let Some(cached) = held.as_ref()
            && (cached.provisional == first_paint || !cached.provisional)
        {
            return Ok(Arc::clone(&cached.store));
        }

        let owned = path.to_owned();
        let store = tokio::task::spawn_blocking(move || {
            let stream = GixCommitStream::open(std::path::Path::new(&owned))?;
            let opts = if first_paint {
                StreamOpts::first_paint(StreamOpts::FIRST_PAINT_ROWS)
            } else {
                StreamOpts::default()
            };
            build(&stream, &opts)
        })
        .await
        .map_err(|e| CoralError::Protocol {
            label: "graph",
            detail: e.to_string(),
        })??;

        let store = Arc::new(store);
        // The commit-time walk stays readable beside the lock; the topological one replaces it,
        // because from then on it is the walk the window is showing.
        slot.set_quick(first_paint.then(|| Arc::clone(&store)));
        *held = Some(Cached {
            provisional: first_paint,
            store: Arc::clone(&store),
        });
        Ok(store)
    }
}

/// Returns one binary frame of graph rows.
///
/// The response is raw bytes rather than JSON: a million rows as JSON objects would cost
/// hundreds of megabytes in the webview and seconds of parsing.
/// # Errors
/// Propagates walk failures.
#[tauri::command]
pub async fn graph_frame(
    cache: tauri::State<'_, GraphCache>,
    path: String,
    start_row: u32,
    first_paint: bool,
) -> Result<tauri::ipc::Response, crate::commands::IpcError> {
    tracing::info!(path, start_row, first_paint, "graph_frame");
    let store = cache.store(&path, first_paint).await?;
    tracing::info!(rows = store.len(), "graph_frame served");
    let hash_len = store.oid(0).map_or(20, |id| id.as_bytes().len());
    Ok(tauri::ipc::Response::new(wire::encode(
        &store, start_row, hash_len,
    )))
}

/// Author and summary for a window of rows.
///
/// Kept separate from the frame because the commit-graph carries neither, so these cost an
/// object read each. Fetching them only for rows on screen is what keeps scrolling cheap.
///
/// `provisional` says which walk the rows being asked about came from.
/// # Errors
/// Propagates git failures.
#[tauri::command]
pub async fn row_metadata(
    cache: tauri::State<'_, GraphCache>,
    path: String,
    start_row: u32,
    count: u32,
    provisional: bool,
) -> Result<Vec<coral_core::commit::CommitMeta>, crate::commands::IpcError> {
    // Against the walk the window is showing, not the one it will show next: a row is a
    // position in a particular walk, and reading it from the other one names a different commit.
    let store = cache.store(&path, provisional).await?;
    let end = (start_row + count).min(store.len());
    tracing::info!(
        start_row,
        rows = end.saturating_sub(start_row),
        "row_metadata"
    );
    let oids: Vec<String> = (start_row..end)
        .filter_map(|r| store.oid(r))
        .map(|id| id.to_string())
        .collect();

    let runner = coral_core::process::GitRunner::discover().await?;
    let loc =
        coral_core::repo::RepoLocation::discover(&runner, std::path::Path::new(&path)).await?;
    Ok(loc.commit_metadata(&runner, &oids).await?)
}

/// Everything the details panel shows for one commit.
/// # Errors
/// [`coral_core::CoralError::Refused`] for an unknown revision.
#[tauri::command]
pub async fn commit_detail(
    path: String,
    rev: String,
) -> Result<coral_core::commit::CommitDetail, crate::commands::IpcError> {
    let runner = coral_core::process::GitRunner::discover().await?;
    let loc =
        coral_core::repo::RepoLocation::discover(&runner, std::path::Path::new(&path)).await?;
    Ok(loc.commit_detail(&runner, &rev).await?)
}

/// A ref placed on the row it belongs to.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlacedRef {
    #[serde(flatten)]
    pub git_ref: coral_core::refs::GitRef,
    /// The row this ref labels, or `None` when its commit is outside the loaded graph.
    pub row: Option<u32>,
}

/// Every ref, each resolved to the row it labels.
///
/// The row lookup happens here rather than in the interface: the store already holds a sorted
/// index, so this is a binary search per ref instead of shipping 1.4M object ids to JavaScript
/// for it to build a map.
/// # Errors
/// Propagates git failures.
#[tauri::command]
pub async fn repo_refs(
    cache: tauri::State<'_, GraphCache>,
    path: String,
) -> Result<Vec<PlacedRef>, crate::commands::IpcError> {
    let runner = coral_core::process::GitRunner::discover().await?;
    let loc =
        coral_core::repo::RepoLocation::discover(&runner, std::path::Path::new(&path)).await?;
    let refs = loc.refs(&runner).await?;
    let store = cache.store(&path, false).await?;

    Ok(refs
        .into_iter()
        .map(|git_ref| {
            let row = gix::ObjectId::from_hex(git_ref.commit().as_bytes())
                .ok()
                .and_then(|id| store.row_of(&id));
            PlacedRef { git_ref, row }
        })
        .collect())
}

/// Walks `path` again if it has moved, so the next frame shows what is there now.
///
/// The cache used to be one slot keyed by path, which made a rewalk of the repository already
/// in it impossible: a commit, a stash, a branch — none of them appeared until the user opened
/// another repository and came back, because that is what replaced the slot. Asking for a walk
/// has to mean a walk. It must not mean one when nothing moved, though, which is why this
/// hashes the refs first: two milliseconds against six seconds on a repository of this size.
///
/// # Errors
/// Propagates git failures from reading the refs.
#[tauri::command]
pub async fn graph_rewalk(
    cache: tauri::State<'_, GraphCache>,
    path: String,
) -> Result<(), crate::commands::IpcError> {
    let runner = coral_core::process::GitRunner::discover().await?;
    let loc =
        coral_core::repo::RepoLocation::discover(&runner, std::path::Path::new(&path)).await?;
    let refs = loc.refs_fingerprint(&runner).await?;
    let dropped = cache.forget_if_moved(&path, refs).await;
    tracing::info!(path, dropped, "graph_rewalk");
    Ok(())
}

/// A stash entry placed on the row it sits on.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlacedStash {
    #[serde(flatten)]
    pub entry: coral_core::stash::StashEntry,
    /// What to call it, which is not what git calls it: stashing twice from one commit gives
    /// both entries the same subject, and a list of identical names is a list nobody can act
    /// on. Computed here rather than in the window, so the two front ends cannot disagree.
    pub name: String,
    /// The row it is drawn on, or `None` when its commit is outside the loaded graph.
    pub row: Option<u32>,
}

/// The stash stack, each entry resolved to the row it is drawn on.
///
/// The stack, not the ref. `refs/stash` is the top of it and the only stash git keeps a ref
/// for, so listing refs finds one stash however many there are — and calls it "stash", which is
/// also what it calls the next one.
///
/// # Errors
/// Propagates git failures.
#[tauri::command]
pub async fn repo_stashes(
    cache: tauri::State<'_, GraphCache>,
    path: String,
) -> Result<Vec<PlacedStash>, crate::commands::IpcError> {
    let runner = coral_core::process::GitRunner::discover().await?;
    let loc =
        coral_core::repo::RepoLocation::discover(&runner, std::path::Path::new(&path)).await?;
    let entries = loc.stashes(&runner).await?;
    let store = cache.store(&path, false).await?;

    Ok(entries
        .into_iter()
        .map(|entry| {
            let row = gix::ObjectId::from_hex(entry.oid.as_bytes())
                .ok()
                .and_then(|id| store.row_of(&id));
            PlacedStash {
                name: entry.name(),
                entry,
                row,
            }
        })
        .collect())
}

/// The row a commit sits on, or `None` when it is outside the graph that is loaded.
///
/// Wanted for a commit that no ref names — a detached HEAD above all, which has a row to show
/// and no label to find it by.
///
/// # Errors
/// Propagates git failures.
#[tauri::command]
pub async fn graph_row_of(
    cache: tauri::State<'_, GraphCache>,
    path: String,
    oid: String,
) -> Result<Option<u32>, crate::commands::IpcError> {
    let store = cache.store(&path, false).await?;
    Ok(gix::ObjectId::from_hex(oid.as_bytes())
        .ok()
        .and_then(|id| store.row_of(&id)))
}

/// A frame of known content, used once at startup to prove the binary path works.
///
/// Tauri's JavaScript falls back to `postMessage` permanently if the custom-protocol fetch
/// ever throws, and on that path a `Vec<u8>` is serialized as a JSON array of numbers — about
/// a hundred times slower, with no error anywhere. The UI checks this response's size and
/// contents at startup so the degradation is loud rather than silent.
#[tauri::command]
#[must_use]
pub fn binary_self_test() -> tauri::ipc::Response {
    tracing::info!("binary_self_test");
    // A recognisable pattern, long enough that a JSON-array fallback is obvious.
    let bytes: Vec<u8> = (0..4096_u32)
        .map(|i| u8::try_from(i % 251).unwrap_or(0))
        .collect();
    tauri::ipc::Response::new(bytes)
}

/// The repository's submodules, for the sidebar.
///
/// # Errors
/// Propagates git failures. A repository without a `.gitmodules` yields an empty list rather
/// than an error.
#[tauri::command]
pub async fn repo_submodules(
    path: String,
) -> Result<Vec<coral_core::submodule::Submodule>, crate::commands::IpcError> {
    let runner = coral_core::process::GitRunner::discover().await?;
    let loc =
        coral_core::repo::RepoLocation::discover(&runner, std::path::Path::new(&path)).await?;
    Ok(loc.submodules(&runner).await?)
}

/// The hunks for one file in one commit.
///
/// Fetched per file rather than with the commit: a kernel merge touches thousands of files,
/// and their patches together are far larger than anything the panel can show at once.
///
/// # Errors
/// Propagates git failures.
#[tauri::command]
pub async fn file_diff(
    path: String,
    rev: String,
    file: String,
) -> Result<Option<coral_core::diff::FileDiff>, crate::commands::IpcError> {
    let runner = coral_core::process::GitRunner::discover().await?;
    let loc =
        coral_core::repo::RepoLocation::discover(&runner, std::path::Path::new(&path)).await?;
    let files = loc.commit_diff(&runner, &rev, &[file.as_str()]).await?;
    Ok(files.into_iter().next())
}

/// One file's diff in the working tree, staged or not.
///
/// Separate from [`file_diff`] because it is a different question: that one asks what a commit
/// changed, this one what has changed since. A file can be in both answers with different
/// hunks, which is the whole reason the staging panel has two lists.
///
/// # Errors
/// Propagates git failures.
#[tauri::command]
pub async fn worktree_diff(
    path: String,
    staged: bool,
    file: String,
) -> Result<Option<coral_core::diff::FileDiff>, crate::commands::IpcError> {
    let runner = coral_core::process::GitRunner::discover().await?;
    let loc =
        coral_core::repo::RepoLocation::discover(&runner, std::path::Path::new(&path)).await?;
    let files = loc.diff(&runner, staged, &[file.as_str()]).await?;
    Ok(files.into_iter().next())
}

#[cfg(test)]
mod tests {
    use super::{GraphCache, KEEP};
    use coral_core::testutil::TestRepo;
    use std::sync::Arc;

    fn a_repo() -> TestRepo {
        TestRepo::new().write("a.txt", "1\n").commit("base")
    }

    /// Switching to another repository and back must not walk the first one again.
    ///
    /// It did, because there was one slot: opening the second evicted the first, and coming
    /// back to a tab meant waiting out a full walk of it. On a kernel-sized repository that is
    /// six seconds of an empty window, every time.
    #[tokio::test]
    async fn a_second_repository_does_not_evict_the_first() {
        let one = a_repo();
        let two = a_repo();
        let cache = GraphCache::default();

        let first = cache
            .store(&one.path().to_string_lossy(), false)
            .await
            .unwrap();
        let _ = cache
            .store(&two.path().to_string_lossy(), false)
            .await
            .unwrap();
        let again = cache
            .store(&one.path().to_string_lossy(), false)
            .await
            .unwrap();

        assert!(
            Arc::ptr_eq(&first, &again),
            "the first walk was thrown away"
        );
    }

    /// A held walk is eighty megabytes on a large repository, so they cannot all be kept.
    #[tokio::test]
    async fn the_least_recently_used_walk_is_the_one_that_goes() {
        let repos: Vec<TestRepo> = (0..=KEEP).map(|_| a_repo()).collect();
        let paths: Vec<String> = repos
            .iter()
            .map(|r| r.path().to_string_lossy().into_owned())
            .collect();
        let cache = GraphCache::default();

        let oldest = cache.store(&paths[0], false).await.unwrap();
        for path in &paths[1..] {
            let _ = cache.store(path, false).await.unwrap();
        }

        let after = cache.store(&paths[0], false).await.unwrap();
        assert!(!Arc::ptr_eq(&oldest, &after), "nothing was ever evicted");
        // The one asked for most recently is still there.
        let newest = cache.store(&paths[KEEP], false).await.unwrap();
        let newest_again = cache.store(&paths[KEEP], false).await.unwrap();
        assert!(Arc::ptr_eq(&newest, &newest_again));
    }

    /// A commit made in the terminal has to appear, so a repository that moved is walked again.
    #[tokio::test]
    async fn a_repository_that_moved_is_walked_again_and_its_neighbour_is_not() {
        let one = a_repo();
        let two = a_repo();
        let cache = GraphCache::default();
        let one_path = one.path().to_string_lossy().into_owned();
        let two_path = two.path().to_string_lossy().into_owned();

        cache.forget_if_moved(&one_path, 1).await;
        let held_one = cache.store(&one_path, false).await.unwrap();
        cache.forget_if_moved(&two_path, 1).await;
        let held_two = cache.store(&two_path, false).await.unwrap();

        assert!(cache.forget_if_moved(&one_path, 2).await, "refs moved");
        assert!(!Arc::ptr_eq(
            &held_one,
            &cache.store(&one_path, false).await.unwrap()
        ));
        assert!(Arc::ptr_eq(
            &held_two,
            &cache.store(&two_path, false).await.unwrap()
        ));
    }

    /// The rows painted in the first frame have to be nameable while the real walk runs.
    ///
    /// They were not: the metadata request asked for the topological store and waited the whole
    /// walk for it, so a large repository showed a column of dots with no messages beside them
    /// for six seconds — which is what the fast first paint exists to avoid.
    #[tokio::test]
    async fn the_quick_walk_answers_without_waiting_for_the_real_one() {
        let repo = a_repo();
        let path = repo.path().to_string_lossy().into_owned();
        let cache = GraphCache::default();

        let quick = cache.store(&path, true).await.unwrap();
        let slot = cache.slot(&path);
        // Held exactly as the topological walk holds it.
        let held = slot.held.lock().await;

        let again = cache.store(&path, true).await.unwrap();
        assert!(Arc::ptr_eq(&quick, &again));
        drop(held);
    }

    /// Once the real walk lands it is what the window shows, so it is what a row means.
    #[tokio::test]
    async fn the_real_walk_replaces_the_quick_one() {
        let repo = a_repo();
        let path = repo.path().to_string_lossy().into_owned();
        let cache = GraphCache::default();

        let quick = cache.store(&path, true).await.unwrap();
        let full = cache.store(&path, false).await.unwrap();
        assert!(!Arc::ptr_eq(&quick, &full));

        let asked = cache.store(&path, true).await.unwrap();
        assert!(Arc::ptr_eq(&full, &asked), "a row still meant the old walk");
    }

    /// Coming back to a tab nobody touched must cost nothing.
    #[tokio::test]
    async fn a_repository_that_did_not_move_keeps_its_walk() {
        let repo = a_repo();
        let path = repo.path().to_string_lossy().into_owned();
        let cache = GraphCache::default();

        cache.forget_if_moved(&path, 7).await;
        let first = cache.store(&path, false).await.unwrap();

        assert!(!cache.forget_if_moved(&path, 7).await, "nothing moved");
        assert!(Arc::ptr_eq(
            &first,
            &cache.store(&path, false).await.unwrap()
        ));
    }
}

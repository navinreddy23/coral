use std::sync::Arc;

use coral_core::CoralError;
use coral_core::graph::{GixCommitStream, RowStore, StreamOpts, build, wire};
use tokio::sync::Mutex;

/// The row store for one open repository, kept between frame requests.
///
/// Rebuilding the graph per frame would cost seconds; the store is built once and sliced.
#[derive(Default)]
pub struct GraphCache {
    inner: Mutex<Option<Cached>>,
}

struct Cached {
    path: String,
    /// True when this store came from a commit-time walk and is not topologically sound.
    provisional: bool,
    store: Arc<RowStore>,
}

impl GraphCache {
    /// Returns the store for `path`, building it if this is a different repository.
    /// Drops the walk held for `path`, if it is the one held.
    async fn forget(&self, path: &str) {
        let mut held = self.inner.lock().await;
        if held.as_ref().is_some_and(|cached| cached.path == path) {
            *held = None;
        }
    }

    async fn store(&self, path: &str, first_paint: bool) -> Result<Arc<RowStore>, CoralError> {
        let mut held = self.inner.lock().await;
        // Keying on the path alone would serve the fast provisional store back to the request
        // that wants topological rows, so the real walk would never run and the user would be
        // left on commit-time order for good.
        if let Some(cached) = held.as_ref()
            && cached.path == path
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
        *held = Some(Cached {
            path: path.to_owned(),
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
/// # Errors
/// Propagates git failures.
#[tauri::command]
pub async fn row_metadata(
    cache: tauri::State<'_, GraphCache>,
    path: String,
    start_row: u32,
    count: u32,
) -> Result<Vec<coral_core::commit::CommitMeta>, crate::commands::IpcError> {
    let store = cache.store(&path, false).await?;
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

/// Forgets the walk held for `path`, so the next frame walks the repository again.
///
/// The cache is one slot keyed by path, which made a rewalk of the repository already in it
/// impossible: a commit, a stash, a branch — none of them appeared until the user opened
/// another repository and came back, because that is what replaced the slot. Asking for a walk
/// has to mean a walk.
///
/// # Errors
/// Never; the signature is a `Result` because every command in this layer is one.
#[tauri::command]
pub async fn graph_rewalk(
    cache: tauri::State<'_, GraphCache>,
    path: String,
) -> Result<(), crate::commands::IpcError> {
    cache.forget(&path).await;
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

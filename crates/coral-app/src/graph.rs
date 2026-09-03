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
                StreamOpts::first_paint(u64::from(wire::ROWS_PER_FRAME))
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

/// A frame of known content, used once at startup to prove the binary path works.
///
/// Tauri's JavaScript falls back to `postMessage` permanently if the custom-protocol fetch
/// ever throws, and on that path a `Vec<u8>` is serialized as a JSON array of numbers — about
/// a hundred times slower, with no error anywhere. The UI checks this response's size and
/// contents at startup so the degradation is loud rather than silent.
#[tauri::command]
pub fn binary_self_test() -> tauri::ipc::Response {
    tracing::info!("binary_self_test");
    // A recognisable pattern, long enough that a JSON-array fallback is obvious.
    let bytes: Vec<u8> = (0..4096_u32)
        .map(|i| u8::try_from(i % 251).unwrap_or(0))
        .collect();
    tauri::ipc::Response::new(bytes)
}

//! A file's raw bytes on one side of a change, for the image view.

use coral_core::blob::BlobSide;

use crate::commands::IpcError;

/// The bytes of `file` on `side`, as a binary response: an image is not JSON, and an array of
/// numbers would be a hundred times its size.
///
/// An empty answer means the side has no such file — before an addition, after a deletion —
/// which costs nothing to tell apart, since no image is empty.
///
/// # Errors
/// Propagates git failures, and refuses a file too large to draw.
#[tauri::command]
pub async fn file_blob(
    path: String,
    side: BlobSide,
    file: String,
) -> Result<tauri::ipc::Response, IpcError> {
    let runner = coral_core::process::GitRunner::discover().await?;
    let loc =
        coral_core::repo::RepoLocation::discover(&runner, std::path::Path::new(&path)).await?;
    let bytes = loc.blob(&runner, &side, &file).await?.unwrap_or_default();
    Ok(tauri::ipc::Response::new(bytes))
}

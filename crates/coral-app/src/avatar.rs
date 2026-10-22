//! Authors' pictures for the window, fetched once and kept on disk.
//!
//! Each address is looked up once: a picture is kept for a week, and the answer that there is
//! none for a day, so a history of ten thousand authors is not ten thousand requests each time
//! it is opened. A request that could not be made is not remembered at all, so a window opened
//! offline asks again next time. The window decides whether to ask; the setting lives there.

// Tauri requires `State` by value in a command signature; it is a handle, not the data.
#![allow(clippy::needless_pass_by_value)]

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use coral_hosting::avatar::{Fetcher, key};
use tokio::sync::Semaphore;

use crate::commands::IpcError;

const KEEP_PICTURE: Duration = Duration::from_secs(7 * 24 * 60 * 60);
const KEEP_NONE: Duration = Duration::from_secs(24 * 60 * 60);
/// A screen of rows asks for every author on it at once; the host should see a handful.
const AT_ONCE: usize = 4;

pub struct Avatars {
    dir: PathBuf,
    gate: Semaphore,
}

impl Avatars {
    #[must_use]
    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            gate: Semaphore::new(AT_ONCE),
        }
    }

    /// The file a picture for `email` is kept in, and the one that says there is none. Named by
    /// a hash, so the cache does not hold a list of every address the user has looked at.
    fn files(&self, email: &str) -> (PathBuf, PathBuf) {
        let key = key(email);
        (
            self.dir.join(format!("{key}.img")),
            self.dir.join(format!("{key}.none")),
        )
    }

    async fn picture(&self, email: &str) -> Option<Vec<u8>> {
        let (img, none) = self.files(email);
        if fresh(&img, KEEP_PICTURE) {
            return std::fs::read(&img).ok();
        }
        if fresh(&none, KEEP_NONE) {
            return None;
        }
        let _turn = self.gate.acquire().await.ok()?;
        let fetched = Fetcher::new().ok()?.fetch(email).await;
        // The directory may not exist yet, and a cache that cannot be written is only slower.
        let _ = std::fs::create_dir_all(&self.dir);
        match fetched {
            Ok(Some(bytes)) => {
                let _ = std::fs::write(&img, &bytes);
                let _ = std::fs::remove_file(&none);
                Some(bytes)
            }
            Ok(None) => {
                let _ = std::fs::write(&none, b"");
                None
            }
            Err(e) => {
                tracing::debug!(error = %e, "could not fetch an avatar");
                None
            }
        }
    }
}

fn fresh(path: &PathBuf, keep: Duration) -> bool {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| SystemTime::now().duration_since(t).ok())
        .is_some_and(|age| age < keep)
}

/// The picture of whoever writes as `email`, as bytes; no bytes when there is none.
///
/// # Errors
/// Never in practice: every failure to find a picture is an empty answer, since initials are
/// always there to fall back on.
#[tauri::command]
pub async fn author_avatar(
    avatars: tauri::State<'_, Avatars>,
    email: String,
) -> Result<tauri::ipc::Response, IpcError> {
    Ok(tauri::ipc::Response::new(
        avatars.picture(&email).await.unwrap_or_default(),
    ))
}

#[cfg(test)]
mod tests {
    use super::{Avatars, KEEP_NONE};

    #[test]
    fn keeps_pictures_under_a_hash_of_the_address_not_the_address() {
        let avatars = Avatars::new(std::env::temp_dir().join("coral-avatars-test"));
        let (img, none) = avatars.files("  Ada@Example.org ");
        let name = img.file_name().unwrap().to_string_lossy().into_owned();
        assert!(!name.contains("example"), "{name}");
        assert_eq!(name.len(), 64 + ".img".len());
        // The same author however the address was written.
        assert_eq!(avatars.files("ada@example.org"), (img, none));
    }

    #[tokio::test]
    async fn remembers_that_there_is_no_picture_without_asking_again() {
        let dir = tempfile::tempdir().unwrap();
        let avatars = Avatars::new(dir.path().to_path_buf());
        let (_, none) = avatars.files("nobody@example.invalid");
        std::fs::write(&none, b"").unwrap();
        // Fresh, so nothing is fetched: a fetch here would fail the test's network-less run by
        // taking ten seconds to time out, not by returning.
        let started = std::time::Instant::now();
        assert_eq!(avatars.picture("nobody@example.invalid").await, None);
        assert!(started.elapsed() < std::time::Duration::from_secs(1));
        assert!(KEEP_NONE.as_secs() > 0);
    }

    #[tokio::test]
    async fn serves_a_kept_picture_from_disk() {
        let dir = tempfile::tempdir().unwrap();
        let avatars = Avatars::new(dir.path().to_path_buf());
        let (img, _) = avatars.files("ada@example.invalid");
        std::fs::write(&img, b"png bytes").unwrap();
        assert_eq!(
            avatars.picture("ada@example.invalid").await.as_deref(),
            Some(&b"png bytes"[..])
        );
    }
}

//! The repositories this user has opened, most recent first.
//!
//! Not the session: that holds what is open now, and a repository closed yesterday is exactly
//! the one the start page has to offer. Kept beside the session all the same, since both are
//! about which repositories this person works on.

// Tauri requires `State` by value in a command signature; it is a handle, not the data.
#![allow(clippy::needless_pass_by_value)]

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::commands::IpcError;

/// How many are kept. Enough to cover the repositories anyone moves between, short enough that
/// the list is read rather than searched.
const KEEP: usize = 40;

/// One repository, as the start page lists it.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Recent {
    pub path: String,
    /// The last path segment, which is what people call a repository.
    pub name: String,
    /// Seconds since the epoch, so the list can be ordered without storing the order.
    pub opened: i64,
}

/// The list, and where it is persisted.
pub struct Recents {
    inner: Mutex<Vec<Recent>>,
    path: PathBuf,
}

impl Recents {
    #[must_use]
    pub fn load(path: PathBuf) -> Self {
        let stored: Vec<Recent> = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Self {
            inner: Mutex::new(stored),
            path,
        }
    }

    #[must_use]
    pub fn read(&self) -> Vec<Recent> {
        self.held().clone()
    }

    /// Records that a repository was opened, moving it to the front.
    pub fn opened(&self, path: &str) {
        let mut held = self.held();
        *held = with(&held, path, now());
        if let Err(e) = save(&self.path, &held) {
            tracing::warn!(error = %e, path = %self.path.display(), "could not save the recents");
        }
    }

    /// Empties the list. The repositories themselves are not touched.
    pub fn clear(&self) {
        let mut held = self.held();
        held.clear();
        if let Err(e) = save(&self.path, &held) {
            tracing::warn!(error = %e, path = %self.path.display(), "could not save the recents");
        }
    }

    /// Takes one off the list. The repository itself is not touched.
    pub fn forget(&self, path: &str) {
        let mut held = self.held();
        held.retain(|r| r.path != path);
        if let Err(e) = save(&self.path, &held) {
            tracing::warn!(error = %e, path = %self.path.display(), "could not save the recents");
        }
    }

    fn held(&self) -> std::sync::MutexGuard<'_, Vec<Recent>> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// The list with `path` at the front, however many times it was already in it.
///
/// Separated from the store so the ordering rule can be tested without a file: opening a
/// repository again has to move it rather than add it, or the list fills with one name.
#[must_use]
pub fn with(existing: &[Recent], path: &str, opened: i64) -> Vec<Recent> {
    let mut out = Vec::with_capacity(existing.len() + 1);
    out.push(Recent {
        path: path.to_owned(),
        name: name_of(path),
        opened,
    });
    out.extend(existing.iter().filter(|r| r.path != path).cloned());
    out.truncate(KEEP);
    out
}

/// What to call a repository: the last segment of its path.
#[must_use]
pub fn name_of(path: &str) -> String {
    Path::new(path).file_name().map_or_else(
        || path.to_owned(),
        |name| name.to_string_lossy().into_owned(),
    )
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0))
}

fn save(path: &Path, list: &[Recent]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, serde_json::to_vec_pretty(list)?)
}

/// Every repository this user has opened, most recent first.
#[tauri::command]
#[must_use]
pub fn recent_repos(recents: tauri::State<'_, Recents>) -> Vec<Recent> {
    recents.read()
}

/// Takes one off the list.
///
/// # Errors
/// Never; the signature is a `Result` because every command in this layer is one.
#[tauri::command]
pub fn forget_recent(
    recents: tauri::State<'_, Recents>,
    path: String,
) -> Result<Vec<Recent>, IpcError> {
    recents.forget(&path);
    Ok(recents.read())
}

/// Empties the list, for someone who does not want their repositories named on this page.
///
/// # Errors
/// Never; the signature is a `Result` because every command in this layer is one.
#[tauri::command]
pub fn forget_all_recents(recents: tauri::State<'_, Recents>) -> Result<Vec<Recent>, IpcError> {
    recents.clear();
    Ok(recents.read())
}

#[cfg(test)]
mod tests {
    use super::{KEEP, Recent, name_of, with};

    fn listed(paths: &[&str]) -> Vec<Recent> {
        paths
            .iter()
            .enumerate()
            .map(|(i, p)| Recent {
                path: (*p).to_owned(),
                name: name_of(p),
                opened: i64::try_from(i).unwrap_or(0),
            })
            .collect()
    }

    #[test]
    fn opening_one_again_moves_it_rather_than_adding_it() {
        // Otherwise the list fills with the repository someone works in every day, and the one
        // they are looking for falls off the end.
        let start = listed(&["/a", "/b", "/c"]);
        let next = with(&start, "/c", 99);

        assert_eq!(
            next.iter().map(|r| r.path.as_str()).collect::<Vec<_>>(),
            ["/c", "/a", "/b"]
        );
        assert_eq!(next[0].opened, 99);
    }

    #[test]
    fn keeps_the_newest_and_drops_the_rest() {
        let many: Vec<String> = (0..KEEP + 10).map(|i| format!("/repo{i}")).collect();
        let refs: Vec<&str> = many.iter().map(String::as_str).collect();
        let list = with(&listed(&refs), "/fresh", 1);

        assert_eq!(list.len(), KEEP);
        assert_eq!(list[0].path, "/fresh");
        assert!(!list.iter().any(|r| r.path == format!("/repo{}", KEEP + 9)));
    }

    #[test]
    fn calls_a_repository_what_its_directory_is_called() {
        assert_eq!(name_of("/home/someone/Work/thing"), "thing");
        assert_eq!(name_of("/home/someone/Work/thing/"), "thing");
        // Nothing to take a name from, so the path is the name rather than an empty row.
        assert_eq!(name_of("/"), "/");
    }
}

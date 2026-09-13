//! The session as Tauri commands.
//!
//! Every mutation returns the whole session rather than nothing, so the tab bar cannot drift
//! from what is on disk: there is no separate reload to forget.

// Tauri requires `State` by value in a command signature; it is a handle, not the data.
#![allow(clippy::needless_pass_by_value)]

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::commands::IpcError;
use crate::session::Session;

/// The open session, and the file it came from.
///
/// The two are held under one lock because a profile switch changes both at once: it writes
/// what is open to where it came from and reads what is at the new path, and a window that
/// observed the halfway point would show one profile's tabs against another's file.
struct Held {
    session: Session,
    path: PathBuf,
}

/// The open session, and where it is persisted.
pub struct Tabs {
    inner: Mutex<Held>,
}

impl Tabs {
    /// Loads the session stored at `path`, or an empty one.
    #[must_use]
    pub fn load(path: PathBuf) -> Self {
        Self {
            inner: Mutex::new(Held {
                session: Session::load(&path),
                path,
            }),
        }
    }

    /// Puts this session away and takes out the one at `path`.
    ///
    /// The outgoing side is written first and unconditionally, so a crash immediately after a
    /// profile switch loses nothing that was open.
    pub fn switch_to(&self, path: PathBuf) -> Session {
        let mut held = self.held();
        save(&held.session, &held.path);
        held.session = Session::load(&path);
        held.path = path;
        held.session.clone()
    }

    /// Opens a repository, or focuses the tab that already has it.
    pub fn opened(&self, path: &str) -> Session {
        self.update(|s| {
            s.open(PathBuf::from(path));
        })
    }

    /// Applies a change and writes the result.
    ///
    /// A failed write is not worth refusing the change over — the tab is already open — so it
    /// is reported and the session continues in memory.
    fn update(&self, change: impl FnOnce(&mut Session)) -> Session {
        let mut held = self.held();
        change(&mut held.session);
        save(&held.session, &held.path);
        held.session.clone()
    }

    /// What is open.
    ///
    /// Two `exists` per tab before answering, so a repository that was moved or deleted while
    /// its tab was open is marked as gone rather than going on looking like one that is there.
    #[must_use]
    pub fn read(&self) -> Session {
        let mut held = self.held();
        held.session.mark_missing();
        held.session.clone()
    }

    fn held(&self) -> std::sync::MutexGuard<'_, Held> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

fn save(session: &Session, path: &Path) {
    if let Err(e) = session.save(path) {
        tracing::warn!(error = %e, path = %path.display(), "could not save the session");
    }
}

#[tauri::command]
#[must_use]
pub fn session_get(tabs: tauri::State<'_, Tabs>) -> Session {
    tabs.read()
}

/// Opens a repository in a tab, or focuses the tab that already has it.
///
/// # Errors
/// Never; async commands that borrow state have to answer with a `Result`.
#[tauri::command]
pub async fn tab_open(
    tabs: tauri::State<'_, Tabs>,
    recents: tauri::State<'_, crate::recent::Recents>,
    path: String,
) -> Result<Session, IpcError> {
    // A file chooser hands back whatever directory was open when the button was pressed, which
    // is often somewhere inside the repository rather than its root. Taken as given, that is a
    // tab named for a subdirectory showing the whole repository, and a second tab for the same
    // repository as soon as it is opened again by its root.
    let Some(root) = root_of(&path).await else {
        // No repository there. The tab is still made, because it is what carries the reason on
        // screen, but nothing goes on the list of repositories to open again.
        return Ok(tabs.opened(&path));
    };
    // Recorded here rather than wherever a repository is read, because this is the moment the
    // user chose one. Reloading after a commit is not choosing, and neither is stepping into a
    // submodule.
    recents.opened(&root);
    Ok(tabs.opened(&root))
}

/// The root of the repository `path` is in, however far inside it the path points.
///
/// `None` when there is no repository above it, which is how a tab that can only report a
/// failure is told apart from one worth remembering.
pub async fn root_of(path: &str) -> Option<String> {
    let runner = coral_core::process::GitRunner::discover().await.ok()?;
    let loc = coral_core::repo::RepoLocation::discover(&runner, Path::new(path))
        .await
        .ok()?;
    Some(loc.display_path().to_string_lossy().into_owned())
}

#[tauri::command]
#[must_use]
pub fn tab_close(tabs: tauri::State<'_, Tabs>, id: u32) -> Session {
    tabs.update(|s| s.close(id))
}

#[tauri::command]
#[must_use]
pub fn tab_activate(
    tabs: tauri::State<'_, Tabs>,
    recents: tauri::State<'_, crate::recent::Recents>,
    id: u32,
) -> Session {
    let session = tabs.update(|s| s.activate(id));
    if let Some(tab) = session.tabs.iter().find(|t| t.id == id) {
        recents.opened(&tab.path.display().to_string());
    }
    session
}

#[tauri::command]
#[must_use]
pub fn tab_group(tabs: tauri::State<'_, Tabs>, name: String, ids: Vec<u32>) -> Session {
    tabs.update(|s| {
        s.group(name, &ids);
    })
}

/// Moves a tab: into a group, out of one, or in front of another.
///
/// One command rather than an ungroup followed by a group, because the two would rearrange the
/// bar twice and a group emptied in between would be collected before the tab arrived.
#[tauri::command]
#[must_use]
pub fn tab_move(
    tabs: tauri::State<'_, Tabs>,
    id: u32,
    group: Option<u32>,
    before: Option<u32>,
) -> Session {
    tabs.update(|s| s.move_tab(id, group, before))
}

#[tauri::command]
#[must_use]
pub fn tab_ungroup(tabs: tauri::State<'_, Tabs>, id: u32) -> Session {
    tabs.update(|s| s.ungroup(id))
}

#[tauri::command]
#[must_use]
pub fn group_collapse(tabs: tauri::State<'_, Tabs>, id: u32, collapsed: bool) -> Session {
    tabs.update(|s| s.set_collapsed(id, collapsed))
}

/// Shows a submodule inside the tab that declares it.
///
/// Not a tab of its own: a submodule belongs to the repository that declares it, and the
/// reference shows it as another step in the same tab's breadcrumb.
#[tauri::command]
#[must_use]
pub fn tab_enter_submodule(tabs: tauri::State<'_, Tabs>, id: u32, path: String) -> Session {
    tabs.update(|s| s.enter_submodule(id, PathBuf::from(path)))
}

#[tauri::command]
#[must_use]
pub fn tab_leave_submodule(tabs: tauri::State<'_, Tabs>, id: u32) -> Session {
    tabs.update(|s| s.leave_submodule(id))
}

/// Sets the picture on a tab, or puts it back to the default with `None`.
#[tauri::command]
#[must_use]
pub fn tab_icon(
    tabs: tauri::State<'_, Tabs>,
    id: u32,
    icon: Option<crate::session::TabIcon>,
) -> Session {
    tabs.update(|s| s.set_icon(id, icon))
}

#[tauri::command]
#[must_use]
pub fn group_rename(tabs: tauri::State<'_, Tabs>, id: u32, name: String) -> Session {
    tabs.update(|s| s.rename_group(id, name))
}

#[tauri::command]
#[must_use]
pub fn group_recolour(
    tabs: tauri::State<'_, Tabs>,
    id: u32,
    colour: crate::session::GroupColour,
) -> Session {
    tabs.update(|s| s.recolour_group(id, colour))
}

/// Dissolves a group, leaving its tabs open.
#[tauri::command]
#[must_use]
pub fn group_dissolve(tabs: tauri::State<'_, Tabs>, id: u32) -> Session {
    tabs.update(|s| s.dissolve_group(id))
}

/// Closes every tab in a group.
#[tauri::command]
#[must_use]
pub fn group_close(tabs: tauri::State<'_, Tabs>, id: u32) -> Session {
    tabs.update(|s| s.close_group(id))
}

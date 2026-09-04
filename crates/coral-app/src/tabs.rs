//! The session as Tauri commands.
//!
//! Every mutation returns the whole session rather than nothing, so the tab bar cannot drift
//! from what is on disk: there is no separate reload to forget.

// Tauri requires `State` by value in a command signature; it is a handle, not the data.
#![allow(clippy::needless_pass_by_value)]

use std::path::PathBuf;
use std::sync::Mutex;

use crate::session::Session;

/// The open session, and where it is persisted.
pub struct Tabs {
    inner: Mutex<Session>,
    path: PathBuf,
}

impl Tabs {
    /// Loads the session stored at `path`, or an empty one.
    #[must_use]
    pub fn load(path: PathBuf) -> Self {
        Self {
            inner: Mutex::new(Session::load(&path)),
            path,
        }
    }

    /// Applies a change and writes the result.
    ///
    /// A failed write is not worth refusing the change over — the tab is already open — so it
    /// is reported and the session continues in memory.
    fn update(&self, change: impl FnOnce(&mut Session)) -> Session {
        let mut held = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        change(&mut held);
        if let Err(e) = held.save(&self.path) {
            tracing::warn!(error = %e, path = %self.path.display(), "could not save the session");
        }
        held.clone()
    }

    fn read(&self) -> Session {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

#[tauri::command]
#[must_use]
pub fn session_get(tabs: tauri::State<'_, Tabs>) -> Session {
    tabs.read()
}

#[tauri::command]
#[must_use]
pub fn tab_open(tabs: tauri::State<'_, Tabs>, path: String) -> Session {
    tabs.update(|s| {
        s.open(PathBuf::from(path));
    })
}

#[tauri::command]
#[must_use]
pub fn tab_close(tabs: tauri::State<'_, Tabs>, id: u32) -> Session {
    tabs.update(|s| s.close(id))
}

#[tauri::command]
#[must_use]
pub fn tab_activate(tabs: tauri::State<'_, Tabs>, id: u32) -> Session {
    tabs.update(|s| s.activate(id))
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

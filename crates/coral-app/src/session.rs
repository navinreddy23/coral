//! Open repositories, arranged as tabs and groups, persisted across launches.
//!
//! This lives in the app, not the engine: `coral-core` knows about repositories, not about
//! how a window chooses to show them.

use std::path::{Path, PathBuf};

/// One of the eight lane colours, named rather than stored as a hex value so a theme change
/// recolours existing groups instead of stranding them on last year's palette.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GroupColour {
    Lane1,
    Lane2,
    Lane3,
    Lane4,
    Lane5,
    Lane6,
    Lane7,
    Lane8,
}

impl GroupColour {
    /// The CSS custom property this colour reads from.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Lane1 => "--lane-1",
            Self::Lane2 => "--lane-2",
            Self::Lane3 => "--lane-3",
            Self::Lane4 => "--lane-4",
            Self::Lane5 => "--lane-5",
            Self::Lane6 => "--lane-6",
            Self::Lane7 => "--lane-7",
            Self::Lane8 => "--lane-8",
        }
    }

    /// Cycles through the palette so consecutive new groups are visibly different.
    #[must_use]
    pub const fn nth(n: usize) -> Self {
        match n % 8 {
            0 => Self::Lane1,
            1 => Self::Lane2,
            2 => Self::Lane3,
            3 => Self::Lane4,
            4 => Self::Lane5,
            5 => Self::Lane6,
            6 => Self::Lane7,
            _ => Self::Lane8,
        }
    }
}

/// The picture on a tab.
///
/// A fixed set rather than a file the user points at. A tab shows fourteen pixels of picture:
/// an arbitrary image is a smudge at that size, and a path into somebody's home directory is
/// one more thing that breaks when they tidy up. These are drawn in the window, so they take
/// the theme with them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TabIcon {
    Branch,
    Github,
    Gitlab,
    Package,
    Terminal,
    Globe,
    Book,
    Beaker,
    Wrench,
    Star,
    Bug,
    Rocket,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TabGroup {
    pub id: u32,
    pub name: String,
    pub colour: GroupColour,
    pub collapsed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tab {
    pub id: u32,
    pub path: PathBuf,
    /// A submodule being looked at inside this tab, relative to `path`.
    ///
    /// A submodule is not a separate workspace: it belongs to the repository that declares it,
    /// and the reference shows it as a step in the same tab's breadcrumb rather than opening a
    /// tab of its own. Keeping it on the tab is what lets the crumb survive a restart.
    #[serde(default)]
    pub submodule: Option<PathBuf>,
    /// The group this tab belongs to, if any.
    pub group: Option<u32>,
    /// Whether the repository is still on disk. A tab for a missing one is kept and marked,
    /// not dropped: a disconnected drive should not silently lose someone's workspace.
    #[serde(default)]
    pub missing: bool,
    /// The picture the user chose for this tab. `None` leaves the window to its default, which
    /// is not the same as choosing the default: a later change to what that is should reach a
    /// tab nobody has had an opinion about.
    #[serde(default)]
    pub icon: Option<TabIcon>,
}

/// Everything the window restores on launch.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub tabs: Vec<Tab>,
    pub groups: Vec<TabGroup>,
    pub active: Option<u32>,
    #[serde(default)]
    next_id: u32,
}

impl Session {
    /// Reads the session, treating anything unreadable or malformed as empty.
    ///
    /// A corrupt session file must never stop the application from opening.
    #[must_use]
    pub fn load(path: &Path) -> Self {
        let mut session: Self = std::fs::read(path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        session.mark_missing();
        session.repair();
        session
    }

    /// Writes the session.
    ///
    /// # Errors
    /// [`std::io::Error`] if the file cannot be written.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let json =
            serde_json::to_vec_pretty(self).map_err(|e| std::io::Error::other(e.to_string()))?;
        std::fs::write(path, json)
    }

    /// Opens `path`, or focuses the tab that already has it.
    ///
    /// Asking for a repository that is already open in a tab currently showing one of its
    /// submodules steps back out to it, which is what the request means.
    pub fn open(&mut self, path: PathBuf) -> u32 {
        if let Some(existing) = self.tabs.iter_mut().find(|t| t.path == path) {
            existing.submodule = None;
            let id = existing.id;
            self.active = Some(id);
            return id;
        }
        let id = self.take_id();
        self.tabs.push(Tab {
            id,
            path,
            submodule: None,
            group: None,
            missing: false,
            icon: None,
        });
        self.active = Some(id);
        id
    }

    /// Closes a tab, moving focus to a neighbour rather than to nothing.
    pub fn close(&mut self, id: u32) {
        let Some(at) = self.tabs.iter().position(|t| t.id == id) else {
            return;
        };
        self.tabs.remove(at);

        if self.active == Some(id) {
            let next = at.min(self.tabs.len().saturating_sub(1));
            self.active = self.tabs.get(next).map(|t| t.id);
        }
        self.drop_empty_groups();
    }

    /// Creates a group holding `tabs`, in the order they already sit in.
    pub fn group(&mut self, name: String, members: &[u32]) -> u32 {
        let id = self.take_id();
        let colour = GroupColour::nth(self.groups.len());
        self.groups.push(TabGroup {
            id,
            name,
            colour,
            collapsed: false,
        });

        for tab in self.tabs.iter_mut().filter(|t| members.contains(&t.id)) {
            tab.group = Some(id);
        }
        self.regroup();
        id
    }

    /// Moves a tab into a group, out of one, or to a new position.
    ///
    /// `group` is where it lands: `Some` to join that group, `None` to sit loose. `before` is
    /// the tab it should end up in front of, which is how a drop between two tabs is
    /// expressed; `None` puts it last in its band.
    ///
    /// This is what dragging does, and it is one operation rather than an ungroup followed by
    /// a group: the two would leave the bar rearranged twice, and a group emptied in between
    /// would be collected before the tab arrived.
    pub fn move_tab(&mut self, tab: u32, group: Option<u32>, before: Option<u32>) {
        // Dropping a tab on itself asks for nothing. Taking it out and putting it back would
        // land it after its own former neighbour instead of where it started.
        if before == Some(tab) {
            return;
        }
        // A group that no longer exists would strand the tab in a band nothing draws.
        let group = group.filter(|id| self.groups.iter().any(|g| g.id == *id));
        let Some(at) = self.tabs.iter().position(|t| t.id == tab) else {
            return;
        };
        let mut moved = self.tabs.remove(at);
        moved.group = group;

        let insert = before
            .and_then(|id| self.tabs.iter().position(|t| t.id == id))
            // Otherwise after the last tab of the group it is joining, so it lands beside its
            // new neighbours rather than at the end of the whole bar.
            .or_else(|| {
                group.and_then(|id| {
                    self.tabs
                        .iter()
                        .rposition(|t| t.group == Some(id))
                        .map(|i| i + 1)
                })
            })
            .unwrap_or(self.tabs.len());
        self.tabs.insert(insert.min(self.tabs.len()), moved);

        self.regroup();
        self.drop_empty_groups();
    }

    /// Shows a submodule inside the tab that declares it.
    ///
    /// `relative` is the submodule's path within the repository, which is how `.gitmodules`
    /// names it. Nothing is validated here: whether the working copy is initialised is the
    /// engine's answer, not the session's.
    pub fn enter_submodule(&mut self, tab: u32, relative: PathBuf) {
        if let Some(t) = self.tabs.iter_mut().find(|t| t.id == tab) {
            t.submodule = Some(relative);
        }
    }

    /// Steps back out to the repository the tab was opened for.
    pub fn leave_submodule(&mut self, tab: u32) {
        if let Some(t) = self.tabs.iter_mut().find(|t| t.id == tab) {
            t.submodule = None;
        }
    }

    /// Sets the picture on a tab. `None` puts it back to the window's default.
    pub fn set_icon(&mut self, tab: u32, icon: Option<TabIcon>) {
        if let Some(t) = self.tabs.iter_mut().find(|t| t.id == tab) {
            t.icon = icon;
        }
    }

    /// Renames a group.
    pub fn rename_group(&mut self, group: u32, name: String) {
        if let Some(g) = self.groups.iter_mut().find(|g| g.id == group) {
            g.name = name;
        }
    }

    /// Changes a group's colour.
    pub fn recolour_group(&mut self, group: u32, colour: GroupColour) {
        if let Some(g) = self.groups.iter_mut().find(|g| g.id == group) {
            g.colour = colour;
        }
    }

    /// Dissolves a group, leaving its tabs open and loose.
    pub fn dissolve_group(&mut self, group: u32) {
        for tab in self.tabs.iter_mut().filter(|t| t.group == Some(group)) {
            tab.group = None;
        }
        self.drop_empty_groups();
        self.regroup();
    }

    /// Closes every tab in a group, and the group with them.
    pub fn close_group(&mut self, group: u32) {
        let doomed: Vec<u32> = self
            .tabs
            .iter()
            .filter(|t| t.group == Some(group))
            .map(|t| t.id)
            .collect();
        for id in doomed {
            self.close(id);
        }
    }

    /// Removes a tab from its group without closing it.
    pub fn ungroup(&mut self, tab: u32) {
        if let Some(t) = self.tabs.iter_mut().find(|t| t.id == tab) {
            t.group = None;
        }
        self.drop_empty_groups();
    }

    pub fn set_collapsed(&mut self, group: u32, collapsed: bool) {
        if let Some(g) = self.groups.iter_mut().find(|g| g.id == group) {
            g.collapsed = collapsed;
        }
    }

    /// Focuses a tab, expanding its group if it was collapsed.
    pub fn activate(&mut self, id: u32) {
        let Some(tab) = self.tabs.iter().find(|t| t.id == id) else {
            return;
        };
        let group = tab.group;
        self.active = Some(id);
        if let Some(g) = group {
            self.set_collapsed(g, false);
        }
    }

    fn take_id(&mut self) -> u32 {
        // Ids are never reused, so a stale reference cannot silently point at a new tab.
        self.next_id = self.next_id.wrapping_add(1);
        self.next_id
    }

    /// The directory a tab is currently showing: the submodule when one is open, else the
    /// repository itself.
    #[must_use]
    pub fn working_path(tab: &Tab) -> PathBuf {
        match &tab.submodule {
            Some(rel) => tab.path.join(rel),
            None => tab.path.clone(),
        }
    }

    /// Marks tabs whose repository is no longer on disk.
    fn mark_missing(&mut self) {
        for tab in &mut self.tabs {
            tab.missing = !tab.path.join(".git").exists() && !tab.path.join("HEAD").exists();
        }
    }

    /// Restores invariants a hand-edited or truncated file might have broken.
    fn repair(&mut self) {
        let ids: Vec<u32> = self.groups.iter().map(|g| g.id).collect();
        for tab in &mut self.tabs {
            if tab.group.is_some_and(|g| !ids.contains(&g)) {
                tab.group = None;
            }
        }
        self.next_id = self
            .next_id
            .max(self.tabs.iter().map(|t| t.id).max().unwrap_or(0))
            .max(self.groups.iter().map(|g| g.id).max().unwrap_or(0));

        if self
            .active
            .is_some_and(|a| !self.tabs.iter().any(|t| t.id == a))
        {
            self.active = self.tabs.first().map(|t| t.id);
        }
        self.drop_empty_groups();
        self.regroup();
    }

    fn drop_empty_groups(&mut self) {
        let used: Vec<u32> = self.tabs.iter().filter_map(|t| t.group).collect();
        self.groups.retain(|g| used.contains(&g.id));
    }

    /// Keeps each group's tabs contiguous, which is what makes a group drawable as one band.
    fn regroup(&mut self) {
        let order: Vec<Option<u32>> = {
            let mut seen: Vec<Option<u32>> = Vec::new();
            for tab in &self.tabs {
                if !seen.contains(&tab.group) {
                    seen.push(tab.group);
                }
            }
            seen
        };
        let mut sorted: Vec<Tab> = Vec::with_capacity(self.tabs.len());
        for key in order {
            sorted.extend(self.tabs.iter().filter(|t| t.group == key).cloned());
        }
        self.tabs = sorted;
    }
}

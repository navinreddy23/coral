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
    /// The group this tab belongs to, if any.
    pub group: Option<u32>,
    /// Whether the repository is still on disk. A tab for a missing one is kept and marked,
    /// not dropped: a disconnected drive should not silently lose someone's workspace.
    #[serde(default)]
    pub missing: bool,
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
    pub fn open(&mut self, path: PathBuf) -> u32 {
        if let Some(existing) = self.tabs.iter().find(|t| t.path == path) {
            self.active = Some(existing.id);
            return existing.id;
        }
        let id = self.take_id();
        self.tabs.push(Tab {
            id,
            path,
            group: None,
            missing: false,
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

//! Who is at the window, and the workspace that belongs to them.
//!
//! A profile is two things at once: the tabs, groups and recent repositories somebody works
//! with, and the identity the repositories they clone should carry. Work and personal are the
//! case it exists for — one machine, two sets of repositories, two email addresses, and no
//! wish to see either while doing the other.
//!
//! The workspace half is a directory. `session.json` and `recent.json` move under
//! `profiles/<id>/`, which is why switching is a matter of pointing [`crate::tabs::Tabs`] and
//! [`crate::recent::Recents`] at another file rather than of teaching either about profiles.
//! The identity half is written into each repository's own git config, never into a private
//! store git would ignore; `crates/coral-core/src/identity.rs` does that part.

// Tauri requires `State` by value in a command signature; it is a handle, not the data.
#![allow(clippy::needless_pass_by_value)]

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use coral_core::identity::{Identity, IdentityScopes};
use coral_core::signing::SigningOverrides;
use coral_core::ssh::SshOverrides;

use crate::commands::IpcError;
use crate::session::{GroupColour, Session};

/// What a profile hands to a repository that joins it.
///
/// The three shapes are the engine's own per-repository ones, so applying a profile is the
/// same write the settings screen already makes. `None` throughout means the profile says
/// nothing and the repository keeps whatever it had.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileSettings {
    #[serde(default)]
    pub user: Identity,
    #[serde(default)]
    pub ssh: SshOverrides,
    #[serde(default)]
    pub signing: SigningOverrides,
}

/// One profile.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    /// The name of its directory, derived from the name and never containing a path.
    pub id: String,
    pub name: String,
    /// One of the eight lane colours, so a profile is as scannable as a tab group and no new
    /// palette has to be maintained.
    pub colour: GroupColour,
    #[serde(default)]
    pub settings: ProfileSettings,
}

/// Every profile, and which one is in use.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Registry {
    pub profiles: Vec<Profile>,
    pub current: String,
}

impl Default for Registry {
    fn default() -> Self {
        let first = Profile {
            id: "personal".to_owned(),
            name: "Personal".to_owned(),
            colour: GroupColour::Lane1,
            settings: ProfileSettings::default(),
        };
        Self {
            current: first.id.clone(),
            profiles: vec![first],
        }
    }
}

impl Registry {
    /// The profile in use, which is always one that exists.
    #[must_use]
    pub fn current(&self) -> &Profile {
        self.profiles
            .iter()
            .find(|p| p.id == self.current)
            .unwrap_or(&self.profiles[0])
    }

    /// Repairs a registry that has been hand-edited or half-written.
    ///
    /// An empty list or a `current` naming nothing would leave a window with no workspace to
    /// put a tab in, so both are corrected rather than refused.
    fn repair(&mut self) {
        if self.profiles.is_empty() {
            *self = Self::default();
        }
        if !self.profiles.iter().any(|p| p.id == self.current) {
            let first = self.profiles[0].id.clone();
            self.current = first;
        }
    }
}

/// The profiles, and the directory their workspaces live under.
pub struct Profiles {
    inner: Mutex<Registry>,
    dir: PathBuf,
}

impl Profiles {
    /// Reads the registry under `dir`, creating the first profile if there is none.
    #[must_use]
    pub fn load(dir: PathBuf) -> Self {
        let mut registry: Registry = std::fs::read(dir.join("profiles.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        registry.repair();

        let profiles = Self {
            dir,
            inner: Mutex::new(registry),
        };
        profiles.adopt_the_workspace_from_before_profiles();
        profiles.save();
        profiles
    }

    /// The whole registry, which is what every command answers with.
    #[must_use]
    pub fn read(&self) -> Registry {
        self.held().clone()
    }

    /// Where a profile's open tabs are kept.
    #[must_use]
    pub fn session_path(&self, id: &str) -> PathBuf {
        self.dir_for(id).join("session.json")
    }

    /// Where a profile's recently opened repositories are kept.
    #[must_use]
    pub fn recent_path(&self, id: &str) -> PathBuf {
        self.dir_for(id).join("recent.json")
    }

    /// Adds a profile and answers with its id. It does not become current.
    pub fn create(&self, name: &str, colour: GroupColour) -> String {
        let mut held = self.held();
        let id = unused(slug(name), &held.profiles);
        held.profiles.push(Profile {
            id: id.clone(),
            name: displayable(name),
            colour,
            settings: ProfileSettings::default(),
        });
        drop(held);
        self.save();
        id
    }

    /// Makes `id` the current profile, if it names one.
    pub fn switch(&self, id: &str) -> Registry {
        let mut held = self.held();
        if held.profiles.iter().any(|p| p.id == id) {
            held.current.clear();
            held.current.push_str(id);
        }
        let answer = held.clone();
        drop(held);
        self.save();
        answer
    }

    pub fn rename(&self, id: &str, name: &str) -> Registry {
        self.change(id, |p| p.name = displayable(name))
    }

    pub fn recolour(&self, id: &str, colour: GroupColour) -> Registry {
        self.change(id, |p| p.colour = colour)
    }

    /// Sets what the profile hands to repositories that join it.
    pub fn set_settings(&self, id: &str, settings: ProfileSettings) -> Registry {
        self.change(id, |p| p.settings = settings.clone())
    }

    /// Sets the name and address alone, leaving the keys as they were.
    pub fn set_identity(&self, id: &str, user: Identity) -> Registry {
        self.change(id, |p| p.settings.user = user.clone())
    }

    /// Removes a profile and everything it kept, answering whether it went.
    ///
    /// The last one never goes: there would then be nowhere to put the next tab. Its
    /// repositories are never touched — only the record of which were open.
    pub fn delete(&self, id: &str) -> bool {
        let mut held = self.held();
        if held.profiles.len() < 2 || !held.profiles.iter().any(|p| p.id == id) {
            return false;
        }
        held.profiles.retain(|p| p.id != id);
        if held.current == id {
            let first = held.profiles[0].id.clone();
            held.current = first;
        }
        drop(held);

        if let Err(e) = std::fs::remove_dir_all(self.dir_for(id)) {
            // Nothing points at it any more, so a directory left behind is untidy rather than
            // wrong, and refusing the deletion over it would be worse.
            if e.kind() != std::io::ErrorKind::NotFound {
                tracing::warn!(error = %e, id, "removed the profile but not its workspace");
            }
        }
        self.save();
        true
    }

    fn dir_for(&self, id: &str) -> PathBuf {
        self.dir.join("profiles").join(id)
    }

    fn change(&self, id: &str, edit: impl Fn(&mut Profile)) -> Registry {
        let mut held = self.held();
        if let Some(profile) = held.profiles.iter_mut().find(|p| p.id == id) {
            edit(profile);
        }
        let answer = held.clone();
        drop(held);
        self.save();
        answer
    }

    /// Moves a pre-profiles `session.json` and `recent.json` into the first profile.
    ///
    /// Somebody upgrading has tabs open, and a feature whose whole purpose is to keep tabs
    /// must not lose them on the launch that introduces it. Moved rather than copied, or the
    /// next launch would read the old file and adopt it a second time over whatever has
    /// happened since.
    fn adopt_the_workspace_from_before_profiles(&self) {
        let first = self.held().profiles[0].id.clone();
        for name in ["session.json", "recent.json"] {
            let from = self.dir.join(name);
            let to = self.dir_for(&first).join(name);
            if !from.exists() || to.exists() {
                continue;
            }
            if let Some(parent) = to.parent()
                && let Err(e) = std::fs::create_dir_all(parent)
            {
                tracing::warn!(error = %e, "could not make the first profile's directory");
                continue;
            }
            if std::fs::rename(&from, &to).is_ok() {
                continue;
            }
            // A rename can fail across a mount; a copy that leaves the original is still
            // better than a launch that opens on nothing.
            match std::fs::copy(&from, &to).and_then(|_| std::fs::remove_file(&from)) {
                Ok(()) => {}
                Err(e) => tracing::warn!(error = %e, name, "could not adopt the old workspace"),
            }
        }
    }

    fn save(&self) {
        let held = self.held();
        if let Err(e) = write(&self.dir, &held) {
            tracing::warn!(error = %e, dir = %self.dir.display(), "could not save the profiles");
        }
    }

    fn held(&self) -> std::sync::MutexGuard<'_, Registry> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

fn write(dir: &Path, registry: &Registry) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    std::fs::write(
        dir.join("profiles.json"),
        serde_json::to_vec_pretty(registry)?,
    )
}

/// The directory name for a profile called `name`.
///
/// Lowercase letters, digits and hyphens, and nothing else. The id names a directory under the
/// application's config directory, so a name somebody types must not be able to decide where
/// that directory is: dots and separators go, and a name that leaves nothing becomes
/// `profile`.
#[must_use]
pub fn slug(name: &str) -> String {
    let mut out = String::new();
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.extend(ch.to_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        "profile".to_owned()
    } else {
        trimmed.to_owned()
    }
}

/// The same slug with a number on it when something already has it.
///
/// Bounded by the number of profiles plus one, so there is always a free candidate in the
/// range and the search cannot run on.
fn unused(base: String, existing: &[Profile]) -> String {
    if !existing.iter().any(|p| p.id == base) {
        return base;
    }
    (2..=existing.len() + 2)
        .map(|n| format!("{base}-{n}"))
        .find(|candidate| !existing.iter().any(|p| &p.id == candidate))
        .unwrap_or(base)
}

/// A name as it will be shown: trimmed, and never empty.
fn displayable(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        "Profile".to_owned()
    } else {
        trimmed.to_owned()
    }
}

/// What one profile's workspace is, after a switch.
///
/// The session and the recents come back with the registry rather than being fetched
/// afterwards, for the reason `tabs.rs` gives about the session: the window cannot then be
/// showing one profile's tabs while another is current.
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Switched {
    pub registry: Registry,
    pub session: Session,
    pub recents: Vec<crate::recent::Recent>,
}

/// Points the workspace at whichever profile is current.
fn workspace(
    profiles: &Profiles,
    tabs: &crate::tabs::Tabs,
    recents: &crate::recent::Recents,
    registry: Registry,
) -> Switched {
    let id = registry.current.clone();
    Switched {
        session: tabs.switch_to(profiles.session_path(&id)),
        recents: recents.switch_to(profiles.recent_path(&id)),
        registry,
    }
}

#[tauri::command]
#[must_use]
pub fn profile_list(profiles: tauri::State<'_, Profiles>) -> Registry {
    profiles.read()
}

#[tauri::command]
#[must_use]
pub fn profile_create(
    profiles: tauri::State<'_, Profiles>,
    name: String,
    colour: GroupColour,
) -> Registry {
    profiles.create(&name, colour);
    profiles.read()
}

#[tauri::command]
#[must_use]
pub fn profile_rename(profiles: tauri::State<'_, Profiles>, id: String, name: String) -> Registry {
    profiles.rename(&id, &name)
}

#[tauri::command]
#[must_use]
pub fn profile_recolour(
    profiles: tauri::State<'_, Profiles>,
    id: String,
    colour: GroupColour,
) -> Registry {
    profiles.recolour(&id, colour)
}

#[tauri::command]
#[must_use]
pub fn profile_set_settings(
    profiles: tauri::State<'_, Profiles>,
    id: String,
    settings: ProfileSettings,
) -> Registry {
    profiles.set_settings(&id, settings)
}

/// Switches profile, answering with the workspace that comes with it.
#[tauri::command]
#[must_use]
pub fn profile_switch(
    profiles: tauri::State<'_, Profiles>,
    tabs: tauri::State<'_, crate::tabs::Tabs>,
    recents: tauri::State<'_, crate::recent::Recents>,
    id: String,
) -> Switched {
    let registry = profiles.switch(&id);
    workspace(&profiles, &tabs, &recents, registry)
}

/// Removes a profile, answering with whatever workspace is current afterwards.
#[tauri::command]
#[must_use]
pub fn profile_delete(
    profiles: tauri::State<'_, Profiles>,
    tabs: tauri::State<'_, crate::tabs::Tabs>,
    recents: tauri::State<'_, crate::recent::Recents>,
    id: String,
) -> Switched {
    let was = profiles.read().current;
    profiles.delete(&id);
    let registry = profiles.read();
    if registry.current == was {
        // Nothing moved, so the workspace must not be reloaded underneath the window.
        return Switched {
            session: tabs.read(),
            recents: recents.read(),
            registry,
        };
    }
    workspace(&profiles, &tabs, &recents, registry)
}

/// What a repository records as its author, at every level.
///
/// # Errors
/// Propagates git failures.
#[tauri::command]
pub async fn repo_identity(path: String) -> Result<IdentityScopes, IpcError> {
    let (runner, loc) = located(&path).await?;
    Ok(loc
        .identity_scopes(&runner, coral_core::config::AppConfig::default())
        .await?)
}

/// Writes the current profile's settings into this repository.
///
/// Only what the profile actually sets is written, so a profile with an email and no signing
/// key leaves the repository's signing alone rather than clearing it.
///
/// # Errors
/// Propagates git failures.
#[tauri::command]
pub async fn profile_apply_here(
    profiles: tauri::State<'_, Profiles>,
    path: String,
) -> Result<IdentityScopes, IpcError> {
    let settings = profiles.read().current().settings.clone();
    let (runner, loc) = located(&path).await?;

    if !settings.user.is_empty() {
        loc.set_identity_local(&runner, &settings.user).await?;
    }
    if !settings.ssh.is_empty() {
        loc.set_ssh_local(&runner, &settings.ssh).await?;
    }
    if !settings.signing.is_empty() {
        loc.set_signing_local(
            &runner,
            &settings.signing,
            signing_format(&runner, &loc, &settings).await,
        )
        .await?;
    }
    Ok(loc
        .identity_scopes(&runner, coral_core::config::AppConfig::default())
        .await?)
}

/// Stamps a repository that has just been made, where there is nothing to overwrite.
pub async fn stamp_new_repository(settings: &ProfileSettings, path: &Path) {
    let Ok(runner) = coral_core::process::GitRunner::discover().await else {
        return;
    };
    let Ok(loc) = coral_core::repo::RepoLocation::discover(&runner, path).await else {
        return;
    };
    // Reported and stepped over rather than propagated: the repository exists and is correct,
    // and failing the whole operation over a config write would throw away a good clone.
    if !settings.user.is_empty()
        && let Err(e) = loc.set_identity_local(&runner, &settings.user).await
    {
        tracing::warn!(error = %e, "could not record the profile's identity");
    }
    if !settings.signing.is_empty() {
        let format = signing_format(&runner, &loc, settings).await;
        if let Err(e) = loc
            .set_signing_local(&runner, &settings.signing, format)
            .await
        {
            tracing::warn!(error = %e, "could not record the profile's signing settings");
        }
    }
}

/// Which format the signing program is filed under.
///
/// git stores `gpg.<format>.program`, so a write has to know the format even when the profile
/// does not set one; the repository's effective format is then the honest answer.
async fn signing_format(
    runner: &coral_core::process::GitRunner,
    loc: &coral_core::repo::RepoLocation,
    settings: &ProfileSettings,
) -> coral_core::signing::SigningFormat {
    if let Some(format) = settings.signing.format {
        return format;
    }
    loc.signing_scopes(runner, coral_core::config::AppConfig::default())
        .await
        .map_or_else(
            |_| coral_core::signing::SigningFormat::default(),
            |s| s.effective.format,
        )
}

async fn located(
    path: &str,
) -> Result<
    (
        coral_core::process::GitRunner,
        coral_core::repo::RepoLocation,
    ),
    IpcError,
> {
    let runner = coral_core::process::GitRunner::discover().await?;
    let loc = coral_core::repo::RepoLocation::discover(&runner, Path::new(path)).await?;
    Ok((runner, loc))
}

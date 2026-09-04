//! Settings that are offered before they are finished, and the screen that offers them.
//!
//! One so far: which git Coral runs. It lives here rather than in git's own configuration for
//! a plain reason — reading git's configuration means running git, and this is the setting that
//! decides which git that would be.

// Tauri requires `State` by value in a command signature; it is a handle, not the data.
#![allow(clippy::needless_pass_by_value)]

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use coral_core::process::{GitInstall, GitRunner};

use crate::commands::IpcError;

/// Which git to run.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum GitChoice {
    /// Whatever `git` means on this machine. The default, and the one nobody has to think about.
    #[default]
    System,
    /// The copy inside this build, when there is one.
    Bundled,
    /// A git the user named.
    Custom { path: String },
}

/// Everything the experimental screen stores.
///
/// A struct with one field rather than the field alone: what is experimental changes, and a
/// stored shape that has to be migrated to add the second setting is a poor trade for the four
/// characters it saves.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub git: GitChoice,
}

/// One git the user could choose, as the screen shows it.
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitCandidate {
    pub choice: GitChoice,
    /// Where it is. Empty for the system git until it has been resolved.
    pub path: String,
    /// What it answers to `--version`, or `None` when it cannot be run.
    pub version: Option<String>,
    /// Why it cannot be used, when it cannot.
    pub problem: Option<String>,
}

/// The screen's whole state, so it never has to assemble one from several calls.
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitView {
    /// What is stored.
    pub chosen: GitChoice,
    pub candidates: Vec<GitCandidate>,
    /// The git actually in use, which is not always the one chosen: a chosen git that cannot
    /// run falls back, and saying so here is what keeps that from being silent.
    pub in_use: String,
    pub in_use_version: Option<String>,
}

/// The stored settings, and where they are persisted.
pub struct Experimental {
    inner: Mutex<Settings>,
    path: PathBuf,
}

impl Experimental {
    /// Loads the settings stored at `path`, or the defaults.
    #[must_use]
    pub fn load(path: PathBuf) -> Self {
        let stored = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Self {
            inner: Mutex::new(stored),
            path,
        }
    }

    #[must_use]
    pub fn read(&self) -> Settings {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    fn write(&self, next: Settings) {
        let mut held = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *held = next;
        if let Err(e) = save(&self.path, &held) {
            tracing::warn!(error = %e, path = %self.path.display(), "could not save the settings");
        }
    }

    /// Tells the engine which git to run, from what is stored.
    ///
    /// Called at startup and after every change, so the choice takes effect without a restart.
    pub fn apply(&self) {
        coral_core::process::use_git(install_for(&self.read().git));
    }
}

fn save(path: &Path, settings: &Settings) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let json = serde_json::to_vec_pretty(settings)?;
    std::fs::write(path, json)
}

/// The installation a choice names, or `None` for the system git and for one that is not there.
fn install_for(choice: &GitChoice) -> Option<GitInstall> {
    match choice {
        GitChoice::System => None,
        GitChoice::Bundled => GitInstall::bundled(),
        GitChoice::Custom { path } => {
            let path = PathBuf::from(path);
            if path.is_dir() {
                return GitInstall::at_prefix(&path);
            }
            if !path.is_file() {
                return None;
            }
            Some(GitInstall {
                program: path,
                exec_path: None,
                templates: None,
            })
        }
    }
}

/// Runs a candidate to find out what it is, without letting a bad one fail the whole screen.
async fn describe(choice: GitChoice, install: Option<GitInstall>) -> GitCandidate {
    let install = install.unwrap_or_else(GitInstall::on_path);
    let path = install.program.display().to_string();
    match GitRunner::install(install).await {
        Ok(runner) => GitCandidate {
            choice,
            path,
            version: Some(runner.version().to_string()),
            problem: None,
        },
        Err(e) => GitCandidate {
            choice,
            path,
            version: None,
            problem: Some(e.to_string()),
        },
    }
}

/// What the experimental screen shows about git.
///
/// # Errors
/// Never: a candidate that cannot be run is reported as a candidate with a problem, because
/// that is the case the screen exists to help with.
#[tauri::command]
pub async fn experimental_git(
    settings: tauri::State<'_, Experimental>,
) -> Result<GitView, IpcError> {
    let chosen = settings.read().git;

    let mut candidates = vec![describe(GitChoice::System, None).await];
    if let Some(bundled) = GitInstall::bundled() {
        candidates.push(describe(GitChoice::Bundled, Some(bundled)).await);
    }
    if matches!(chosen, GitChoice::Custom { .. }) {
        candidates.push(describe(chosen.clone(), install_for(&chosen)).await);
    }

    let (in_use, in_use_version) = match GitRunner::discover().await {
        Ok(runner) => (
            runner.path().display().to_string(),
            Some(runner.version().to_string()),
        ),
        Err(_) => (String::from("git"), None),
    };

    Ok(GitView {
        chosen,
        candidates,
        in_use,
        in_use_version,
    })
}

/// Stores which git to run and applies it immediately.
///
/// # Errors
/// Never: an unusable choice is stored and reported back as unusable rather than refused, since
/// the screen shows what is in use and the engine falls back to a git that works.
#[tauri::command]
pub async fn experimental_set_git(
    settings: tauri::State<'_, Experimental>,
    choice: GitChoice,
) -> Result<GitView, IpcError> {
    settings.write(Settings { git: choice });
    settings.apply();
    experimental_git(settings).await
}

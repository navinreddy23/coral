//! Reading and editing the repository's remotes.
//!
//! Separate from [`crate::actions`] because none of these touch a ref, so none of them belong
//! in the undo journal: adding a remote changes configuration, not history.

use coral_core::process::GitRunner;
use coral_core::remote::Remote;
use coral_core::repo::RepoLocation;

use crate::commands::IpcError;

/// What to do to a remote.
#[derive(Debug, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RemoteEdit {
    Add {
        name: String,
        url: String,
    },
    Remove {
        name: String,
    },
    Rename {
        name: String,
        to: String,
    },
    SetUrl {
        name: String,
        url: String,
    },
    /// Drop tracking branches whose remote counterparts are gone.
    Prune {
        name: String,
    },
}

/// Lists the configured remotes.
///
/// # Errors
/// Propagates git failures.
#[tauri::command]
pub async fn remote_list(path: String) -> Result<Vec<Remote>, IpcError> {
    let (runner, loc) = located(&path).await?;
    Ok(loc.remotes(&runner).await?)
}

/// Applies one edit and returns the remotes as they now stand.
///
/// Returning the whole list rather than nothing is what keeps the window from holding a copy
/// that can disagree with the repository.
///
/// # Errors
/// Propagates git failures: a name already in use, an unknown remote, a malformed URL.
#[tauri::command]
pub async fn remote_edit(path: String, edit: RemoteEdit) -> Result<Vec<Remote>, IpcError> {
    let (runner, loc) = located(&path).await?;
    match edit {
        RemoteEdit::Add { name, url } => loc.remote_add(&runner, &name, &url).await?,
        RemoteEdit::Remove { name } => loc.remote_remove(&runner, &name).await?,
        RemoteEdit::Rename { name, to } => loc.remote_rename(&runner, &name, &to).await?,
        RemoteEdit::SetUrl { name, url } => loc.remote_set_url(&runner, &name, &url).await?,
        RemoteEdit::Prune { name } => loc.remote_prune(&runner, &name).await?,
    }
    Ok(loc.remotes(&runner).await?)
}

/// Where a commit is served on the web, for the remote the repository was cloned from.
///
/// Returns `None` rather than failing when the remote is not a host we recognise: an internal
/// git server has no web address to offer, and that is not an error worth a dialog.
///
/// # Errors
/// Propagates git failures from reading the remotes.
#[tauri::command]
pub async fn commit_url(
    path: String,
    oid: String,
    remote: Option<String>,
) -> Result<Option<String>, IpcError> {
    let (runner, loc) = located(&path).await?;
    let remotes = loc.remotes(&runner).await?;
    let wanted = remote.as_deref().unwrap_or("origin");
    let chosen = remotes
        .iter()
        .find(|r| r.name == wanted)
        .or_else(|| remotes.first());

    Ok(chosen
        .and_then(|r| coral_hosting::Host::detect(&r.fetch_url).ok())
        .map(|host| host.commit_url(&oid)))
}

async fn located(path: &str) -> Result<(GitRunner, RepoLocation), coral_core::CoralError> {
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, std::path::Path::new(path)).await?;
    Ok((runner, loc))
}

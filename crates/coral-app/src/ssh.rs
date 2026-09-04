use coral_core::config::AppConfig;
use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;
use coral_core::ssh::{SshConfig, SshKey, SshOverrides, SshScopes};

use crate::commands::IpcError;

async fn located(path: &str) -> Result<(GitRunner, RepoLocation), IpcError> {
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, std::path::Path::new(path)).await?;
    Ok((runner, loc))
}

/// The ssh settings at every level for a repository.
///
/// # Errors
/// Propagates git failures.
#[tauri::command]
pub async fn ssh_read(path: String) -> Result<SshScopes, IpcError> {
    let (runner, loc) = located(&path).await?;
    Ok(loc.ssh_scopes(&runner, AppConfig::default()).await?)
}

/// Sets the app-level defaults every repository inherits.
///
/// # Errors
/// Propagates git failures.
#[tauri::command]
pub async fn ssh_set_app(path: String, config: SshConfig) -> Result<SshScopes, IpcError> {
    let (runner, loc) = located(&path).await?;
    loc.set_ssh_global(&runner, AppConfig::default(), &config)
        .await?;
    Ok(loc.ssh_scopes(&runner, AppConfig::default()).await?)
}

/// Sets what this repository overrides. A `null` field clears that override.
///
/// # Errors
/// Propagates git failures.
#[tauri::command]
pub async fn ssh_set_repo(path: String, overrides: SshOverrides) -> Result<SshScopes, IpcError> {
    let (runner, loc) = located(&path).await?;
    loc.set_ssh_local(&runner, &overrides).await?;
    Ok(loc.ssh_scopes(&runner, AppConfig::default()).await?)
}

/// Every key pair in the user's ssh directory.
///
/// # Errors
/// Never; an unreadable directory lists nothing rather than failing, because a machine with no
/// keys yet is the normal case on this screen.
#[tauri::command]
#[allow(clippy::unused_async)]
pub async fn ssh_keys() -> Result<Vec<SshKey>, IpcError> {
    Ok(coral_core::ssh::keys_in(
        &coral_core::ssh::default_directory(),
    ))
}

/// Creates an ed25519 key pair.
///
/// # Errors
/// [`coral_core::CoralError::Refused`] with `ssh-keygen`'s own message, which explains a path
/// that already exists or a directory that cannot be written.
#[tauri::command]
pub async fn ssh_generate(
    name: String,
    comment: String,
    passphrase: String,
) -> Result<SshKey, IpcError> {
    let at = coral_core::ssh::default_directory().join(name);
    Ok(coral_core::ssh::generate(&at, &comment, &passphrase).await?)
}

/// Reads a public key file, so its text can be copied and pasted into a host's settings.
///
/// # Errors
/// [`coral_core::CoralError::Io`] when the file cannot be read.
#[tauri::command]
#[allow(clippy::unused_async)]
pub async fn ssh_public_key(path: String) -> Result<String, IpcError> {
    let text = std::fs::read_to_string(&path).map_err(coral_core::CoralError::Io)?;
    Ok(text.trim().to_owned())
}
